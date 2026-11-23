//! Cattery's icon: its windows', and on macOS the Dock's where no bundle
//! gives it. It is drawn light and dark, to go with the theme in force.

use anyhow::Context as _;
use winit::window::Theme;

/// The icon as the Dock shows it, as `scripts/icon.sh` renders it from
/// `assets/logo`.
pub const fn large(look: Theme) -> &'static [u8] {
    match look {
        Theme::Light => include_bytes!("../../../assets/logo/cattery-icon-light-512.png"),
        Theme::Dark => include_bytes!("../../../assets/logo/cattery-icon-dark-512.png"),
    }
}

/// The icon for a title bar or a taskbar. It is a drawing of its own
/// rather than the large one scaled down: at a few dozen pixels the large
/// one's strokes are too thin to see.
pub const fn small(look: Theme) -> &'static [u8] {
    match look {
        Theme::Light => include_bytes!("../../../assets/logo/cattery-icon-light-64.png"),
        Theme::Dark => include_bytes!("../../../assets/logo/cattery-icon-dark-64.png"),
    }
}

/// The icon's pixels, as 8-bit RGBA rows, with its width and height.
pub fn rgba(png: &[u8]) -> anyhow::Result<(Vec<u8>, u32, u32)> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(png));
    decoder.set_transformations(
        png::Transformations::EXPAND | png::Transformations::ALPHA | png::Transformations::STRIP_16,
    );
    let mut reader = decoder.read_info()?;
    let mut pixels = vec![
        0;
        reader
            .output_buffer_size()
            .context("the picture is too large")?
    ];
    let frame = reader.next_frame(&mut pixels)?;
    if frame.color_type != png::ColorType::Rgba {
        anyhow::bail!("the picture is {:?}, not RGBA", frame.color_type);
    }
    pixels.truncate(frame.buffer_size());
    Ok((pixels, frame.width, frame.height))
}

/// The icon for a window's title bar and the taskbar, where the platform
/// shows one there.
pub fn window(look: Theme) -> Option<winit::window::Icon> {
    let (pixels, width, height) = rgba(small(look))
        .inspect_err(|err| tracing::warn!(?err, "the icon was not read"))
        .ok()?;
    winit::window::Icon::from_rgba(pixels, width, height)
        .inspect_err(|err| tracing::warn!(?err, "the icon was not made"))
        .ok()
}

/// Show the icon in the Dock when Cattery runs outside its bundle, where
/// the Dock would show a generic one. A bundle's icon is left to macOS,
/// which draws it in the icon style the user chose: one set here would
/// override that for as long as Cattery runs.
pub fn show_in_dock(look: Theme) {
    #[cfg(target_os = "macos")]
    {
        if bundled() {
            return;
        }
        use objc2::AllocAnyThread as _;
        use objc2::MainThreadMarker;
        use objc2_app_kit::{NSApplication, NSImage};
        use objc2_foundation::NSData;

        let Some(main) = MainThreadMarker::new() else {
            return;
        };
        let data = NSData::with_bytes(large(look));
        if let Some(image) = NSImage::initWithData(NSImage::alloc(), &data) {
            // SAFETY: called on the main thread, which the marker proves.
            unsafe {
                NSApplication::sharedApplication(main).setApplicationIconImage(Some(&image));
            }
        }
    }
    #[cfg(not(target_os = "macos"))]
    let _ = look;
}

/// Whether Cattery runs as an app, from its bundle. `NSBundle` is not
/// asked: run by a link on `PATH` it finds no bundle, while the Dock
/// follows the link and shows the app.
#[cfg(target_os = "macos")]
fn bundled() -> bool {
    std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .is_ok_and(|executable| in_bundle(&executable))
}

/// Whether `executable`, with no link left in it, is an app bundle's.
#[cfg(target_os = "macos")]
fn in_bundle(executable: &std::path::Path) -> bool {
    executable.parent().is_some_and(|folder| {
        folder.ends_with("Contents/MacOS")
            && folder
                .ancestors()
                .nth(2)
                .and_then(|app| app.extension())
                .is_some_and(|extension| extension == "app")
    })
}

