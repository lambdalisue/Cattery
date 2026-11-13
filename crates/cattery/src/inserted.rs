//! Text an input method inserts for a keystroke without composing it
//! first, as a kana input method does in its direct mode.
//!
//! winit's macOS view reports `insertText:` as an input method's commit
//! only after text was being composed, and otherwise sends the key itself:
//! typing あ in a direct kana mode reaches the window as "a". Until winit
//! reports such text, its view's `insertText:` and `keyDown:` are wrapped
//! here so the window can tell what was inserted for the key it receives.
//! Other platforms report no such text.

/// Start remembering what is inserted into `window`'s view.
pub fn watch(window: &winit::window::Window) {
    #[cfg(target_os = "macos")]
    macos::watch(window);
    #[cfg(not(target_os = "macos"))]
    let _ = window;
}

/// The text an input method inserted, without composing it, during the
/// keystroke being handled.
pub fn take() -> Option<String> {
    #[cfg(target_os = "macos")]
    return macos::take();
    #[cfg(not(target_os = "macos"))]
    None
}

#[cfg(target_os = "macos")]
mod macos {
    use std::cell::RefCell;
    use std::sync::{Once, OnceLock};

    use objc2::msg_send;
    use objc2::runtime::{AnyObject, Imp, Sel};
    use objc2::sel;
    use objc2_foundation::{NSAttributedString, NSRange, NSString};
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    thread_local! {
        /// AppKit calls both on the main thread, as winit handles events.
        static INSERTED: RefCell<Option<String>> = const { RefCell::new(None) };
    }

    static INSERT_TEXT: OnceLock<Imp> = OnceLock::new();
    static KEY_DOWN: OnceLock<Imp> = OnceLock::new();

    type InsertText = unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject, NSRange);
    type KeyDown = unsafe extern "C-unwind" fn(&AnyObject, Sel, &AnyObject);

    pub fn watch(window: &winit::window::Window) {
        static WRAPPED: Once = Once::new();
        let Ok(handle) = window.window_handle() else {
            return;
        };
        let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
            return;
        };
        // SAFETY: winit hands out the pointer to its live view.
        let view: &AnyObject = unsafe { handle.ns_view.cast().as_ref() };
        // Every window's view is of winit's one view class, so wrapping it
        // once covers every window.
        WRAPPED.call_once(|| {
            let class = view.class();
            if let (Some(insert_text), Some(key_down)) = (
                class.instance_method(sel!(insertText:replacementRange:)),
                class.instance_method(sel!(keyDown:)),
            ) {
                // SAFETY: each replacement takes the arguments of the method
                // it replaces and calls the original with them.
                unsafe {
                    let original =
                        insert_text.set_implementation(std::mem::transmute::<InsertText, Imp>(
                            wrapped_insert_text,
                        ));
                    _ = INSERT_TEXT.set(original);
                    let original = key_down
                        .set_implementation(std::mem::transmute::<KeyDown, Imp>(wrapped_key_down));
                    _ = KEY_DOWN.set(original);
                }
            }
        });
    }

    pub fn take() -> Option<String> {
        INSERTED.with_borrow_mut(Option::take)
    }

    unsafe extern "C-unwind" fn wrapped_key_down(this: &AnyObject, cmd: Sel, event: &AnyObject) {
        // Text inserted outside a keystroke, as from the Character Viewer,
        // is not this keystroke's.
        INSERTED.with_borrow_mut(|inserted| *inserted = None);
        if let Some(original) = KEY_DOWN.get() {
            // SAFETY: `original` is the implementation this one replaced.
            unsafe {
                std::mem::transmute::<Imp, KeyDown>(*original)(this, cmd, event);
            }
        }
    }

    unsafe extern "C-unwind" fn wrapped_insert_text(
        this: &AnyObject,
        cmd: Sel,
        text: &AnyObject,
        range: NSRange,
    ) {
        // SAFETY: the view is an `NSTextInputClient`.
        let composing: bool = unsafe { msg_send![this, hasMarkedText] };
        // Text inserted while composing is a commit winit reports itself.
        if !composing {
            let text = text
                .downcast_ref::<NSAttributedString>()
                .map(|text| text.string().to_string())
                .or_else(|| text.downcast_ref::<NSString>().map(ToString::to_string));
            INSERTED.with_borrow_mut(|inserted| *inserted = text);
        }
        if let Some(original) = INSERT_TEXT.get() {
            // SAFETY: `original` is the implementation this one replaced.
            unsafe {
                std::mem::transmute::<Imp, InsertText>(*original)(this, cmd, text, range);
            }
        }
    }
}
