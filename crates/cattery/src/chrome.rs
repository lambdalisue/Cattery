//! What a window draws around its grid: on macOS, the title bar it is
//! drawn under; on Linux and Windows, a title bar of Cattery's own in place
//! of the platform's frame.

use winit::window::{ResizeDirection, Window, WindowAttributes};

/// `attributes` with the window's content drawn where its title bar is, so
/// that the title bar shows the theme's background, translucent as the
/// window is: under a see-through title bar on macOS, and without the
/// platform's frame elsewhere, where Cattery draws the title bar itself.
pub fn under_title_bar(attributes: WindowAttributes) -> WindowAttributes {
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::WindowAttributesExtMacOS as _;
        attributes
            .with_titlebar_transparent(true)
            .with_fullsize_content_view(true)
    }
    #[cfg(not(target_os = "macos"))]
    attributes.with_decorations(false)
}

/// The title bar Cattery draws over `window`, where it draws one.
pub fn bar(window: &Window) -> Option<Bar> {
    if cfg!(target_os = "macos") || window.fullscreen().is_some() {
        None
    } else {
        Some(Bar::at(window.scale_factor()))
    }
}

/// How tall, in physical pixels, the title bar over `window`'s content is;
/// `0` where the content is not drawn under one, as in full screen.
pub fn title_bar(window: &Window) -> u32 {
    #[cfg(target_os = "macos")]
    {
        let points = macos::title_bar(window);
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a title bar is a few dozen pixels"
        )]
        let pixels = (points * window.scale_factor()).round().max(0.0) as u32;
        pixels
    }
    #[cfg(not(target_os = "macos"))]
    bar(window).map_or(0, |bar| bar.height)
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2::msg_send;
    use objc2::runtime::AnyObject;
    use objc2_foundation::NSRect;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use winit::window::Window;

    /// The window's height less the part its content lays out in, in
    /// points.
    pub fn title_bar(window: &Window) -> f64 {
        let Ok(handle) = window.window_handle() else {
            return 0.0;
        };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return 0.0;
        };
        // SAFETY: winit hands out the pointer to its live view, and calls
        // reach here on the main thread, as AppKit requires; `window`,
        // `frame` and `contentLayoutRect` are getters that take nothing.
        unsafe {
            let view: &AnyObject = handle.ns_view.cast().as_ref();
            let window: *mut AnyObject = msg_send![view, window];
            if window.is_null() {
                return 0.0;
            }
            let frame: NSRect = msg_send![window, frame];
            let layout: NSRect = msg_send![window, contentLayoutRect];
            (frame.size.height - layout.size.height).max(0.0)
        }
    }
}

/// A part of the title bar Cattery draws where it draws the window's frame
/// itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Part {
    /// The button that opens the app's menu.
    Menu,
    Minimize,
    Maximize,
    Close,
    /// The rest, which moves the window.
    Drag,
}

/// The measures, in physical pixels, of the title bar Cattery draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bar {
    pub height: u32,
    /// The width of each button.
    pub button: u32,
}

impl Bar {
    /// The bar on a display of `scale` physical pixels to the point, sized
    /// as Windows sizes its own.
    pub fn at(scale: f64) -> Self {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a bar of a few dozen pixels"
        )]
        let pixels = |points: f64| (points * scale).round().max(1.0) as u32;
        Self {
            height: pixels(34.0),
            button: pixels(46.0),
        }
    }

    /// The part of the bar at `x`, `y` in a window `width` pixels wide, if
    /// that is in the bar.
    pub fn part(&self, x: f64, y: f64, width: u32) -> Option<Part> {
        if y < 0.0 || y >= f64::from(self.height) {
            return None;
        }
        let button = f64::from(self.button);
        let from_right = f64::from(width) - x;
        Some(if x < button {
            Part::Menu
        } else if from_right <= button {
            Part::Close
        } else if from_right <= 2.0 * button {
            Part::Maximize
        } else if from_right <= 3.0 * button {
            Part::Minimize
        } else {
            Part::Drag
        })
    }
}

