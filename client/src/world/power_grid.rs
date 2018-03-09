use definitions::*;

// A piece of the grid which is described by a starting point, and then a length in the x or y direction.
//struct GridSegment {
//    start: Coord,
//    length: i32  // Positive means in +x direction, negative means in +y direction.
//}

/// Concept of a power or data grid. It is the over-arching result of many components that can be
/// reduced to a list of producers and consumers.
pub struct Grid<'a> {
    segments: Vec<&'a Structure>,

    producers: Vec<()>,
    consumers: Vec<()>
}