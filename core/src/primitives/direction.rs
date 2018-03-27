use std::slice::Iter;

#[derive(Copy, Clone)]
pub enum Direction {
    N, NE, E, SE, S, SW, W, NW
}

impl Direction {
    /// An iterator over all the directions.
    pub fn iterator() -> Iter<'static, Direction> {
        use self::Direction::*;
        static DIRECTIONS: [Direction; 8] = [N, NE, E, SE, S, SW, W, NW];
        DIRECTIONS.into_iter()
    }

    pub fn cardinal_iterator() -> Iter<'static, Direction> {
        use self::Direction::*;
        static DIRECTIONS: [Direction; 4] = [N, E, S, W];
        DIRECTIONS.into_iter()
    }

    /// Retrieve the change in x and change in y which represent the direction specified.
    #[inline]
    pub fn dx_dy(self) -> (i8, i8) {
        use self::Direction::*;
        match self {
            N => (0, 1),
            NE => (1, 1),
            E => (1, 0),
            SE => (1, -1),
            S => (0, -1),
            SW => (-1, -1),
            W => (-1, 0),
            NW => (-1, 1)
        }
    }

    /// Returns true if the direction is a cardinal direction, (i.e. N, E, S, or W).
    #[inline]
    pub fn is_cardinal(self) -> bool {
        use self::Direction::*;
        match self {
            N | E | S | W => true,
            _ => false
        }
    }
}