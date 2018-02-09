use jsonrpc_core::*;
use jsonrpc_core::error::Error;
use jsonrpc_macros::IoDelegate;
use rpc::types::*;
use serde::Serialize;
use std::result::Result;
use std::sync::Arc;

use blockscape_core::record_keeper::PlotID;
use blockscape_core::primitives::Coord;
use blockscape_core::primitives::*;

use game::CheckersGame;
use checkers;

pub struct CheckersRPC {
    game: Arc<CheckersGame>,
    my_player: U160
}

impl CheckersRPC {
    pub fn add(game: Arc<CheckersGame>, my_player: U160, io: &mut MetaIoHandler<SocketMetadata, LogMiddleware>) -> Arc<CheckersRPC> {
        let rpc = Arc::new(CheckersRPC {
            game,
            my_player
        });

        let mut d = IoDelegate::<CheckersRPC, SocketMetadata>::new(rpc.clone());
        d.add_method_with_meta("get_checkers_board", Self::get_checkers_board);
        d.add_method_with_meta("play_checkers", Self::play_checkers);
        d.add_method_with_meta("new_checkers_game", Self::new_checkers_game);
        d.add_method_with_meta("get_my_player", Self::get_my_player);

        io.extend_with(d);
        rpc
    }

    fn get_checkers_board(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let p_arr = parse_args_simple(params, 2..3)?;

        let pid = read_plot_id(&p_arr[0..2])?;

        // TODO: take into oaccount move number
        to_rpc_res(self.game.get_board(pid, None).map(|b| format!("{}", b)).map_err(|_| Error::internal_error()))
    }

    fn play_checkers(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let p_arr = parse_args_simple(params, 4..100)?;

        let pid = read_plot_id(&p_arr[0..2])?;

        let c = read_chess_coord(&p_arr[2])?;

        let e = if p_arr[3].to_lowercase() == "move" {
            let n = read_direction(&p_arr[4])?;
            Ok(checkers::Event::Move(checkers::Board::rc_to_idx(c.0 as u8, c.1 as u8).unwrap_or(0), n))
        }
        else if p_arr[3].to_lowercase() == "jump" {
            let mut moves = Vec::with_capacity(p_arr.len() - 4);

            let mut iter = p_arr.into_iter();

            while let Some(m) = iter.next() {
                moves.push(read_direction(&m)?);
            }

            Ok(checkers::Event::Jump(checkers::Board::rc_to_idx(c.0 as u8, c.1 as u8).unwrap_or(0), moves))
        }
        else {
            return Err(Error::invalid_params(format!("Unrecognized play command: {}", p_arr[3])));
        }?;

        to_rpc_res(self.game.play(pid, e).map_err(map_rk_err))
    }

    fn new_checkers_game(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let p_arr = parse_args_simple(params, 3..4)?;

        let pid = read_plot_id(&p_arr[0..2])?;

        let other_player: U160 = p_arr[2].parse().map_err(|_| Error::invalid_params("Could not parse other player!"))?;

        let event = checkers::Event::Start(self.my_player, other_player);

        to_rpc_res(self.game.play(pid, event).map_err(map_rk_err))
    }

    fn get_my_player(&self, _params: Params, _meta: SocketMetadata) -> RpcResult {
        let m: JU160 = self.my_player.into();

        to_rpc_res(to_rpc_res(Ok(m)))
    }
}

#[inline]
fn read_plot_id(p_arr: &[String]) -> Result<PlotID, Error> {
    let x = p_arr[0].parse().map_err(|_| Error::invalid_params("X coordinate is not a number!"))?;
    let y = p_arr[1].parse().map_err(|_| Error::invalid_params("Y coordinate is not a number!"))?;

    Ok(Coord(x, y))
}

fn read_chess_coord(p: &String) -> Result<Coord, Error> {

    if p.len() != 2 {
        return Err(Error::invalid_params("Chess coordinate is not valid"));
    }

    let p = p.to_lowercase();

    let mut chars = p.chars();

    let row = chars.next().unwrap() as u8 - ('a' as u8);
    let col = chars.next().unwrap() as u8 - ('1' as u8);

    if row >= 8 || col >= 8 {
        return Err(Error::invalid_params("Chess coordinate is not valid"));
    }

    Ok(Coord(row as i32, col as i32))
}

fn read_direction(p: &String) -> Result<checkers::Direction, Error> {

    match p.as_str() {
        "NW" => Ok(checkers::Direction::NW),
        "NE" => Ok(checkers::Direction::NE),
        "SE" => Ok(checkers::Direction::SE),
        "SW" => Ok(checkers::Direction::SW),
        _ => Err(Error::invalid_params("Invalid move direction"))
    }
}

#[inline]
fn to_rpc_res<T: Serialize>(r: Result<T, Error>) -> RpcResult {
    r.map(|v| to_value(v).unwrap())
}