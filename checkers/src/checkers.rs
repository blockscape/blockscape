use blockscape_core::primitives::{U160};
use blockscape_core::primitives::Event as CoreEvent;
use blockscape_core::bin::*;
use bincode;
use std::fmt;
use std::error::Error as StdErr;
use std::ops::Deref;

/// Representation of the two players in a game of Chess.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum Player {
    Red, Black
}

impl Player {
    /// Determine what player's turn it is for a given turn number.
    #[inline]
    pub fn from_turn(turn: u64) -> Result<Player, Error> {
        match turn {
            0 | 1 => Err(Error::InvalidPlay),
            t @ _ if t % 2 == 0 => Ok(Player::Red),
            _ => Ok(Player::Black)
        }
    }
}



/// The 4 cardinal directions of checkers game play.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum Direction {
    NW, NE, SE, SW
}

impl Direction {
    /// Move a coordinate one tile in the direction specified. (Will wrap if it goes below 0).
    pub fn move_in_dir(self, r: usize, c: usize) -> (usize, usize) {
        let (dr, dc) = self.dir_vec();
        ((r as i8 + dr) as usize, (c as i8 + dc) as usize)
    }

    /// Move a coordinate two tiles in the direction specified. (Will wrap if it goes below 0)
    pub fn jump_in_dir(self, r: usize, c: usize) -> (usize, usize) {
        let (dr, dc) = self.dir_vec();
        ((r as i8 + dr * 2) as usize, (c as i8 + dc * 2) as usize)
    }

    fn dir_vec(self) -> (i8, i8) {
        use self::Direction::*;
        match self {
            NW => (-1, -1),
            NE => (-1, 1),
            SE => (1, 1),
            SW => (1, -1)
        }
    }
}



/// The base game events for checkers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Event {
	/// Initializes a new game on an empty plot, first player is white, last is black. One slot must be the player's ID,
	/// but the other can be set to "0" to leave an opening for a join (see below)
    Start(U160, U160),
    /// Fill an empty slot in a started game which does not have a second player filled in already
    Join(U160),
    /// Set the specified checkers piece to be located in the direction specified
    Move(u8, Direction),
    /// Like move, but jumps over all the given pieces
    Jump(u8, Vec<Direction>)
} impl CoreEvent for Event {}

impl AsBin for Event {
    fn as_bin(&self) -> Bin {
        bincode::serialize(self, bincode::Infinite).unwrap()
    }
}



/// Values a checker's board tile may take. (i.e. what's on it)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Hash, Eq, PartialEq)]
pub enum Tile {
    None, Red, RedKing, Black, BlackKing
}

impl fmt::Display for Tile {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use self::Tile::*;
        write!(f, "{}", match *self {
            None => '.',
            Red => 'r',
            RedKing => 'R',
            Black => 'b',
            BlackKing => 'B'
        })
    }
}

impl Tile {
    /// Upgrade a piece to a king, (or do nothing if it is not a basic piece).
    pub fn upgrade(self) -> Tile {
        use self::Tile::*;
        match self {
            Red => RedKing,
            Black => BlackKing,
            t @ _ => t
        }
    }

    /// Check if the tile is owned by the given player.
    pub fn is_this_player(self, player: Player) -> bool {
        use self::Tile::*;
        match self {
            Red | RedKing => player == Player::Red,
            Black | BlackKing => player == Player::Black,
            None => false
        }
    }

    /// Check if the tile is owned by a different player.
    pub fn is_other_player(self, player: Player) -> bool {
        use self::Tile::*;
        match self {
            Red | RedKing => player != Player::Red,
            Black | BlackKing => player != Player::Black,
            None => false
        }
    }

    /// Check if the piece on this tile can be moved in the specified direction.
    pub fn valid_direction(self, dir: Direction) -> bool {
        use self::Tile::*;
        use self::Direction::*;
        match self {
            Red => dir == NE || dir == NW,
            Black => dir == SE || dir == SW,
            RedKing | BlackKing => true,
            None => false
        }
    }
}



/// Some things which can go wrong when trying to play a game of Checkers.
#[derive(Debug)]
pub enum Error {
    GameAlreadyStarted,
    InvalidPlay,
    WrongPlayer,
    MissingPiece,
    InvalidCoordinate
}

