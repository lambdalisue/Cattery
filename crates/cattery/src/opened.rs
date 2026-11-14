//! What macOS hands Cattery to open: a folder dropped on its icon in the
//! Dock, opened with it from Finder, named to `open -a Cattery`, or chosen
//! for New Cattery Window Here in Finder's context menu.
//!
//! AppKit hands it to the application's delegate and to its services
//! provider, which winit does not answer and offers no way to answer, so
//! the methods are added to winit's delegate's class here, and the delegate
//! is made the provider. A folder Cattery was launched to open comes before
//! the event loop starts, so what is handed is kept until the app takes it,
//! and the app is told only that there is something to take. Other
//! platforms hand nothing.

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

/// Whether Cattery was started only to run, rather than to open something
/// or for a service from another app, which hands what to open once
/// Cattery is up. Always so but on macOS.
pub fn launched_by_default() -> bool {
    #[cfg(target_os = "macos")]
    return macos::launched_by_default();
    #[cfg(not(target_os = "macos"))]
    true
}

/// What Cattery does as it starts.
#[derive(Debug, PartialEq, Eq)]
pub enum First {
    /// Open a window in each of these folders, where Cattery was for none.
    Open(Vec<Option<PathBuf>>),
    /// Open nothing yet: what to open is still to be handed.
    Await,
}

