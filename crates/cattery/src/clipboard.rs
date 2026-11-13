//! The system clipboard.

/// Put `text` on the clipboard.
pub fn write(text: &str) {
    if let Err(err) = arboard::Clipboard::new().and_then(|mut board| board.set_text(text)) {
        tracing::warn!(?err, "the clipboard was not written");
    }
}

/// The text on the clipboard, if it holds text.
pub fn read() -> Option<String> {
    arboard::Clipboard::new()
        .and_then(|mut board| board.get_text())
        .inspect_err(|err| tracing::debug!(?err, "the clipboard holds no text"))
        .ok()
}
