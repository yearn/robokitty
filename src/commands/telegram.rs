use teloxide::utils::command::BotCommands;
use crate::core::budget_system::BudgetSystem;
use crate::commands::common::{Command, CommandExecutor};
use chrono::{NaiveDate, DateTime, Utc, TimeZone};

/// These commands are supported:
#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "snake_case",
    parse_with = "split"
)]
pub enum TelegramCommand {
    /// Display this text.
    Help,
    
    /// Display team information.
    PrintTeamReport,
    
    /// Show current epoch status.
    PrintEpochState,
    
    /// Activate an epoch. Usage: /activate_epoch <name>
    ActivateEpoch {
        name: String
    },

    /// Set epoch reward. Usage: /set_epoch_reward <token> <amount>
    SetEpochReward {
        token: String,
        amount: String
    },

    /// Display a team's vote participation. Usage: /print_team_participation <team_name> <epoch_name>
    PrintTeamParticipation{
        team_name: String,
        epoch_name: String
    },

    /// Create a new epoch. Usage: /create_epoch <name> <start_date YYYY-MM-DD> <end_date YYYY-MM-DD>
    CreateEpoch{
        name: String,
        start_date: String,
        end_date: String
    },
}

fn parse_date(date_str: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
        .map_err(|e| format!("Invalid date format (use YYYY-MM-DD): {}", e))
}

fn parse_start_date(date_str: &str) -> Result<DateTime<Utc>, String> {
    let date = parse_date(date_str)?;
    Ok(Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap()))
}

fn parse_end_date(date_str: &str) -> Result<DateTime<Utc>, String> {
    let date = parse_date(date_str)?;
    Ok(Utc.from_utc_datetime(&date.and_hms_opt(23, 59, 59).unwrap()))
}

