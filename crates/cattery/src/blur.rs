//! The blur of what shows through a translucent window.

use winit::window::Window;

/// The radius a window is blurred by: what the appearance asks, while the
/// window is translucent; an opaque window shows nothing to blur.
pub fn radius(asked: u32, translucent: bool) -> u32 {
    if translucent { asked } else { 0 }
}

/// Blur what shows through `window` by `radius` points, `0` for none.
///
/// macOS's window server blurs behind a window by any radius; elsewhere
/// winit can only turn the compositor's blur on or off, where the
/// compositor offers one.
pub fn apply(window: &Window, radius: u32) {
    #[cfg(target_os = "macos")]
    macos::apply(window, radius);
    #[cfg(not(target_os = "macos"))]
    window.set_blur(radius > 0);
}

#[cfg(target_os = "macos")]
mod macos {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send};
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use winit::window::Window;

    // Private, as winit's own blur is: AppKit has no public call that
    // blurs behind a window by a radius.
    #[link(name = "CoreGraphics", kind = "framework")]
    unsafe extern "C" {
        fn CGSMainConnectionID() -> *mut AnyObject;
        fn CGSSetWindowBackgroundBlurRadius(
            connection: *mut AnyObject,
            window: isize,
            radius: i64,
        ) -> i32;
    }

    pub fn apply(window: &Window, radius: u32) {
        let Ok(handle) = window.window_handle() else {
            return;
        };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return;
        };
        // SAFETY: winit hands out the pointer to its live view, and calls
        // reach here on the main thread, as AppKit requires; `window`,
        // `windowNumber` and NSColor's constructors take what is given, and
        // `setBackgroundColor:` takes an NSColor.
        let number: isize = unsafe {
            let view: &AnyObject = handle.ns_view.cast().as_ref();
            let window: *mut AnyObject = msg_send![view, window];
            if window.is_null() {
                return;
            }
            // A clear window background leaves the window server blurring
            // the window's whole square frame, past its rounded corners; a
            // background all but clear keeps the blur to the window's shape, as
            // Ghostty keeps it.
            let background: *mut AnyObject = if radius > 0 {
                msg_send![class!(NSColor), colorWithWhite: 1.0f64, alpha: 0.001f64]
            } else {
                msg_send![class!(NSColor), clearColor]
            };
            let _: () = msg_send![window, setBackgroundColor: background];
            msg_send![window, windowNumber]
        };
        // SAFETY: the connection is this process's own, and the number
        // names one of its windows.
        let failed = unsafe {
            CGSSetWindowBackgroundBlurRadius(CGSMainConnectionID(), number, i64::from(radius))
        };
        if failed != 0 {
            tracing::warn!(failed, radius, "the window was not blurred");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_translucent_window_is_blurred_as_asked() {
        assert_eq!(radius(20, true), 20);
    }

    #[test]
    fn an_opaque_window_has_nothing_to_blur() {
        assert_eq!(radius(20, false), 0);
    }
}