/// The edge or corner of a window `size` pixels large that a point at `x`,
/// `y` within `border` pixels of it resizes the window from.
pub fn edge(x: f64, y: f64, size: (u32, u32), border: f64) -> Option<ResizeDirection> {
    let (width, height) = (f64::from(size.0), f64::from(size.1));
    let (west, east) = (x < border, x >= width - border);
    let (north, south) = (y < border, y >= height - border);
    Some(match (north, south, west, east) {
        (true, _, true, _) => ResizeDirection::NorthWest,
        (true, _, _, true) => ResizeDirection::NorthEast,
        (_, true, true, _) => ResizeDirection::SouthWest,
        (_, true, _, true) => ResizeDirection::SouthEast,
        (true, ..) => ResizeDirection::North,
        (_, true, ..) => ResizeDirection::South,
        (.., true, _) => ResizeDirection::West,
        (.., true) => ResizeDirection::East,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const BAR: Bar = Bar {
        height: 34,
        button: 46,
    };

    #[test]
    fn the_bar_grows_with_the_display_s_scale() {
        assert_eq!(Bar::at(1.0), BAR);
        assert_eq!(
            Bar::at(2.0),
            Bar {
                height: 68,
                button: 92
            }
        );
    }

    #[test]
    fn the_menu_button_is_at_the_left() {
        assert_eq!(BAR.part(0.0, 0.0, 800), Some(Part::Menu));
        assert_eq!(BAR.part(45.9, 33.9, 800), Some(Part::Menu));
    }

    #[test]
    fn the_window_buttons_are_at_the_right_close_last() {
        assert_eq!(BAR.part(799.0, 10.0, 800), Some(Part::Close));
        assert_eq!(BAR.part(754.0, 10.0, 800), Some(Part::Close));
        assert_eq!(BAR.part(753.9, 10.0, 800), Some(Part::Maximize));
        assert_eq!(BAR.part(708.0, 10.0, 800), Some(Part::Maximize));
        assert_eq!(BAR.part(707.9, 10.0, 800), Some(Part::Minimize));
        assert_eq!(BAR.part(662.0, 10.0, 800), Some(Part::Minimize));
    }

    #[test]
    fn the_rest_of_the_bar_moves_the_window() {
        assert_eq!(BAR.part(46.0, 10.0, 800), Some(Part::Drag));
        assert_eq!(BAR.part(661.9, 10.0, 800), Some(Part::Drag));
    }

    #[test]
    fn below_the_bar_is_none_of_it() {
        assert_eq!(BAR.part(10.0, 34.0, 800), None);
        assert_eq!(BAR.part(10.0, -1.0, 800), None);
    }

    #[test]
    fn a_point_near_an_edge_resizes_the_window_from_it() {
        let size = (800, 600);
        assert_eq!(edge(1.0, 300.0, size, 6.0), Some(ResizeDirection::West));
        assert_eq!(edge(798.0, 300.0, size, 6.0), Some(ResizeDirection::East));
        assert_eq!(edge(400.0, 1.0, size, 6.0), Some(ResizeDirection::North));
        assert_eq!(edge(400.0, 598.0, size, 6.0), Some(ResizeDirection::South));
        assert_eq!(edge(400.0, 300.0, size, 6.0), None);
    }

    #[test]
    fn a_point_near_a_corner_resizes_the_window_from_it() {
        let size = (800, 600);
        assert_eq!(edge(1.0, 1.0, size, 6.0), Some(ResizeDirection::NorthWest));
        assert_eq!(
            edge(798.0, 1.0, size, 6.0),
            Some(ResizeDirection::NorthEast)
        );
        assert_eq!(
            edge(1.0, 598.0, size, 6.0),
            Some(ResizeDirection::SouthWest)
        );
        assert_eq!(
            edge(798.0, 598.0, size, 6.0),
            Some(ResizeDirection::SouthEast)
        );
    }
}
