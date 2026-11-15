//! Cattery: a GUI terminal app built on the felis terminal daemon.
//!
//! The `cattery` executable is this library's front door; the library is
//! separate so tests can drive a daemon started as the executable starts it.

pub mod about;
pub mod app;
pub mod appearance;
pub mod blur;
pub mod chrome;
pub mod clipboard;
pub mod commands;
pub mod daemon;
pub mod edit;
pub mod folders;
pub mod fonts;
pub mod fuzzy;
pub mod grid;
pub mod icon;
pub mod inserted;
pub mod integration;
pub mod keymap;
pub mod keys;
pub mod links;
pub mod look;
pub mod menu;
pub mod mouse;
pub mod notify;
pub mod opened;
pub mod palette;
pub mod place;
#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub mod popup;
pub mod preferences;
pub mod roster;
pub mod search;
pub mod select;
pub mod session;
pub mod settings;
pub mod socket;
pub mod store;
pub mod surface;
pub mod terminal;
pub mod terminfo;
pub mod themes;
pub mod title;
