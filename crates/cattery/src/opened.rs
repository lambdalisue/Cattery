//! What macOS hands Cattery to open: a folder dropped on its icon in the
//! Dock, opened with it from Finder, or named to `open -a Cattery`.
//!
//! AppKit hands it to the application's delegate, which winit's does not
//! answer and offers no way to answer, so the method is added to its class
//! here. A folder Cattery was launched to open comes before the event loop
//! starts, so what is handed is kept until the app takes it, and the app is
//! told only that there is something to take. Other platforms hand nothing.

use std::path::PathBuf;
use std::sync::Mutex;

use winit::event_loop::EventLoopProxy;

use crate::app::Event;

static HANDED: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

/// Start keeping what is handed to Cattery, telling `proxy` each time.
/// Called once the event loop is made, before it runs.
pub fn listen(proxy: EventLoopProxy<Event>) {
    #[cfg(target_os = "macos")]
    macos::listen(proxy);
    #[cfg(not(target_os = "macos"))]
    let _ = proxy;
}

/// What was handed since it was last taken, in the order it came.
pub fn take() -> Vec<PathBuf> {
    HANDED
        .lock()
        .map(|mut handed| std::mem::take(&mut *handed))
        .unwrap_or_default()
}

/// Keep `paths` as handed.
fn hand(paths: impl IntoIterator<Item = PathBuf>) {
    if let Ok(mut handed) = HANDED.lock() {
        handed.extend(paths);
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::sync::{Mutex, OnceLock};

    use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
    use objc2::{class, msg_send, sel};
    use objc2_foundation::{NSArray, NSURL};
    use winit::event_loop::EventLoopProxy;

    use crate::app::Event;

    type OpenUrls = unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject, &NSArray<NSURL>);

    static PROXY: OnceLock<Mutex<EventLoopProxy<Event>>> = OnceLock::new();

    unsafe extern "C-unwind" fn open_urls(
        _: &AnyObject,
        _: Sel,
        _: &AnyObject,
        urls: &NSArray<NSURL>,
    ) {
        super::hand(urls.iter().filter_map(|url| url.to_file_path()));
        if let Some(proxy) = PROXY.get().and_then(|proxy| proxy.lock().ok()) {
            _ = proxy.send_event(Event::Opened);
        }
    }

    pub fn listen(proxy: EventLoopProxy<Event>) {
        _ = PROXY.set(Mutex::new(proxy));
        // SAFETY: AppKit's application and its delegate live as long as the
        // app.
        let delegate: *mut AnyObject = unsafe {
            let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
            msg_send![app, delegate]
        };
        // SAFETY: a delegate AppKit hands out is a live object, or none.
        let Some(delegate) = (unsafe { delegate.as_ref() }) else {
            tracing::warn!("nothing handed to Cattery is opened: the app has no delegate");
            return;
        };
        let class: *const AnyClass = delegate.class();
        // SAFETY: `open_urls` takes what `application:openURLs:` does, as
        // its type encoding says.
        let added = unsafe {
            objc2::ffi::class_addMethod(
                class.cast_mut(),
                sel!(application:openURLs:),
                std::mem::transmute::<OpenUrls, Imp>(open_urls),
                c"v@:@@".as_ptr(),
            )
        };
        if !added.as_bool() {
            tracing::warn!("nothing handed to Cattery is opened: the delegate already opens it");
        }
    }
}