/// What Cattery does as it starts, `handed` these folders, started
/// `by_default` or not, and `given` a folder on its command line.
pub fn first(handed: Vec<PathBuf>, by_default: bool, given: Option<PathBuf>) -> First {
    if !handed.is_empty() {
        First::Open(handed.into_iter().map(Some).collect())
    } else if by_default {
        First::Open(vec![given])
    } else {
        First::Await
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_handed_at_launch_take_the_first_window_s_place() {
        let handed = vec![PathBuf::from("/a"), PathBuf::from("/b")];
        assert_eq!(
            first(handed, false, Some(PathBuf::from("/given"))),
            First::Open(vec![Some(PathBuf::from("/a")), Some(PathBuf::from("/b"))])
        );
    }

    #[test]
    fn a_plain_launch_opens_the_first_window_where_it_was_asked() {
        assert_eq!(
            first(Vec::new(), true, Some(PathBuf::from("/given"))),
            First::Open(vec![Some(PathBuf::from("/given"))])
        );
        assert_eq!(first(Vec::new(), true, None), First::Open(vec![None]));
    }

    #[test]
    fn a_launch_for_what_is_handed_later_waits_for_it() {
        assert_eq!(first(Vec::new(), false, None), First::Await);
    }
}

#[cfg(target_os = "macos")]
mod macos {
    use std::ffi::CStr;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Mutex, OnceLock};

    use objc2::rc::Retained;
    use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
    use objc2::{class, msg_send, sel};
    use objc2_foundation::{NSArray, NSString, NSURL};
    use winit::event_loop::EventLoopProxy;

    use crate::app::Event;

    type OpenUrls = unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject, &NSArray<NSURL>);
    type OpenFolder =
        unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject, *mut AnyObject, *mut AnyObject);
    type FinishLaunching = unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject);

    static PROXY: OnceLock<Mutex<EventLoopProxy<Event>>> = OnceLock::new();
    static FINISH_LAUNCHING: OnceLock<Imp> = OnceLock::new();
    static DEFAULT_LAUNCH: AtomicBool = AtomicBool::new(true);

    pub fn launched_by_default() -> bool {
        DEFAULT_LAUNCH.load(Ordering::Relaxed)
    }

    /// winit's `applicationDidFinishLaunching:`, after noting what AppKit
    /// says of the launch: winit starts the app in it, and keeps nothing of
    /// what it is told.
    unsafe extern "C-unwind" fn finish_launching(
        this: &AnyObject,
        cmd: Sel,
        notification: &AnyObject,
    ) {
        // SAFETY: the notification's user info is a dictionary or none, and
        // its value for the key a number.
        unsafe {
            let info: *mut AnyObject = msg_send![notification, userInfo];
            if let Some(info) = info.as_ref() {
                let key = NSString::from_str("NSApplicationLaunchIsDefaultLaunchKey");
                let value: *mut AnyObject = msg_send![info, objectForKey: &*key];
                if let Some(value) = value.as_ref() {
                    let default: bool = msg_send![value, boolValue];
                    DEFAULT_LAUNCH.store(default, Ordering::Relaxed);
                }
            }
        }
        if let Some(original) = FINISH_LAUNCHING.get() {
            // SAFETY: `original` is the implementation this one replaced.
            unsafe {
                std::mem::transmute::<Imp, FinishLaunching>(*original)(this, cmd, notification);
            }
        }
    }

    fn handed(urls: &NSArray<NSURL>) {
        super::hand(urls.iter().filter_map(|url| url.to_file_path()));
        if let Some(proxy) = PROXY.get().and_then(|proxy| proxy.lock().ok()) {
            _ = proxy.send_event(Event::Opened);
        }
    }

    unsafe extern "C-unwind" fn open_urls(
        _: &AnyObject,
        _: Sel,
        _: &AnyObject,
        urls: &NSArray<NSURL>,
    ) {
        handed(urls);
    }

    /// The service Finder's New Cattery Window Here calls, with the folders
    /// chosen on `pasteboard`.
    unsafe extern "C-unwind" fn open_folder(
        _: &AnyObject,
        _: Sel,
        pasteboard: &AnyObject,
        _: *mut AnyObject,
        _: *mut AnyObject,
    ) {
        // SAFETY: a pasteboard reads the file URLs on it as NSURLs, given
        // the class of NSURL; the array holds that one class.
        let urls: Option<Retained<NSArray<NSURL>>> = unsafe {
            let url: *const AnyObject = (class!(NSURL) as *const AnyClass).cast();
            let classes: Retained<AnyObject> = msg_send![class!(NSArray), arrayWithObject: url];
            msg_send![pasteboard, readObjectsForClasses: &*classes, options: std::ptr::null::<AnyObject>()]
        };
        if let Some(urls) = urls {
            handed(&urls);
        }
    }

    pub fn listen(proxy: EventLoopProxy<Event>) {
        _ = PROXY.set(Mutex::new(proxy));
        // SAFETY: AppKit's application and its delegate live as long as the
        // app.
        let (app, delegate): (*mut AnyObject, *mut AnyObject) = unsafe {
            let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
            (app, msg_send![app, delegate])
        };
        // SAFETY: a delegate AppKit hands out is a live object, or none.
        let Some(delegate) = (unsafe { delegate.as_ref() }) else {
            tracing::warn!("nothing handed to Cattery is opened: the app has no delegate");
            return;
        };
        let class: *const AnyClass = delegate.class();
        if let Some(method) = delegate
            .class()
            .instance_method(sel!(applicationDidFinishLaunching:))
        {
            // SAFETY: the replacement takes what the method it replaces does
            // and calls it with them.
            let original = unsafe {
                method.set_implementation(std::mem::transmute::<FinishLaunching, Imp>(
                    finish_launching,
                ))
            };
            _ = FINISH_LAUNCHING.set(original);
        }
        let methods: [(Sel, Imp, &CStr); 2] = [
            (
                sel!(application:openURLs:),
                // SAFETY: the function takes what the selector does.
                unsafe { std::mem::transmute::<OpenUrls, Imp>(open_urls) },
                c"v@:@@",
            ),
            (
                // The name ends the `NSMessage` of the service in Info.plist.
                sel!(openFolder:userData:error:),
                // SAFETY: the function takes what the selector does.
                unsafe { std::mem::transmute::<OpenFolder, Imp>(open_folder) },
                c"v@:@@^@",
            ),
        ];
        for (name, imp, types) in methods {
            // SAFETY: `imp` takes what `types` says, which is what AppKit
            // calls `name` with.
            let added =
                unsafe { objc2::ffi::class_addMethod(class.cast_mut(), name, imp, types.as_ptr()) };
            if !added.as_bool() {
                tracing::warn!(?name, "the delegate already answers; it is left as it is");
            }
        }
        // SAFETY: the provider, winit's delegate, lives as long as the app.
        unsafe {
            let _: () = msg_send![app, setServicesProvider: delegate];
        }
    }
}