/// Have the menu bar and the Dock name the app Cattery when it runs outside
/// its bundle, as it does as it is built, where macOS names it after its
/// executable. Call it before the app starts, when AppKit reads the name.
#[cfg(target_os = "macos")]
pub fn name_outside_bundle() {
    use objc2::runtime::AnyObject;
    use objc2::{class, msg_send, sel};
    use objc2_foundation::NSString;

    // SAFETY: `mainBundle` and `infoDictionary` are getters that take
    // nothing; the dictionary is written only where it answers
    // `setObject:forKey:`, which a bundle's own, read from its Info.plist,
    // does not.
    unsafe {
        let bundle: *mut AnyObject = msg_send![class!(NSBundle), mainBundle];
        if bundle.is_null() {
            return;
        }
        let info: *mut AnyObject = msg_send![bundle, infoDictionary];
        let Some(info) = info.as_ref() else {
            return;
        };
        let writable: bool = msg_send![info, respondsToSelector: sel!(setObject:forKey:)];
        if !writable {
            return;
        }
        let name = NSString::from_str("Cattery");
        let key = NSString::from_str("CFBundleName");
        let _: () = msg_send![info, setObject: &*name, forKey: &*key];
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOOKS: [Theme; 2] = [Theme::Light, Theme::Dark];

    /// The red, green, blue and alpha at `x`, `y` of the picture `png`.
    fn pixel(png: &[u8], x: usize, y: usize) -> [u8; 4] {
        let (pixels, width, _) = rgba(png).unwrap();
        let at = (y * width as usize + x) * 4;
        [pixels[at], pixels[at + 1], pixels[at + 2], pixels[at + 3]]
    }

    #[test]
    fn each_look_s_pictures_are_square_and_have_every_pixel() {
        for look in LOOKS {
            for (png, side) in [(large(look), 512), (small(look), 64)] {
                let (pixels, width, height) = rgba(png).unwrap();
                assert_eq!((width, height), (side, side), "{look:?}");
                assert_eq!(pixels.len(), (side * side * 4) as usize, "{look:?}");
            }
        }
    }

    #[test]
    fn the_pictures_corners_show_through() {
        for look in LOOKS {
            assert_eq!(pixel(large(look), 0, 0)[3], 0, "{look:?}");
            assert_eq!(pixel(large(look), 256, 256)[3], 255, "{look:?}");
            assert_eq!(pixel(small(look), 0, 0)[3], 0, "{look:?}");
            assert_eq!(pixel(small(look), 32, 32)[3], 255, "{look:?}");
        }
    }

    #[test]
    fn the_light_look_has_a_light_ground_and_the_dark_a_dark_one() {
        // Above the prompt, where nothing is drawn on the ground.
        for (png, x, y) in [
            (large as fn(Theme) -> &'static [u8], 128, 128),
            (small, 16, 16),
        ] {
            let [red, ..] = pixel(png(Theme::Light), x, y);
            assert!(red > 200, "the light ground is {red}");
            let [red, ..] = pixel(png(Theme::Dark), x, y);
            assert!(red < 80, "the dark ground is {red}");
        }
    }

    #[test]
    fn the_small_picture_fills_more_of_its_square() {
        // Halfway down the left edge, a sixteenth of the way in: the margin
        // of the large picture, the ground of the small one.
        for look in LOOKS {
            assert_eq!(pixel(large(look), 32, 256)[3], 0, "{look:?}");
            assert_eq!(pixel(small(look), 4, 32)[3], 255, "{look:?}");
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn an_app_s_executable_is_in_its_bundle_and_a_bare_one_is_in_none() {
        for (executable, expected) in [
            ("/Applications/Cattery.app/Contents/MacOS/cattery", true),
            ("/opt/homebrew/bin/cattery", false),
            ("/src/cattery/target/debug/cattery", false),
            ("/src/Cattery.app/target/debug/cattery", false),
            ("/src/Contents/MacOS/cattery", false),
        ] {
            assert_eq!(
                in_bundle(std::path::Path::new(executable)),
                expected,
                "{executable}"
            );
        }
        assert!(!bundled());
    }

    #[test]
    fn what_is_not_a_png_is_refused() {
        assert!(rgba(b"not a picture").is_err());
    }
}
