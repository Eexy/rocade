// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::env;

fn main() {
    if env::var("WEBKIT_DISABLE_DMABUF_RENDERER").is_err() {
        env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
    rocade_lib::run()
}
