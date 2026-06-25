use forge_core_schema::{compact_agent_views, generated_contract_schemas};
use std::env;

fn main() {
    let mode = env::args().nth(1).unwrap_or_else(|| "views".to_owned());
    let result = match mode.as_str() {
        "schemas" => serde_json::to_string_pretty(&generated_contract_schemas()),
        "views" => serde_json::to_string_pretty(&compact_agent_views()),
        _ => {
            eprintln!("usage: forge-core-schema [schemas|views]");
            std::process::exit(2);
        }
    };

    match result {
        Ok(text) => println!("{text}"),
        Err(error) => {
            eprintln!("failed to serialize generated schema output: {error}");
            std::process::exit(1);
        }
    }
}
