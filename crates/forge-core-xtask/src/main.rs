fn main() {
    std::process::exit(forge_core_xtask::run_cli(std::env::args().skip(1)));
}
