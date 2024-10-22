// src/commands/telegram.rs

use crate::core::models::{
    BudgetRequestDetails, PaymentStatus, Resolution, TeamStatus, VoteChoice
};
use crate::core::budget_system::BudgetSystem;
use crate::commands::common::{Command, CommandExecutor};

#[derive(Debug, PartialEq, Clone)]
pub enum TelegramCommand {
    PrintEpochState,
    MarkdownTest,
    // Map directly to common::Command variants
    PrintTeamReport,
    PrintPointReport {
        epoch_name: Option<String>,
    },
    GenerateReportForProposal {
        proposal_name: String,
    },
}

pub fn parse_command(text: &str) -> Option<TelegramCommand> {
    let parts: Vec<&str> = text.trim().split_whitespace().collect();
    if parts.is_empty() {
        return None;
    }

    match parts[0] {
        "/print_epoch_state" => Some(TelegramCommand::PrintEpochState),
        "/markdown_test" => Some(TelegramCommand::MarkdownTest),
        "/print_team_report" => Some(TelegramCommand::PrintTeamReport),
        "/print_point_report" => {
            let epoch_name = parts.get(1).map(|s| s.to_string());
            Some(TelegramCommand::PrintPointReport { epoch_name })
        }
        "/generate_proposal_report" => {
            if parts.len() < 2 {
                return None;
            }
            let proposal_name = parts[1..].join(" ");
            Some(TelegramCommand::GenerateReportForProposal { proposal_name })
        }
        _ => None,
    }
}

pub async fn handle_command(telegram_cmd: TelegramCommand, budget_system: &mut BudgetSystem) -> Result<String, Box<dyn std::error::Error>> {
    // Convert TelegramCommand to common::Command
    let cmd = match telegram_cmd {
        TelegramCommand::PrintEpochState => Command::PrintEpochState,
        TelegramCommand::MarkdownTest => return Ok(budget_system.generate_markdown_test()),
        TelegramCommand::PrintTeamReport => Command::PrintTeamReport,
        TelegramCommand::PrintPointReport { epoch_name } => Command::PrintPointReport { epoch_name },
        TelegramCommand::GenerateReportForProposal { proposal_name } => {
            Command::GenerateReportForProposal { proposal_name }
        }
    };

    // Use CommandExecutor trait to execute the command
    budget_system.execute_command(cmd).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_command_basic() {
        assert_eq!(parse_command("/print_epoch_state"), Some(TelegramCommand::PrintEpochState));
        assert_eq!(parse_command("/markdown_test"), Some(TelegramCommand::MarkdownTest));
        assert_eq!(parse_command("/print_team_report"), Some(TelegramCommand::PrintTeamReport));
    }

    #[test]
    fn test_parse_command_with_args() {
        assert_eq!(
            parse_command("/print_point_report E1"),
            Some(TelegramCommand::PrintPointReport { epoch_name: Some("E1".to_string()) })
        );
        assert_eq!(
            parse_command("/generate_proposal_report Test Proposal"),
            Some(TelegramCommand::GenerateReportForProposal { proposal_name: "Test Proposal".to_string() })
        );
    }

    #[test]
    fn test_parse_command_invalid() {
        assert_eq!(parse_command("/unknown_command"), None);
        assert_eq!(parse_command("not_a_command"), None);
    }
}