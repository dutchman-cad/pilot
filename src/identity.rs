//! Product identity. Executable and storage names follow Cargo's package identity.
pub const DISPLAY_NAME: &str = "SolidCraft-Pilot";
pub const BINARY: &str = env!("CARGO_PKG_NAME");
pub const EVAL_BINARY: &str = concat!(env!("CARGO_PKG_NAME"), "-eval");

// These markers are part of the public protocol; preserve them across renames.
pub const HINT: &str = "[dutchman-pilot hint]";
pub const PROMPT_HELP: &str = "[dutchman-pilot prompt help]";
pub const RESPONSE_HEADER: &str = "x-dutchman-pilot";
pub const HAND_BACKS_HEADER: &str = "x-dutchman-pilot-hand-backs";
pub const PROBE_PREFIX: &str = "dutchman-pilot/probe/";
pub const RECOVERED_PREFIX: &str = "dutchman-pilot-recovered-";
