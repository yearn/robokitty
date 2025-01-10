use uuid::Uuid;
use std::error::Error;
use crate::core::models::TeamStatus;

#[derive(Debug, Clone)]
pub enum RaffleProgress {
    Preparing {
        proposal_name: String,
        raffle_id: Uuid,
        ticket_ranges: Vec<(String, u64, u64)>
    },
    WaitingForBlock {
        proposal_name: String,
        raffle_id: Uuid,
        current_block: u64,
        target_block: u64
    },
    RandomnessAcquired {
        proposal_name: String,
        raffle_id: Uuid,
        current_block: u64,
        target_block: u64,
        randomness: String
    },
    Completed {
        proposal_name: String,
        raffle_id: Uuid,
        counted: Vec<(TeamStatus, String)>,
        uncounted: Vec<(TeamStatus, String)>,
    },
    Failed(String)
}

impl RaffleProgress {
    pub fn format_message(&self) -> String {
        match self {
            RaffleProgress::Preparing { proposal_name, ticket_ranges, .. } => {
                let mut msg = format!("Preparing raffle for proposal: {}\n", proposal_name);
                for (team_name, start, end) in ticket_ranges {
                    msg.push_str(&format!("  {} ballot range [{}..{}]\n", team_name, start, end));
                }
                msg
            },
            RaffleProgress::WaitingForBlock { current_block, target_block, .. } => {
                format!(
                    "Current block number: {}\n\
                     Target block for randomness: {}\n\
                     Latest observed block: {}", 
                    current_block, target_block, current_block)
            },
            RaffleProgress::RandomnessAcquired { target_block, randomness, .. } => {
                format!(
                    "Block randomness: {}\n\
                     Etherscan URL: https://etherscan.io/block/{}#consensusinfo",
                    randomness, target_block)
            },
            RaffleProgress::Completed { proposal_name, raffle_id, counted, uncounted } => {
                let mut msg = format!("Raffle results for proposal '{}' (Raffle ID: {})\n\n", proposal_name, raffle_id);
                
                msg.push_str("**Counted voters:**\n");
                msg.push_str("Earner teams:\n");
                let earner_count = counted.iter()
                    .filter(|(status, _)| matches!(status, TeamStatus::Earner { .. }))
                    .count();
                
                // Print counted earners
                for (status, team_info) in counted.iter() {
                    if matches!(status, TeamStatus::Earner { .. }) {
                        msg.push_str(&format!("  {}\n", team_info));
                    }
                }
                
                msg.push_str("Supporter teams:\n");
                for (status, team_info) in counted.iter() {
                    if matches!(status, TeamStatus::Supporter) {
                        msg.push_str(&format!("  {}\n", team_info));
                    }
                }
                
                msg.push_str(&format!("Total counted voters: {} (Earners: {}, Supporters: {})\n\n", 
                    counted.len(), earner_count, counted.len() - earner_count));
                
                msg.push_str("**Uncounted voters:**\n");
                msg.push_str("Earner teams:\n");
                for (status, team_info) in uncounted.iter() {
                    if matches!(status, TeamStatus::Earner { .. }) {
                        msg.push_str(&format!("  {}\n", team_info));
                    }
                }
                
                msg.push_str("Supporter teams:\n");
                for (status, team_info) in uncounted.iter() {
                    if matches!(status, TeamStatus::Supporter) {
                        msg.push_str(&format!("  {}\n", team_info));
                    }
                }
                msg
            },
            RaffleProgress::Failed(error) => format!("Raffle failed: {}", error),
        }
    }

