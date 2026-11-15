//! How many cells a window holds, and where they sit in it.

use felis_protocol::messages::RequestedDims;
use felis_render_wgpu::{PostParam, PostStageSource};

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

/// How wide a window is whose [`dims`] are `cols` columns of cells
/// `cell_width` pixels wide: the columns and the margin at either side.
pub const fn width_for(cols: u32, cell_width: u32) -> u32 {
    cols * cell_width + 2 * cell_width.div_ceil(10)
}

/// How tall a window is whose [`dims`] are `rows` rows of cells
/// `cell_height` pixels tall, starting `top` pixels down.
pub const fn height_for(rows: u32, cell_height: u32, top: u32) -> u32 {
    rows * cell_height + top
}

fn cells(pixels: u32, cell: u32) -> u32 {
    pixels.checked_div(cell).unwrap_or(0).max(1)
}

/// The stage that shows the grid [`inset`] and starting `top` pixels down,
/// paints the margin around it with its edge cells' colour, and the `top`
/// pixels above it with the theme's background, with the buttons of a title
/// bar Cattery draws in it where `button`, their width, is not `0`.
///
/// felis draws the grid from the window's left edge, centring it only once
/// a whole cell is left over, and clears the margin to the theme's
/// background, which frames a program that paints a background of its own.
/// felis can neither inset the grid by less than a cell nor extend the edge
/// cells' background, so until it can, this post-process stage moves what
/// felis drew and copies each edge cell's corner pixel out.
pub fn margin(top: u32, button: u32) -> PostStageSource {
    PostStageSource {
        label: "cattery margin".to_owned(),
        wgsl: include_str!("margin.wgsl").to_owned(),
        params: vec![
            ("top".to_owned(), PostParam::Float(f64::from(top))),
            ("button".to_owned(), PostParam::Float(f64::from(button))),
        ],
    }
}

/// The cell, as `(row, column)`, whose colour [`margin`] must not copy out
/// for the cursor felis may paint on it: the cursor `at`, where it is
/// `visible`, on a grid of `size` rows and columns, unless the view is
/// `scrolled` back, where felis paints none.
pub fn cursor_cell(
    visible: bool,
    scrolled: bool,
    at: (u16, u16),
    size: (u16, u16),
) -> Option<(u16, u16)> {
    (visible && !scrolled && at.0 < size.0 && at.1 < size.1).then_some(at)
}

#[cfg(test)]
mod tests {
    use felis_render_core::layout::letterbox_origin;
    use felis_render_wgpu::validate_post_shader;

    use super::*;

    #[test]
    fn the_margin_stage_is_told_of_the_cursor_on_the_live_screen() {
        assert_eq!(cursor_cell(true, false, (23, 5), (24, 80)), Some((23, 5)));
    }

    #[test]
    fn a_hidden_cursor_or_one_scrolled_away_or_off_the_grid_is_none() {
        assert_eq!(cursor_cell(false, false, (23, 5), (24, 80)), None);
        assert_eq!(cursor_cell(true, true, (23, 5), (24, 80)), None);
        assert_eq!(cursor_cell(true, false, (24, 5), (24, 80)), None);
        assert_eq!(cursor_cell(true, false, (0, 80), (24, 80)), None);
    }

    #[test]
    fn the_margin_stage_passes_felis_s_checks() {
        validate_post_shader(&margin(0, 0).wgsl).unwrap();
    }

    #[test]
    fn the_margin_stage_is_told_where_the_grid_starts_and_how_wide_buttons_are() {
        assert_eq!(
            margin(56, 92).params,
            [
                ("top".to_owned(), PostParam::Float(56.0)),
                ("button".to_owned(), PostParam::Float(92.0)),
            ]
        );
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
    fn a_window_sized_for_a_grid_holds_exactly_that_grid() {
        for cell in [[10, 20], [17, 34], [9, 19], [5, 12]] {
            for (cols, rows) in [(1, 1), (80, 24), (120, 35), (192, 50)] {
                for top in [0, 56] {
                    let width = width_for(cols, cell[0]);
                    let height = height_for(rows, cell[1], top);
                    let dims = dims(width, height - top, cell[0], cell[1]);
                    assert_eq!(
                        (dims.cols, dims.rows),
                        (cols, rows),
                        "{cell:?} {cols}x{rows} under {top}"
                    );
                }
            }
        }
    }

    #[test]
    fn a_window_sized_for_a_grid_leaves_only_the_margin_over() {
        assert_eq!(width_for(80, 10), 802);
        assert_eq!(width_for(80, 17), 1364);
        assert_eq!(height_for(24, 20, 56), 536);
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
