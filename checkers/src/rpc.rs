use blockscape_core::rpc::*;
use blockscape_core::rpc::RPC;
use serde::Serialize;
use std::result::Result;
use std::sync::Arc;
use std::rc::Rc;
use std::net::SocketAddr;

use serde_json;

use openssl::pkey::PKey;

use blockscape_core::record_keeper::PlotID;
use blockscape_core::primitives::Coord;
use blockscape_core::primitives::*;
use blockscape_core::time::Time;
use blockscape_core::bin::Bin;
use blockscape_core::hash::hash_pub_key;

use game::CheckersGame;
use context::Context;
use checkers;

pub fn make_rpc(ctx: &Rc<Context>, bind_addr: SocketAddr) -> RPC {

    let mut handler = RPC::build_handler();

    ControlRPC::add(&ControlRPC::new(), &mut handler);
    NetworkRPC::add(&NetworkRPC::new(ctx.network.clone()), &mut handler);

    let forge_key = PKey::private_key_from_der(&ctx.forge_key.private_key_to_der().unwrap()).unwrap();
    BlockchainRPC::add(&BlockchainRPC::new(ctx.rk.clone(), forge_key), &mut handler);
    CheckersRPC::add(&CheckersRPC::new(ctx.game.clone(), PKey::private_key_from_der(&ctx.forge_key.private_key_to_der().unwrap()).unwrap()), &mut handler);

    RPC::run(bind_addr, handler)
}

pub struct CheckersRPC {
    game: Arc<CheckersGame>,
    my_player: PKey,
    my_player_hash: U160
}

impl RPCHandler for CheckersRPC {
    fn add(this: &Arc<CheckersRPC>, io: &mut MetaIoHandler<SocketMetadata, LogMiddleware>) {

        let mut d = IoDelegate::<CheckersRPC, SocketMetadata>::new(this.clone());
        d.add_method_with_meta("get_checkers_board", Self::get_checkers_board);
        d.add_method_with_meta("play_checkers", Self::play_checkers);
        d.add_method_with_meta("new_checkers_game", Self::new_checkers_game);
        d.add_method_with_meta("join_checkers_game", Self::join_checkers_game);
        d.add_method_with_meta("register_my_player", Self::register_my_player);
        d.add_method_with_meta("get_my_player", Self::get_my_player);

        io.extend_with(d);
    }
}

impl CheckersRPC {

    fn new(game: Arc<CheckersGame>, my_player: PKey) -> Arc<CheckersRPC> { 
        Arc::new(CheckersRPC {
            game,
            my_player_hash: hash_pub_key(&my_player.public_key_to_der().unwrap()),
            my_player
        })
    }

    fn get_checkers_board(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let p_arr = parse_args_simple(params, 2..3)?;

        let pid = read_plot_id(&p_arr[0..2])?;
        
        let players = self.game.get_players(pid).map_err(|_| Error::internal_error())?;
        let status = if players.0 == U160_ZERO && players.1 == U160_ZERO {
			"not started"
		}
		else if players.0 == U160_ZERO || players.1 == U160_ZERO {
			"waiting for join"
		}
        else {
            "active"
        };
        
        let player_info = format!("PLAYER 1: {}\nPLAYER 2: {}", players.0, players.1);

        // TODO: take into account move number
        to_rpc_res(self.game.get_board(pid, None).map(|b| format!("STATUS: {}\n{}\n{}", status, player_info, b)).map_err(|_| Error::internal_error()))
    }

    fn play_checkers(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let p_arr = parse_args_simple(params, 4..100)?;
        let pid = read_plot_id(&p_arr[0..2])?;
        let c = read_chess_coord(&p_arr[2])?;

        let e = match p_arr[3].to_lowercase().as_str() {
            "move" => {
                let n = read_direction(&p_arr[4])?;
                Ok(checkers::Event::Move(
                    checkers::Board::rc_to_idx(c.0 as u8, c.1 as u8)
                        .unwrap_or(0), n)
                )
            },
            "jump" => {
                let mut moves: Vec<checkers::Direction> = Vec::with_capacity(p_arr.len() - 4);

                for m in &p_arr[4..] {
                    moves.push(read_direction(m)?);
                }

                Ok(checkers::Event::Jump(
                    checkers::Board::rc_to_idx(c.0 as u8, c.1 as u8)
                        .unwrap_or(0), moves)
                )
            },
            _ => { return Err(Error::invalid_params(format!("Unrecognized play command: {}", p_arr[3]))); }
        }?;

        to_rpc_res(self.game.play(pid, e).map_err(map_rk_err))
    }

    fn new_checkers_game(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
        let p_arr = parse_args_simple(params, 3..4)?;
        let pid = read_plot_id(&p_arr[0..2])?;
        let other_player: U160 = p_arr[2].parse().map_err(|_| Error::invalid_params("Could not parse other player!"))?;

        let event = checkers::Event::Start(self.my_player_hash, other_player);

        to_rpc_res(self.game.play(pid, event).map_err(map_rk_err))
    }
    
    fn join_checkers_game(&self, params: Params, _meta: SocketMetadata) -> RpcResult {
		let p_arr = parse_args_simple(params, 2..3)?;
		let pid = read_plot_id(&p_arr[0..2])?;
		let event = checkers::Event::Join(self.my_player_hash);
		
		// find the empty slot
		to_rpc_res(self.game.play(pid, event).map_err(map_rk_err))
	}

    fn get_my_player(&self, _params: Params, _meta: SocketMetadata) -> RpcResult {
        to_rpc_res(Ok(JU160::from(self.my_player_hash)))
    }

    fn register_my_player(&self, _params: Params, _meta: SocketMetadata) -> RpcResult {

        let mut m = Mutation::new();

        m.changes.push(Change::NewValidator{pub_key: self.my_player.public_key_to_der().unwrap().into()});
        
        let txn = Txn {
            timestamp: Time::current(),
            creator: self.my_player_hash,
            mutation: m,
            signature: Bin::new(),
        }.sign(&self.my_player);

        let res = self.game.rk.add_pending_txn(txn, true);

        if res.is_ok() {
            to_rpc_res(Ok(JU160::from(self.my_player_hash)))
        }
        else  {
            Err(map_rk_err(res.unwrap_err()))
        }
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

    let col = chars.next().unwrap() as u8 - ('a' as u8);
    let row = chars.next().unwrap() as u8 - ('1' as u8);

    if row >= 8 || col >= 8 {
        return Err(Error::invalid_params("Chess coordinate is not valid"));
    }

    Ok(Coord(row as i32, col as i32))
}

fn read_direction(p: &String) -> Result<checkers::Direction, Error> {
    let p = p.to_lowercase();
    match p.as_str() {
        "nw" => Ok(checkers::Direction::NW),
        "ne" => Ok(checkers::Direction::NE),
        "se" => Ok(checkers::Direction::SE),
        "sw" => Ok(checkers::Direction::SW),
        _ => Err(Error::invalid_params("Invalid move direction"))
    }
}

#[inline]
fn to_rpc_res<T: Serialize>(r: Result<T, Error>) -> RpcResult {
    r.map(|v| serde_json::to_value(v).unwrap())
}
