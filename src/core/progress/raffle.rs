use uuid::Uuid;
use std::error::Error;

#[derive(Debug, Clone)]
pub enum RaffleProgress {
    Preparing {
        proposal_name: String,
        ticket_ranges: Vec<(String, u64, u64)> // team_name, start, end
    },
    WaitingForBlock {
        current: u64,
        target: u64
    },
    RandomnessAcquired {
        block_hash: String
    },
    ReadyToFinalize {
        raffle_id: Uuid,
        proposal_name: String,
        current_block: u64,
        target_block: u64,
        randomness: String
    },
    Completed {
        raffle_id: Uuid,
        proposal_name: String,
        counted: Vec<String>,   // team names
        uncounted: Vec<String>  // team names
    }
}

impl RaffleProgress {
    /// Formats the progress update as a human-readable message suitable for display
    pub fn format_message(&self) -> String {
        match self {
            RaffleProgress::Preparing { proposal_name, ticket_ranges } => {
                let mut msg = format!("Preparing raffle for proposal: {}\n\n", proposal_name);
                for (team_name, start, end) in ticket_ranges {
                    msg.push_str(&format!("  {} ballot range [{}..{}]\n", team_name, start, end));
                }
                msg
            },
            RaffleProgress::WaitingForBlock { current, target } => {
                format!("Current block: {}\nTarget block: {}\nWaiting for target block...", current, target)
            },
            RaffleProgress::RandomnessAcquired { block_hash } => {
                format!("Block randomness acquired: {}", block_hash)
            },
            RaffleProgress::ReadyToFinalize { proposal_name, current_block, target_block, .. } => {
                format!(
                    "Ready to finalize raffle for '{}'\nUsing randomness from block {} (initiated at block {})",
                    proposal_name, target_block, current_block
                )
            },
            RaffleProgress::Completed { proposal_name, counted, uncounted, .. } => {
                let mut msg = format!("Raffle completed for '{}'\n\n", proposal_name);
                
                msg.push_str("**Counted voters:**\n");
                for team in counted {
                    msg.push_str(&format!("- {}\n", team));
                }
                
                msg.push_str("\n**Uncounted voters:**\n");
                for team in uncounted {
                    msg.push_str(&format!("- {}\n", team));
                }
                
                msg
            }
        }
    }

    /// Formats the progress update as a markdown-compatible message for Telegram
    pub fn format_telegram_message(&self) -> String {
        use crate::escape_markdown;
        
        match self {
            RaffleProgress::Preparing { proposal_name, ticket_ranges } => {
                let mut msg = format!("*Preparing raffle for proposal:* {}\n\n", escape_markdown(proposal_name));
                for (team_name, start, end) in ticket_ranges {
                    msg.push_str(&format!("  `{}` ballot range \\[{}\\.\\.{}\\]\n", 
                        escape_markdown(team_name), start, end));
                }
                msg
            },
            RaffleProgress::WaitingForBlock { current, target } => {
                format!("Current block: `{}`\nTarget block: `{}`\n_Waiting for target block\\.\\.\\._", 
                    current, target)
            },
            RaffleProgress::RandomnessAcquired { block_hash } => {
                format!("Block randomness acquired: `{}`", escape_markdown(block_hash))
            },
            RaffleProgress::ReadyToFinalize { proposal_name, current_block, target_block, .. } => {
                format!(
                    "Ready to finalize raffle for *{}*\nUsing randomness from block `{}` \\(initiated at block `{}`\\)",
                    escape_markdown(proposal_name), target_block, current_block
                )
            },
            RaffleProgress::Completed { proposal_name, counted, uncounted, .. } => {
                let mut msg = format!("Raffle completed for *{}*\n\n", escape_markdown(proposal_name));
                
                msg.push_str("*Counted voters:*\n");
                for team in counted {
                    msg.push_str(&format!("\\- {}\n", escape_markdown(team)));
                }
                
                msg.push_str("\n*Uncounted voters:*\n");
                for team in uncounted {
                    msg.push_str(&format!("\\- {}\n", escape_markdown(team)));
                }
                
                msg
            }
        }
    }

    /// Returns true if this progress represents a state where finalization is possible
    pub fn is_ready_to_finalize(&self) -> bool {
        matches!(self, RaffleProgress::ReadyToFinalize { .. })
    }

    /// Returns true if this progress represents a completed raffle
    pub fn is_completed(&self) -> bool {
        matches!(self, RaffleProgress::Completed { .. })
    }
}

#[derive(Debug)]
pub struct RaffleCreationError(pub String);

impl std::fmt::Display for RaffleCreationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Raffle creation error: {}", self.0)
    }
}

impl Error for RaffleCreationError {}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_progress_formatting() {
        // Test Preparing state
        let progress = RaffleProgress::Preparing {
            proposal_name: "Test Proposal".to_string(),
            ticket_ranges: vec![
                ("Team A".to_string(), 0, 5),
                ("Team B".to_string(), 6, 10),
            ]
        };
        let msg = progress.format_message();
        assert!(msg.contains("Test Proposal"));
        assert!(msg.contains("Team A"));
        assert!(msg.contains("[0..5]"));

        // Test ReadyToFinalize state
        let progress = RaffleProgress::ReadyToFinalize {
            raffle_id: Uuid::new_v4(),
            proposal_name: "Test Proposal".to_string(),
            current_block: 100,
            target_block: 110,
            randomness: "test_hash".to_string(),
        };
        let msg = progress.format_message();
        assert!(msg.contains("Ready to finalize"));
        assert!(msg.contains("Test Proposal"));
        assert!(msg.contains("block 110"));

        // Test Completed state
        let progress = RaffleProgress::Completed {
            raffle_id: Uuid::new_v4(),
            proposal_name: "Test Proposal".to_string(),
            counted: vec!["Team A".to_string()],
            uncounted: vec!["Team B".to_string()],
        };
        let msg = progress.format_message();
        assert!(msg.contains("Team A"));
        assert!(msg.contains("Team B"));
    }

    #[test]
    fn test_telegram_message_escaping() {
        let progress = RaffleProgress::ReadyToFinalize {
            raffle_id: Uuid::new_v4(),
            proposal_name: "Test_Proposal*".to_string(),
            current_block: 100,
            target_block: 110,
            randomness: "test_hash*".to_string(),
        };
        let msg = progress.format_telegram_message();
        assert!(msg.contains("Test\\_Proposal\\*"));
        assert!(msg.contains("\\(initiated at block"));
    }

    #[test]
    fn test_state_checks() {
        let progress = RaffleProgress::ReadyToFinalize {
            raffle_id: Uuid::new_v4(),
            proposal_name: "Test".to_string(),
            current_block: 100,
            target_block: 110,
            randomness: "test".to_string(),
        };
        assert!(progress.is_ready_to_finalize());
        assert!(!progress.is_completed());

        let progress = RaffleProgress::Completed {
            raffle_id: Uuid::new_v4(),
            proposal_name: "Test".to_string(),
            counted: vec![],
            uncounted: vec![],
        };
        assert!(!progress.is_ready_to_finalize());
        assert!(progress.is_completed());
    }
}