    pub fn format_telegram_message(&self) -> String {
        use crate::escape_markdown;
        
        match self {
            RaffleProgress::Preparing { proposal_name, ticket_ranges, .. } => {
                let mut msg = format!("Preparing raffle for proposal: {}\n", escape_markdown(proposal_name));
                for (team_name, start, end) in ticket_ranges {
                    msg.push_str(&format!("  {} ballot range \\[{}\\.\\.{}\\]\n", 
                        escape_markdown(team_name), start, end));
                }
                msg
            },
            RaffleProgress::WaitingForBlock { current_block, target_block, .. } => {
                format!(
                    "Current block number: `{}`\n\
                     Target block for randomness: `{}`\n\
                     Latest observed block: `{}`", 
                    current_block, target_block, current_block)
            },
            RaffleProgress::RandomnessAcquired { target_block, randomness, .. } => {
                format!(
                    "Block randomness: `{}`\n\
                     Etherscan URL: https://etherscan\\.io/block/{}\\#consensusinfo",
                    escape_markdown(randomness), target_block)
            },
            RaffleProgress::Completed { proposal_name, raffle_id, counted, uncounted } => {
                let mut msg = format!("Raffle results for proposal '{}' \\(Raffle ID: {}\\)\n\n", 
                    escape_markdown(proposal_name), raffle_id);
                
                msg.push_str("*Counted voters:*\n");
                msg.push_str("_Earner teams:_\n");
                let earner_count = counted.iter()
                    .filter(|(status, _)| matches!(status, TeamStatus::Earner { .. }))
                    .count();
                
                // Print counted earners
                for (status, team_info) in counted.iter() {
                    if matches!(status, TeamStatus::Earner { .. }) {
                        msg.push_str(&format!("  {}\n", escape_markdown(team_info)));
                    }
                }
                
                msg.push_str("_Supporter teams:_\n");
                for (status, team_info) in counted.iter() {
                    if matches!(status, TeamStatus::Supporter) {
                        msg.push_str(&format!("  {}\n", escape_markdown(team_info)));
                    }
                }
                
                msg.push_str(&format!("Total counted voters: {} \\(Earners: {}, Supporters: {}\\)\n\n", 
                    counted.len(), earner_count, counted.len() - earner_count));
                
                msg.push_str("*Uncounted voters:*\n");
                msg.push_str("_Earner teams:_\n");
                for (status, team_info) in uncounted.iter() {
                    if matches!(status, TeamStatus::Earner { .. }) {
                        msg.push_str(&format!("  {}\n", escape_markdown(team_info)));
                    }
                }
                
                msg.push_str("_Supporter teams:_\n");
                for (status, team_info) in uncounted.iter() {
                    if matches!(status, TeamStatus::Supporter) {
                        msg.push_str(&format!("  {}\n", escape_markdown(team_info)));
                    }
                }
                msg
            },
            RaffleProgress::Failed(error) => format!("❌ Raffle failed: {}", escape_markdown(error)),
        }
    }

    pub fn raffle_id(&self) -> Option<Uuid> {
        match self {
            RaffleProgress::Preparing { raffle_id, .. } |
            RaffleProgress::WaitingForBlock { raffle_id, .. } |
            RaffleProgress::RandomnessAcquired { raffle_id, .. } |
            RaffleProgress::Completed { raffle_id, .. } => Some(*raffle_id),
            RaffleProgress::Failed(_) => None,
        }
    }

    pub fn is_complete(&self) -> bool {
        matches!(self, RaffleProgress::Completed { .. })
    }

    pub fn is_failed(&self) -> bool {
        matches!(self, RaffleProgress::Failed(_))
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

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_progress_formatting() {
//         // Test Preparing state
//         let progress = RaffleProgress::Preparing {
//             raffle_id: Uuid::new_v4(),
//             proposal_name: "Test Proposal".to_string(),
//             ticket_ranges: vec![
//                 ("Team A".to_string(), 0, 5),
//                 ("Team B".to_string(), 6, 10),
//             ]
//         };
//         let msg = progress.format_message();
//         assert!(msg.contains("Test Proposal"));
//         assert!(msg.contains("Team A"));
//         assert!(msg.contains("[0..5]"));

//         // Test ReadyToFinalize state
//         let progress = RaffleProgress::ReadyToFinalize {
//             raffle_id: Uuid::new_v4(),
//             proposal_name: "Test Proposal".to_string(),
//             current_block: 100,
//             target_block: 110,
//             randomness: "test_hash".to_string(),
//         };
//         let msg = progress.format_message();
//         assert!(msg.contains("Ready to finalize"));
//         assert!(msg.contains("Test Proposal"));
//         assert!(msg.contains("block 110"));

//         // Test Completed state
//         let progress = RaffleProgress::Completed {
//             raffle_id: Uuid::new_v4(),
//             proposal_name: "Test Proposal".to_string(),
//             counted: vec!["Team A".to_string()],
//             uncounted: vec!["Team B".to_string()],
//         };
//         let msg = progress.format_message();
//         assert!(msg.contains("Team A"));
//         assert!(msg.contains("Team B"));
//     }

//     #[test]
//     fn test_telegram_message_escaping() {
//         let progress = RaffleProgress::ReadyToFinalize {
//             raffle_id: Uuid::new_v4(),
//             proposal_name: "Test_Proposal*".to_string(),
//             current_block: 100,
//             target_block: 110,
//             randomness: "test_hash*".to_string(),
//         };
//         let msg = progress.format_telegram_message();
//         assert!(msg.contains("Test\\_Proposal\\*"));
//         assert!(msg.contains("\\(initiated at block"));
//     }

//     #[test]
//     fn test_state_checks() {
//         let progress = RaffleProgress::ReadyToFinalize {
//             raffle_id: Uuid::new_v4(),
//             proposal_name: "Test".to_string(),
//             current_block: 100,
//             target_block: 110,
//             randomness: "test".to_string(),
//         };
//         assert!(progress.is_ready_to_finalize());
//         assert!(!progress.is_completed());

//         let progress = RaffleProgress::Completed {
//             raffle_id: Uuid::new_v4(),
//             proposal_name: "Test".to_string(),
//             counted: vec![],
//             uncounted: vec![],
//         };
//         assert!(!progress.is_ready_to_finalize());
//         assert!(progress.is_completed());
//     }
// }