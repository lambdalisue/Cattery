//! How many cells a window holds, and where they sit in it.

use felis_protocol::messages::RequestedDims;
use felis_render_wgpu::PostStageSource;

/// The grid a window of `width` × `height` pixels holds in cells of
/// `cell_width` × `cell_height` pixels: every row that fits, and the columns
/// that fit with a tenth of a cell, in whole pixels, kept free at either
/// side. Every axis holds at least one cell, however small the window.
pub fn dims(width: u32, height: u32, cell_width: u32, cell_height: u32) -> RequestedDims {
    let rows = cells(height, cell_height);
    let cols = cells(
        width.saturating_sub(2 * cell_width.div_ceil(10)),
        cell_width,
    );
    RequestedDims {
        rows,
        cols,
        pixel_w: cols * cell_width,
        pixel_h: rows * cell_height,
    }
}

/// How far right of where felis draws it the grid is shown, so the width
/// its columns leave over is split evenly between the sides. felis centres
/// the grid itself once a whole cell is left over, and draws it from the
/// left edge otherwise. Rows start at the top, as in every terminal, and
/// leave theirs below.
pub fn inset(width: u32, cell_width: u32) -> u32 {
    let cols = dims(width, 0, cell_width, 0).cols;
    match width.checked_sub(cols * cell_width) {
        Some(spare) if spare < cell_width => spare / 2,
        _ => 0,
    }
}

fn cells(pixels: u32, cell: u32) -> u32 {
    pixels.checked_div(cell).unwrap_or(0).max(1)
}

/// The stage that shows the grid [`inset`], and paints the margin around
/// it with its edge cells' colour.
///
/// felis draws the grid from the window's left edge, centring it only once
/// a whole cell is left over, and clears the margin to the theme's
/// background, which frames a program that paints a background of its own.
/// felis can neither inset the grid by less than a cell nor extend the edge
/// cells' background, so until it can, this post-process stage moves what
/// felis drew and copies each edge cell's corner pixel out.
pub fn margin() -> PostStageSource {
    PostStageSource {
        label: "cattery margin".to_owned(),
        wgsl: include_str!("margin.wgsl").to_owned(),
        params: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use felis_render_core::layout::letterbox_origin;
    use felis_render_wgpu::validate_post_shader;

    use super::*;

    #[test]
    fn the_margin_stage_passes_felis_s_checks() {
        validate_post_shader(&margin().wgsl).unwrap();
    }

    #[test]
    fn every_row_that_fits() {
        assert_eq!(dims(800, 600, 10, 20).rows, 30);
        assert_eq!(dims(800, 619, 10, 20).rows, 30);
    }

    #[test]
    fn the_columns_that_fit_inside_a_tenth_of_a_cell_at_either_side() {
        assert_eq!(dims(802, 600, 10, 20).cols, 80);
        assert_eq!(dims(801, 600, 10, 20).cols, 79);
        assert_eq!(dims(800, 600, 10, 20).cols, 79);
    }

    #[test]
    fn a_tenth_of_a_cell_is_rounded_up_to_a_whole_pixel() {
        assert_eq!(dims(1364, 600, 17, 20).cols, 80);
        assert_eq!(dims(1363, 600, 17, 20).cols, 79);
    }

    #[test]
    fn a_window_too_small_for_the_margin_still_holds_a_cell() {
        let dims = dims(3, 3, 10, 20);
        assert_eq!((dims.rows, dims.cols), (1, 1));
    }

    #[test]
    fn the_grid_s_pixels_are_reported() {
        let dims = dims(809, 619, 10, 20);
        assert_eq!((dims.pixel_w, dims.pixel_h), (800, 600));
    }

    #[test]
    fn a_cell_without_size_does_not_divide_by_zero() {
        let dims = dims(800, 600, 0, 0);
        assert_eq!((dims.rows, dims.cols), (1, 1));
        assert_eq!(inset(800, 0), 0);
    }

    #[test]
    fn a_window_narrower_than_a_cell_is_not_inset() {
        assert_eq!(inset(3, 10), 0);
    }

    /// Where the grid is shown is where felis draws it, letterboxed once a
    /// whole cell is spare, moved right by [`inset`].
    #[test]
    fn the_grid_sits_a_tenth_of_a_cell_or_more_from_either_side_and_centred() {
        for cell in [[10, 20], [17, 34], [9, 19]] {
            for width in cell[0] * 3..cell[0] * 120 {
                let dims = dims(width, 600, cell[0], cell[1]);
                let [drawn, y] = letterbox_origin(
                    u16::try_from(dims.rows).unwrap(),
                    u16::try_from(dims.cols).unwrap(),
                    cell,
                    [width, 600],
                );
                let left = drawn as u32 + inset(width, cell[0]);
                let right = width - dims.pixel_w - left;
                let tenth = cell[0].div_ceil(10);
                assert!(
                    left >= tenth && right >= tenth,
                    "{cell:?} {width}: {left} | {right}"
                );
                assert!(
                    left.abs_diff(right) <= 1,
                    "{cell:?} {width}: {left} | {right}"
                );
                assert_eq!(y, 0.0, "{cell:?} {width}");
            }
        }
    }
}
