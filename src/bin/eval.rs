use dutchman_pilot::eval::{run, Options};
use dutchman_pilot::identity::EVAL_BINARY;
fn main() {
    let args: Vec<String> = std::env::args().skip(1).take(33).collect();
    if args.len() > 32 || args.iter().any(|a| a.len() > 4096) {
        eprintln!("{EVAL_BINARY}: at most 32 arguments, each at most 4096 bytes");
        std::process::exit(2);
    }
    if args.iter().any(|s| s == "--help" || s == "-h") {
        print!("{}", dutchman_pilot::eval::help());
        return;
    }
    match Options::parse(args.into_iter()).and_then(run) {
        Ok(true) => {}
        Ok(false) => std::process::exit(1),
        Err(error) => {
            eprintln!("{EVAL_BINARY}: {error}");
            std::process::exit(2);
        }
    }
}