impl StdErr for Error {
    fn description(&self) -> &str {
        use self::Error::*;
        match *self {
            GameAlreadyStarted => "Game already started.",
            InvalidPlay => "Cannot move the piece in that way.",
            WrongPlayer => "Cannot move the other players pieces.",
            MissingPiece => "Cannot move nothing...",
            InvalidCoordinate => "The coordinate specified is not on the board."
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str(self.description())
    }
}



/// A checkers game board. Top left corner is (0,0); top right corner is (0, 7).
#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq)]
pub struct Board([[Tile; 8]; 8]);

impl Default for Board {
    fn default() -> Board {
        use self::Tile::None as N;
        use self::Tile::Red as R;
        use self::Tile::Black as B;

        Board([
            [B, N, B, N, B, N, B, N],
            [N, B, N, B, N, B, N, B],
            [B, N, B, N, B, N, B, N],
            [N; 8],
            [N; 8],
            [N, R, N, R, N, R, N, R],
            [R, N, R, N, R, N, R, N],
            [N, R, N, R, N, R, N, R]
        ])
    }
}

impl fmt::Display for Board {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        writeln!(f, "\n    A B C D E F G H")?;
        writeln!(f, "  -------------------")?;
        for (c, r) in self.0.iter().enumerate() {
            writeln!(f, "{} | {} {} {} {} {} {} {} {} |", c+1, r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7])?;
        }
        writeln!(f, "  -------------------")
    }
}

impl Deref for Board {
    type Target = [[Tile; 8]; 8];
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Board {
    #[allow(dead_code)]
    pub fn new() -> Board {
        Board([[Tile::None; 8]; 8])
    }

    /// Make a move defined by the event for the given player. This will validate the action to make
    /// sure it is valid and will change the board state if it is valid and return an error if not.
    /// *Note:* The state will remain unchanged if the play is invalid.
    pub fn play(&mut self, event: Event, player: Player) -> Result<(), Error> {
        use self::Error::*;
        match event {
            Event::Move(idx, dir) => {
                let (r, c) = Self::idx_to_rc(idx)?;
                let (r, c) = (r as usize, c as usize);

                // check basic direction and player ownership logic
                if self.0[r][c] == Tile::None { return Err(MissingPiece); }
                if !self.0[r][c].is_this_player(player) { return Err(WrongPlayer); }
                if !self.0[r][c].valid_direction(dir) { return Err(InvalidPlay); }

                // find new position and verify it is valid
                let (nr, nc) = dir.move_in_dir(r, c);
                if nr > 7 || nc > 7 || self.0[nr][nc] != Tile::None {
                    //unsigned, so never less than zero
                    return Err(InvalidPlay);
                }

                // move the piece
                self.0[nr][nc] = self.0[r][c];
                self.0[r][c] = Tile::None;

                // upgrade piece to a king if needed
                if nr == 0 || nr == 7 {
                    self.0[nr][nc] = self.0[nr][nc].upgrade();
                }

                Ok(())
            },
            Event::Jump(idx, path) => {
                let (r, c) = Self::idx_to_rc(idx)?;
                let (mut r, mut c) = (r as usize, c as usize);

                // check basic direction and player ownership logic
                if self.0[r][c] == Tile::None { return Err(MissingPiece); }
                if !self.0[r][c].is_this_player(player) { return Err(WrongPlayer); }
                let (mut nr, mut nc): (usize, usize) = (r, c);

                let old_board = self.0.clone();

                for dir in path {
                    if !self.0[r][c].valid_direction(dir) {
                        self.0 = old_board;
                        return Err(InvalidPlay);
                    }

                    let (pr, pc) = dir.move_in_dir(r, c); //passover piece
                    {let t = dir.jump_in_dir(r, c); nr = t.0; nc = t.1;}

                    if nr > 7 || nc > 7 ||
                        self.0[nr][nc] != Tile::None ||
                        !self.0[pr][pc].is_other_player(player)
                    {
                        //unsigned, so never less than zero
                        //must land on an empty tile
                        //must pass over an opponent tile
                        self.0 = old_board;
                        return Err(InvalidPlay);
                    }

                    // perform jump
                    self.0[nr][nc] = self.0[r][c];
                    self.0[pr][pc] = Tile::None;
                    self.0[r][c] = Tile::None;
                    r = nr; c = nc;
                }

                // upgrade piece to a king if needed
                if nr == 0 || nr == 7 {
                    self.0[nr][nc] = self.0[nr][nc].upgrade();
                }

                Ok(())
            },
            Event::Start(..) => Err(GameAlreadyStarted),
            Event::Join(..) => Err(GameAlreadyStarted),
        }
    }

    /// Convert an index to row-column
    #[inline]
    pub fn idx_to_rc(idx: u8) -> Result<(u8, u8), Error> {
        if idx > 63 {
            Err(Error::InvalidCoordinate)
        } else {
            Ok((idx / 8, idx % 8))
        }
    }

    /// Convert row-column to an index
    #[inline]
    pub fn rc_to_idx(r: u8, c: u8) -> Result<u8, Error> {
        if r > 7 || c > 7 {
            Err(Error::InvalidCoordinate)
        } else {
            Ok(r*8 + c)
        }
    }
}



#[cfg(test)]
mod tests {
    #[test]
    fn from_turn() {
        use super::Player;
        assert!(Player::from_turn(0).is_err());
        assert!(Player::from_turn(1).is_err());
        assert_eq!(Player::from_turn(2).unwrap(), Player::Red);
        assert_eq!(Player::from_turn(3).unwrap(), Player::Black);
        assert_eq!(Player::from_turn(4).unwrap(), Player::Red);
    }

