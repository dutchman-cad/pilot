//! Version-only installer fixtures; compiled directly by the Windows shell test.
fn main() {
    if cfg!(full_dutchman) {
        println!("dutchman 0.9.0");
    } else if cfg!(old_launcher) {
        println!("dutchman launcher 0.0.1");
    } else {
        println!("dutchman launcher 1.0.0");
    }
}
