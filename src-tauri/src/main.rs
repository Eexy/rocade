// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::env;

fn main() {
    if env::var("__NV_DISABLE_EXPLICIT_SYNC").is_err() {
        env::set_var("__NV_DISABLE_EXPLICIT_SYNC", "1");
    }
    rocade_lib::run()
}