    #[test]
    fn move_in_dir() {
        use super::Direction::*;
        assert_eq!(NE.move_in_dir(1, 0), (0, 1));
        assert_eq!(SE.move_in_dir(0, 0), (1, 1));
        assert_eq!(NW.move_in_dir(1, 1), (0, 0));
        assert_eq!(SW.move_in_dir(1, 1), (2, 0));
    }

    #[test]
    fn jump_in_dir() {
        use super::Direction::*;
        assert_eq!(NE.jump_in_dir(2, 2), (0, 4));
        assert_eq!(SE.jump_in_dir(2, 2), (4, 4));
        assert_eq!(NW.jump_in_dir(2, 2), (0, 0));
        assert_eq!(SW.jump_in_dir(2, 2), (4, 0));
    }

    #[test]
    fn basic_move() {
        use super::Board;
        use super::Event::*;
        use super::Player::*;
        use super::Direction::*;
        let rc_to_idx = |r, c| Board::rc_to_idx(r, c).unwrap();
        let mut board = Board::default();
        assert!(board.play(Move(rc_to_idx(0, 0), SE), Black).is_err()); //collision
        assert!(board.play(Move(rc_to_idx(2, 0), SW), Black).is_err()); //off board
        assert!(board.play(Move(rc_to_idx(3, 1), SW), Black).is_err()); //nothing there
        assert!(board.play(Move(rc_to_idx(2, 0), SE), Black).is_ok()); //valid
        assert!(board.play(Move(rc_to_idx(2, 0), SE), Black).is_err()); //nothing there
        assert!(board.play(Move(rc_to_idx(3, 1), NW), Black).is_err()); //wrong direction
        assert!(board.play(Move(rc_to_idx(3, 1), SW), Black).is_ok()); //valid
        assert!(board.play(Move(rc_to_idx(5, 7), NW), Black).is_err()); //wrong player
        assert!(board.play(Move(rc_to_idx(5, 7), NW), Red).is_ok()); //valid
    }

    #[test]
    fn king_move() {
        use super::Board;
        use super::Tile;
        use super::Event::*;
        use super::Player::*;
        use super::Direction::*;
        let rc_to_idx = |r, c| Board::rc_to_idx(r, c).unwrap();
        let mut board = Board::new();
        board.0[3][3] = Tile::RedKing;
        assert!(board.play(Move(rc_to_idx(3, 3), NW), Black).is_err()); //wrong player
        assert!(board.play(Move(rc_to_idx(3, 3), NW), Red).is_ok()); //valid
        assert!(board.play(Move(rc_to_idx(2, 2), SW), Red).is_ok()); //valid
        assert!(board.play(Move(rc_to_idx(3, 1), SE), Red).is_ok()); //valid
    }

    #[test]
    fn jump() {
        use super::Board;
        use super::Tile;
        use super::Event::*;
        use super::Player::*;
        use super::Direction::*;
        let rc_to_idx = |r, c| Board::rc_to_idx(r, c).unwrap();
        let mut board = Board::default();
        assert!(board.play(Move(rc_to_idx(5, 3), NW), Red).is_ok());
        assert!(board.play(Move(rc_to_idx(2, 0), SE), Black).is_ok());
        assert!(board.play(Jump(rc_to_idx(4, 2), vec![NW]), Red).is_ok());

        board = Board::new();
        board.0[0][0] = Tile::BlackKing;
        board.0[1][1] = Tile::Red;
        board.0[3][3] = Tile::RedKing;
        board.0[3][5] = Tile::Red;
        assert!(board.play(Jump(0, vec![SE, SE, NE]), Black).is_ok());
    }


    #[test]
    fn idx_to_rc() {
        use super::Board;
        assert!(Board::idx_to_rc(120).is_err());
        assert_eq!(Board::idx_to_rc(5).unwrap(), (0, 5));
        assert_eq!(Board::idx_to_rc(63).unwrap(), (7, 7));
        assert_eq!(Board::idx_to_rc(25).unwrap(), (3, 1));
    }

    #[test]
    fn rc_to_idx() {
        use super::Board;
        assert!(Board::rc_to_idx(8, 0).is_err());
        assert!(Board::rc_to_idx(0, 8).is_err());
        assert_eq!(Board::rc_to_idx(7, 7).unwrap(), 63);
        assert_eq!(Board::rc_to_idx(0, 0).unwrap(), 0);
        assert_eq!(Board::rc_to_idx(3, 1).unwrap(), 25);
        assert_eq!(Board::rc_to_idx(0, 5).unwrap(), 5);
    }
}
