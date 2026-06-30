// fail-soft fixture (valid): no unwrap, no panic, no todo macro.
//
// This file passes every rule in fail-soft.yaml.

use std::fs;

fn read_config(path: &str) -> Result<String, std::io::Error> {
    fs::read_to_string(path)
}

fn main() {
    match read_config("config.toml") {
        Ok(content) => println!("loaded: {content}"),
        Err(err) => eprintln!("failed to load config: {err}"),
    }
}