pub async fn execute_command(
    telegram_cmd: TelegramCommand,
    budget_system: &mut BudgetSystem,
) -> Result<String, Box<dyn std::error::Error>> {
    match telegram_cmd {
        TelegramCommand::Help => {
            Ok(format!("Available commands:\n\n{}", TelegramCommand::descriptions()))
        },
        TelegramCommand::PrintTeamReport => {
            budget_system.execute_command(Command::PrintTeamReport).await
        },
        TelegramCommand::PrintEpochState => {
            budget_system.execute_command(Command::PrintEpochState).await
        },
        TelegramCommand::PrintTeamParticipation { team_name, epoch_name } => {
            budget_system.execute_command(Command::PrintTeamVoteParticipation { 
                team_name, 
                epoch_name: Some(epoch_name)
            }).await
        },
        TelegramCommand::CreateEpoch { name, start_date, end_date } => {
            let start_date = parse_start_date(&start_date)
                .map_err(|e| format!("Invalid start date: {}", e))?;
            let end_date = parse_end_date(&end_date)
                .map_err(|e| format!("Invalid end date: {}", e))?;

            budget_system.execute_command(Command::CreateEpoch { 
                name, 
                start_date, 
                end_date
            }).await
        },
        TelegramCommand::ActivateEpoch { name } => {
            budget_system.execute_command(Command::ActivateEpoch { name }).await
        },
        TelegramCommand::SetEpochReward { token, amount } => {
            let amount = amount.parse::<f64>()
                .map_err(|e| format!("Invalid amount: {}", e))?;
            budget_system.execute_command(Command::SetEpochReward { token, amount }).await
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use teloxide::utils::command::BotCommands;
    use chrono::TimeZone;

    #[test]
    fn test_parse_help_command() {
        assert!(matches!(
            TelegramCommand::parse("/help", "bot_name").unwrap(),
            TelegramCommand::Help
        ));
    }

    #[test]
    fn test_parse_print_team_report_command() {
        assert!(matches!(
            TelegramCommand::parse("/print_team_report", "bot_name").unwrap(),
            TelegramCommand::PrintTeamReport
        ));
    }

    #[test]
    fn test_parse_print_team_participation_command() {
        let cmd = TelegramCommand::parse("/print_team_participation TeamA EpochB", "bot_name").unwrap();
        match cmd {
            TelegramCommand::PrintTeamParticipation { team_name, epoch_name } => {
                assert_eq!(team_name, "TeamA");
                assert_eq!(epoch_name, "EpochB");
            },
            _ => panic!("Wrong command parsed")
        }
    }

    #[test]
    fn test_parse_create_epoch_command() {
        let cmd = TelegramCommand::parse(
            "/create_epoch TestEpoch 2024-01-01 2024-12-31", 
            "bot_name"
        ).unwrap();
        match cmd {
            TelegramCommand::CreateEpoch { name, start_date, end_date } => {
                assert_eq!(name, "TestEpoch");
                assert_eq!(start_date, "2024-01-01");
                assert_eq!(end_date, "2024-12-31");
            },
            _ => panic!("Wrong command parsed")
        }
    }

    #[test]
    fn test_parse_activate_epoch_command() {
        let cmd = TelegramCommand::parse("/activate_epoch TestEpoch", "bot_name").unwrap();
        match cmd {
            TelegramCommand::ActivateEpoch { name } => {
                assert_eq!(name, "TestEpoch");
            },
            _ => panic!("Wrong command parsed")
        }
    }

    #[test]
    fn test_parse_set_epoch_reward_command() {
        let cmd = TelegramCommand::parse("/set_epoch_reward ETH 100.5", "bot_name").unwrap();
        match cmd {
            TelegramCommand::SetEpochReward { token, amount } => {
                assert_eq!(token, "ETH");
                assert_eq!(amount, "100.5");
            },
            _ => panic!("Wrong command parsed")
        }
    }

    // Add tests for error cases
    #[test]
    fn test_parse_print_team_participation_missing_args() {
        assert!(TelegramCommand::parse("/print_team_participation", "bot_name").is_err());
        assert!(TelegramCommand::parse("/print_team_participation TeamA", "bot_name").is_err());
    }

    #[test]
    fn test_parse_create_epoch_invalid_args() {
        assert!(TelegramCommand::parse("/create_epoch", "bot_name").is_err());
        assert!(TelegramCommand::parse("/create_epoch TestEpoch", "bot_name").is_err());
        assert!(TelegramCommand::parse("/create_epoch TestEpoch 2024-01-01", "bot_name").is_err());
    }
    
    #[test]
    fn test_activate_epoch_command() {
        let cmd = TelegramCommand::parse("/activate_epoch TestEpoch", "bot_name").unwrap();
        if let TelegramCommand::ActivateEpoch { name } = cmd {
            assert_eq!(name, "TestEpoch");
        } else {
            panic!("Wrong command parsed");
        }
    }

    #[test]
    fn test_set_epoch_reward_command() {
        let cmd = TelegramCommand::parse("/set_epoch_reward ETH 100.5", "bot_name").unwrap();
        if let TelegramCommand::SetEpochReward { token, amount } = cmd {
            assert_eq!(token, "ETH");
            assert_eq!(amount, "100.5");
        } else {
            panic!("Wrong command parsed");
        }
    }

    #[test]
    fn test_invalid_commands() {
        assert!(TelegramCommand::parse("/unknown_command", "bot_name").is_err());
        assert!(TelegramCommand::parse("/create_epoch", "bot_name").is_err()); // Missing arguments
        assert!(TelegramCommand::parse("/set_epoch_reward ETH", "bot_name").is_err()); // Missing amount
    }

    #[test]
    fn test_date_parsing_edge_cases() {
        // Test leap year
        assert!(parse_date("2024-02-29").is_ok());
        assert!(parse_date("2023-02-29").is_err());
        
        // Test invalid days
        assert!(parse_date("2024-04-31").is_err());
        assert!(parse_date("2024-06-31").is_err());
        
        // Test boundary dates
        assert!(parse_date("9999-12-31").is_ok());
        assert!(parse_date("0000-01-01").is_ok());
    }

    #[test]
    fn test_date_parsing() {
        // Test start date parsing (00:00:00 UTC)
        let start_result = parse_start_date("2024-01-01").unwrap();
        assert_eq!(
            start_result,
            Utc.ymd(2024, 1, 1).and_hms(0, 0, 0)
        );

        // Test end date parsing (23:59:59 UTC)
        let end_result = parse_end_date("2024-01-01").unwrap();
        assert_eq!(
            end_result,
            Utc.ymd(2024, 1, 1).and_hms(23, 59, 59)
        );

        // Test invalid dates
        assert!(parse_start_date("2024-13-01").is_err()); // Invalid month
        assert!(parse_end_date("01/01/2024").is_err()); // Wrong format
    }

    #[test]
    fn test_date_boundaries() {
        let start = parse_start_date("2024-01-01").unwrap();
        let end = parse_end_date("2024-01-01").unwrap();
        
        assert_eq!(start.time(), chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
        assert_eq!(end.time(), chrono::NaiveTime::from_hms_opt(23, 59, 59).unwrap());
        
        // Test day difference
        assert_eq!((end - start).num_seconds(), 86399); // 23:59:59 worth of seconds
    }
}