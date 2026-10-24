//! Pure Windows-1252 framing with allocation limits.
use std::io::{Read, Write};
pub const COMMAND: usize = 4096;
pub const ANSWER: usize = 262144;
const HIGH: &str = "€�‚ƒ„…†‡ˆ‰Š‹Œ�Ž��‘’“”•–—˜™š›œ�žŸ";
pub fn encode(s: &str) -> Result<Vec<u8>, String> {
    if s.len() > COMMAND || s.contains(['\r', '\n', '\0']) {
        return Err("command exceeds 4 KiB or contains CR, LF, or NUL".into());
    }
    s.chars()
        .map(|c| {
            let n = c as u32;
            if n < 128 || (160..=255).contains(&n) {
                Ok(n as u8)
            } else {
                HIGH.chars()
                    .position(|x| x == c && c != '\u{fffd}')
                    .map(|i| 128 + i as u8)
                    .ok_or_else(|| format!("text is not Windows-1252: {c}"))
            }
        })
        .collect()
}
pub fn decode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| {
            if (128..160).contains(b) {
                HIGH.chars()
                    .nth(usize::from(*b - 128))
                    .unwrap_or('\u{fffd}')
            } else {
                char::from(*b)
            }
        })
        .collect()
}
pub fn write(out: &mut impl Write, text: &str) -> Result<(), String> {
    let mut bytes = encode(text)?;
    bytes.push(b'\n');
    out.write_all(&bytes)
        .and_then(|()| out.flush())
        .map_err(|e| format!("remote write failed: {e}"))
}
pub fn read(input: &mut impl Read) -> Result<String, String> {
    let mut answer = Vec::new();
    for _ in 0..4096 {
        let mut line = Vec::new();
        loop {
            let mut b = [0];
            input
                .read_exact(&mut b)
                .map_err(|e| format!("remote answer incomplete: {e}"))?;
            if b[0] == b'\n' {
                break;
            }
            if line.len() >= 8192 {
                return Err("remote answer line exceeds 8 KiB".into());
            }
            line.push(b[0]);
        }
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        if line == b"." {
            return Ok(decode(&answer));
        }
        if line.starts_with(b"..") {
            line.remove(0);
        }
        if answer.len() + line.len() + 1 > ANSWER {
            return Err("remote answer exceeds 256 KiB".into());
        }
        answer.extend(line);
        answer.push(b'\n');
    }
    Err("remote answer exceeds 4096 lines or lacks terminator".into())
}
