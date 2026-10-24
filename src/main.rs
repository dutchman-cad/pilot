use clap::Parser;

fn main() {
    let cmd = dutchman_pilot::cli::Command::parse();
    let code = match dutchman_pilot::cli::run(cmd) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{}: {error}", dutchman_pilot::identity::BINARY);
            1
        }
    };
    std::process::exit(code);
}
