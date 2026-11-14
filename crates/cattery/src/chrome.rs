//! What a window draws around its grid: on macOS, the title bar it is
//! drawn under.

use winit::window::{Window, WindowAttributes};

/// `attributes` with the window's content drawn under its title bar, where
/// the title bar can be made see-through, so that the title bar shows the
/// theme's background, translucent as the window is.
pub fn under_title_bar(attributes: WindowAttributes) -> WindowAttributes {
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::WindowAttributesExtMacOS as _;
        attributes
            .with_titlebar_transparent(true)
            .with_fullsize_content_view(true)
    }
    #[cfg(not(target_os = "macos"))]
    attributes
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
    {
        let _ = window;
        0
    }
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
