// exception-swallowing fixture (invalid).
//
// Expected behaviour: this file triggers the rules defined in the
// matching policy because it contains the anti-patterns they look for.
// See the policy yaml for the rule ids and the lines below for the
// offending call sites.

use std::fs;

fn read_config(path: &str) -> Result<String, std::io::Error> {
    fs::read_to_string(path)
}

fn main() {
    let _ = read_config("ignored.toml");
    let _maybe: Option<String> = read_config("maybe.toml").ok();
    println!("done");
}
