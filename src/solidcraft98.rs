//! Bounded screen protocol for the separate SolidCraft 98 application.
pub mod guidance;
#[cfg(feature = "proxy")]
pub mod mcp;
pub mod protocol;
pub mod screen;
#[cfg(feature = "proxy")]
pub mod transport;
