//! Desktop and web entry point.
//!
//! Deliberately thin: everything lives in the library so that integration tests
//! and the iOS static library drive exactly the same app definition.

// No console window on a Windows release build, and none exists on the web.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    holdfast::run();
}
