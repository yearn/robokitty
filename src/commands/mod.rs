// src/commands/mod.rs
pub mod common;
pub mod cli;
pub mod telegram;

pub use common::{Command, CommandExecutor};