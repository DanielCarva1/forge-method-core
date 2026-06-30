// fail-soft fixture (invalid): contains every anti-pattern fail-soft.yaml flags.
//
// This file is expected to trigger:
//   - rust-no-unwrap   (line ~8: `.unwrap()`)
//   - rust-no-todo-macro (line ~11: `todo!()`)
//   - rust-no-panic-in-product (line ~14: `panic!()`)

use std::fs;

fn read_config_or_die(path: &str) -> String {
    fs::read_to_string(path).unwrap()
}

fn not_implemented_yet() -> i32 {
    todo!()
}

fn die() -> ! {
    panic!("unrecoverable")
}

fn main() {
    let cfg = read_config_or_die("config.toml");
    println!("{cfg}");
    let _ = not_implemented_yet();
    if cfg.is_empty() {
        die();
    }
}
