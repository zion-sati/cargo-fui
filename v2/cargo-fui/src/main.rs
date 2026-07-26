fn main() {
    let io = cargo_fui::CliIo::stdio();
    if let Err(error) = cargo_fui::run_cli(std::env::args().skip(1), std::env::current_dir(), &io) {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}
