//! Pilot: make small language models drive SolidCraft reliably, from outside SolidCraft.
//!
//! With the default `proxy` feature, `model` is an OpenAI-compatible endpoint that sits between
//! SolidCraft's Assistant (or any client) and a local model, and `mcp` is an MCP server that
//! sits between an agent and SolidCraft's own MCP server. See SPEC.md.
//!
//! The `core` feature alone exposes deterministic catalogue lookup, planning, argument
//! repair and state guidance without host I/O. Callers supply catalogue JSON and tool state.

#[cfg(feature = "proxy")]
mod audit;
#[cfg(feature = "core")]
pub mod catalog;
#[cfg(feature = "proxy")]
pub mod cli;
#[cfg(feature = "proxy")]
pub mod eval;
#[cfg(feature = "core")]
pub mod guidance;
pub mod identity;
#[cfg(feature = "proxy")]
pub mod mcp;
#[cfg(feature = "proxy")]
pub mod model;
#[cfg(feature = "core")]
pub mod planner;
#[cfg(feature = "proxy")]
pub mod profile;
#[cfg(feature = "proxy")]
pub mod prompting;
#[cfg(feature = "core")]
pub mod repair;

#[cfg(feature = "proxy")]
pub mod capture;
#[cfg(feature = "proxy")]
pub mod redaction;
#[cfg(feature = "proxy")]
pub mod report;
