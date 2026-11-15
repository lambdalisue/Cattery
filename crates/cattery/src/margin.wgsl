// Moves the grid right by `grid::inset`, splitting the width the grid does
// not fill evenly between its sides, and down to start `top` pixels from the
// window's top. Paints the `top` pixels above it with the theme's background,
// and the margins around it with the colour of the nearest edge cell, so a
// program that paints its own background shows it out to the window's edge
// rather than framed by the theme's.
//
// felis knows where it drew the grid but hands a stage only the window and
// cell sizes, so the grid is found again from those: `cells` and `columns`
// mirror `grid::dims` of the window less `top`, `origin` mirrors felis's
// `letterbox_origin`, and `inset` mirrors `grid::inset`.

struct FelisPostUniforms {
    resolution_px: vec2<f32>,
    cell_size_px: vec2<f32>,
    time_s: f32,
    time_delta_s: f32,
    frame: u32,
    contract_version: u32,
    cursor_rect: vec4<f32>,
    prev_cursor_rect: vec4<f32>,
    cursor_color: vec4<f32>,
    trail_color: vec4<f32>,
    trail_corners_x: vec4<f32>,
    trail_corners_y: vec4<f32>,
    cursor_change_time_s: f32,
    cursor_style: u32,
    cursor_visible: u32,
    focused: u32,
    mouse_pos: vec4<f32>,
    mouse_buttons: vec4<f32>,
    background: vec4<f32>,
    foreground: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: FelisPostUniforms;
@group(0) @binding(1) var backbuffer: texture_2d<f32>;

// Where the grid starts, in pixels from the window's top: below a title bar
// the window's content is drawn under.
override top: f32 = 0.0;

// The width of each button of the title bar Cattery draws, mirroring
// `chrome::Bar`; `0` where the platform draws the title bar.
override button: f32 = 0.0;

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

fn shade(uv: vec2<f32>) -> vec4<f32> {
    let window = u.resolution_px;
    let cell = u.cell_size_px;
    let count = vec2<f32>(columns(window.x, cell.x), cells(window.y - top, cell.y));
    let drawn = vec2<f32>(origin(window.x, cell.x, count.x), origin(window.y, cell.y, count.y));
    let shift = vec2<f32>(inset(window.x, cell.x, count.x), top - drawn.y);
    let start = drawn + shift;
    let first = max(start, vec2<f32>(0.0));
    let last = min(start + count * cell, window) - 1.0;
    let pixel = floor(uv * window);
    if pixel.y < top {
        // The grid leaves at least `top` pixels unfilled in what felis drew,
        // some of them at its foot, which felis clears to the theme's
        // background.
        let background = textureLoad(backbuffer, vec2<i32>(0, i32(window.y) - 1), 0);
        return bar(pixel + 0.5, window, background);
    }
    let nearest = clamp(pixel, first, last);
    if all(nearest == pixel) {
        return textureLoad(backbuffer, vec2<i32>(pixel - shift), 0);
    }
    // The corner of the nearest edge cell rather than its nearest pixel:
    // glyphs reach a cell's edges (a `d`'s stem, a `│`) far more often than
    // its corner, and a glyph's pixel stretched across the margin shows.
    var corner = max(start + floor((nearest - start) / cell) * cell, first);
    // The cursor is no colour of the cell it is on: stretched across the
    // margin it would draw a bar out to the window's edge. The cell beside
    // it, along the edge, gives the colour instead.
    if under_cursor(corner - shift, window) {
        let left = corner.x - cell.x;
        corner.x = select(corner.x + cell.x, left, left >= first.x);
    }
    return textureLoad(backbuffer, vec2<i32>(max(corner - shift, vec2<f32>(0.0))), 0);
}

// Whether the pixel `at` of what felis drew is under the cursor felis drew.
fn under_cursor(at: vec2<f32>, window: vec2<f32>) -> bool {
    let low = u.cursor_rect.xy * window;
    let high = low + u.cursor_rect.zw * window;
    return u.cursor_rect.z > 0.0 && all(at >= floor(low)) && all(at < ceil(high));
}

// Which of the title bar's buttons is at `at`, mirroring `chrome::Bar::part`:
// 0 the menu, 1 minimize, 2 maximize, 3 close; -1 for none.
fn part(at: vec2<f32>, window: vec2<f32>) -> i32 {
    if button <= 0.0 || at.y < 0.0 || at.y >= top || at.x < 0.0 || at.x >= window.x {
        return -1;
    }
    let from_right = window.x - at.x;
    if at.x < button {
        return 0;
    } else if from_right <= button {
        return 3;
    } else if from_right <= 2.0 * button {
        return 2;
    } else if from_right <= 3.0 * button {
        return 1;
    }
    return -1;
}

fn segment(p: vec2<f32>, a: vec2<f32>, b: vec2<f32>) -> f32 {
    let along = clamp(dot(p - a, b - a) / dot(b - a, b - a), 0.0, 1.0);
    return length(p - a - (b - a) * along);
}

// How far `p`, from the middle of button `which`, is from its icon, `h` half
// the icon's size.
fn icon(which: i32, p: vec2<f32>, h: f32) -> f32 {
    switch which {
        case 0: {
            let gap = h * 0.7;
            return min(
                segment(p, vec2<f32>(-h, -gap), vec2<f32>(h, -gap)),
                min(
                    segment(p, vec2<f32>(-h, 0.0), vec2<f32>(h, 0.0)),
                    segment(p, vec2<f32>(-h, gap), vec2<f32>(h, gap)),
                ),
            );
        }
        case 1: {
            return segment(p, vec2<f32>(-h, 0.0), vec2<f32>(h, 0.0));
        }
        case 2: {
            return abs(max(abs(p.x), abs(p.y)) - h);
        }
        default: {
            return min(
                segment(p, vec2<f32>(-h, -h), vec2<f32>(h, h)),
                segment(p, vec2<f32>(-h, h), vec2<f32>(h, -h)),
            );
        }
    }
}

// The title bar at `at` over `background`: the buttons' icons in the
// theme's foreground, and the button under the pointer marked, the close
// button in red as Windows marks it.
fn bar(at: vec2<f32>, window: vec2<f32>, background: vec4<f32>) -> vec4<f32> {
    let which = part(at, window);
    if which < 0 {
        return background;
    }
    var middle = vec2<f32>(button * 0.5, top * 0.5);
    if which > 0 {
        middle.x = window.x - (3.5 - f32(which)) * button;
    }
    let scale = top / 34.0;
    let width = max(1.0, round(scale));
    let coverage = clamp(width * 0.5 + 0.5 - icon(which, at - middle, 5.0 * scale), 0.0, 1.0);
    let pointer = u.mouse_pos.xy * window;
    let hovered = part(pointer, window) == which;
    var fill = background;
    var ink = u.foreground;
    if hovered && which == 3 {
        fill = vec4<f32>(0.55, 0.026, 0.012, 1.0);
        ink = vec4<f32>(1.0, 1.0, 1.0, 1.0);
    } else if hovered {
        fill = vec4<f32>(mix(background.rgb, u.foreground.rgb, 0.12), background.a);
    }
    return mix(fill, vec4<f32>(ink.rgb, max(fill.a, ink.a)), coverage);
}

@fragment
fn fs_post(@location(0) uv: vec2<f32>) -> @location(0) vec4<f32> {
    return shade(uv);
}
