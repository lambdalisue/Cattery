//! The name Cattery's windows go by on the Linux desktop: the Wayland app
//! id and the X11 class. The desktop entry the package installs names it as
//! its `StartupWMClass`, so a launcher and a taskbar group the windows under
//! that entry and show its icon.

use winit::window::WindowAttributes;

/// What every window of Cattery's is named to the Linux desktop. It is the
/// executable's name, so it is also the desktop entry's and the icon's.
pub const APP_ID: &str = "cattery";

/// `attributes` with the window named [`APP_ID`] on Linux. winit names an
/// X11 window after `argv[0]` when it is not told, which a wrapper or a
/// symlink can change, and leaves a Wayland window without an app id.
pub fn named(attributes: WindowAttributes) -> WindowAttributes {
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        use winit::platform::x11::WindowAttributesExtX11 as _;
        // The X11 name is the field Wayland's app id is taken from too.
        attributes.with_name(APP_ID, APP_ID)
    }
    #[cfg(not(all(unix, not(target_os = "macos"))))]
    attributes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_app_id_is_the_executables_name() {
        assert_eq!(APP_ID, env!("CARGO_PKG_NAME"));
    }
}
