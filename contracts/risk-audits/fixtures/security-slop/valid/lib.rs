// security-slop fixture (valid): no hardcoded secrets, no disabled TLS,
// no empty expect, no TODO in security-sensitive paths.
//
// This file passes every rule in security-slop.yaml.

use std::env;

fn load_db_password() -> Result<String, env::VarError> {
    env::var("DB_PASSWORD")
}

fn main() {
    let pwd = load_db_password().expect("DB_PASSWORD must be set in env");
    println!("connected with {} chars", pwd.len());
}
