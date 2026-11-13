//! Cattery: a GUI terminal app built on the felis terminal daemon.
//!
//! The `cattery` executable is this library's front door; the library is
//! separate so tests can drive a daemon started as the executable starts it.

pub mod about;
pub mod app;
pub mod appearance;
pub mod daemon;
pub mod edit;
pub mod grid;
pub mod icon;
pub mod inserted;
pub mod keys;
pub mod menu;
pub mod mouse;
pub mod place;
pub mod preferences;
pub mod session;
pub mod settings;
pub mod socket;
pub mod store;
pub mod terminal;
pub mod terminfo;
pub mod themes;
pub mod title;
