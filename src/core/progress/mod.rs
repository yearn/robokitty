//! Progress tracking for long-running operations
//! 
//! This module contains types and traits for tracking progress of
//! operations that may take multiple steps or require waiting.

pub mod raffle;
pub use raffle::RaffleProgress;