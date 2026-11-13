//! How many cells a window holds.

use felis_protocol::messages::RequestedDims;

/// The grid a window of `width` × `height` pixels holds in cells of
/// `cell_width` × `cell_height` pixels. Every axis holds at least one
/// cell, however small the window.
pub fn dims(width: u32, height: u32, cell_width: u32, cell_height: u32) -> RequestedDims {
    let cells = |pixels: u32, cell: u32| pixels.checked_div(cell).unwrap_or(0).max(1);
    RequestedDims {
        rows: cells(height, cell_height),
        cols: cells(width, cell_width),
        pixel_w: width,
        pixel_h: height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whole_cells_fill_the_window() {
        let dims = dims(800, 600, 10, 20);
        assert_eq!((dims.rows, dims.cols), (30, 80));
    }

    #[test]
    fn a_partial_cell_is_left_out() {
        let dims = dims(809, 619, 10, 20);
        assert_eq!((dims.rows, dims.cols), (30, 80));
    }

    #[test]
    fn a_window_smaller_than_a_cell_still_holds_one() {
        let dims = dims(3, 3, 10, 20);
        assert_eq!((dims.rows, dims.cols), (1, 1));
    }

    #[test]
    fn the_window_s_pixels_are_reported_as_they_are() {
        let dims = dims(809, 619, 10, 20);
        assert_eq!((dims.pixel_w, dims.pixel_h), (809, 619));
    }

    #[test]
    fn a_cell_without_size_does_not_divide_by_zero() {
        let dims = dims(800, 600, 0, 0);
        assert_eq!((dims.rows, dims.cols), (1, 1));
    }
}
