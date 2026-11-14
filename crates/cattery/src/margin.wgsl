// Moves the grid right by `grid::inset`, splitting the width the grid does
// not fill evenly between its sides, and paints the margins around it with
// the colour of the nearest edge cell, so a program that paints its own
// background shows it out to the window's edge rather than framed by the
// theme's.
//
// felis knows where it drew the grid but hands a stage only the window and
// cell sizes, so the grid is found again from those: `cells` and `columns`
// mirror `grid::dims`, `origin` mirrors felis's `letterbox_origin`, and
// `inset` mirrors `grid::inset`.

struct FelisPostUniforms {
    resolution_px: vec2<f32>,
    cell_size_px: vec2<f32>,
}

@group(0) @binding(0) var<uniform> u: FelisPostUniforms;
@group(0) @binding(1) var backbuffer: texture_2d<f32>;

fn cells(pixels: f32, cell: f32) -> f32 {
    if cell <= 0.0 {
        return 1.0;
    }
    return max(floor(pixels / cell), 1.0);
}

fn origin(pixels: f32, cell: f32, count: f32) -> f32 {
    let surplus = pixels - count * cell;
    if surplus >= cell {
        return floor(surplus / 2.0);
    }
    return min(surplus, 0.0);
}

fn columns(pixels: f32, cell: f32) -> f32 {
    return cells(max(pixels - 2.0 * ceil(cell / 10.0), 0.0), cell);
}

fn inset(pixels: f32, cell: f32, count: f32) -> f32 {
    let spare = pixels - count * cell;
    if spare < 0.0 || spare >= cell {
        return 0.0;
    }
    return floor(spare / 2.0);
}

@fragment
fn fs_post(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    let window = u.resolution_px;
    let cell = u.cell_size_px;
    let count = vec2<f32>(columns(window.x, cell.x), cells(window.y, cell.y));
    let drawn = vec2<f32>(origin(window.x, cell.x, count.x), origin(window.y, cell.y, count.y));
    let shift = vec2<f32>(inset(window.x, cell.x, count.x), 0.0);
    let start = drawn + shift;
    let first = max(start, vec2<f32>(0.0));
    let last = min(start + count * cell, window) - 1.0;
    let pixel = floor(uv * window);
    let nearest = clamp(pixel, first, last);
    if all(nearest == pixel) {
        return textureLoad(backbuffer, vec2<i32>(pixel - shift), 0);
    }
    // The corner of the nearest edge cell rather than its nearest pixel:
    // glyphs reach a cell's edges (a `d`'s stem, a `│`) far more often than
    // its corner, and a glyph's pixel stretched across the margin shows.
    let corner = max(start + floor((nearest - start) / cell) * cell, first);
    return textureLoad(backbuffer, vec2<i32>(max(corner - shift, vec2<f32>(0.0))), 0);
}
