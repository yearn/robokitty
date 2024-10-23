use teloxide::utils::command::BotCommands;
use crate::core::budget_system::BudgetSystem;
use crate::commands::common::{Command, CommandExecutor};
use chrono::{NaiveDate, DateTime, Utc, TimeZone};

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "snake_case",
    description = "RoboKitty commands:",
    parse_with = "split"
)]
pub enum TelegramCommand {
    #[command(description = "show available commands")]
    Help,
    
    #[command(description = "display team information")]
    PrintTeamReport,
    
    #[command(description = "show current epoch status")]
    PrintEpochState,
    
    #[command(description = "show team's vote participation. Usage: /print_team_participation <team_name> <epoch_name>")]
    PrintTeamParticipation(String, String),

    #[command(description = "create a new epoch. Usage: /create_epoch <name> <start_date YYYY-MM-DD> <end_date YYYY-MM-DD>")]
    CreateEpoch(String, String, String),
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
        TelegramCommand::PrintTeamParticipation(team_name, epoch_name) => {
            budget_system.execute_command(Command::PrintTeamVoteParticipation { 
                team_name, 
                epoch_name: Some(epoch_name)
            }).await
        },
        TelegramCommand::CreateEpoch(name, start_date_str, end_date_str) => {
            let start_date = parse_start_date(&start_date_str)
                .map_err(|e| format!("Invalid start date: {}", e))?;
            let end_date = parse_end_date(&end_date_str)
                .map_err(|e| format!("Invalid end date: {}", e))?;

            budget_system.execute_command(Command::CreateEpoch { 
                name, 
                start_date, 
                end_date
            }).await
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use teloxide::utils::command::BotCommands;
    use chrono::TimeZone;

    #[test]
    fn test_command_parsing() {
        assert!(matches!(
            TelegramCommand::parse("/help", "bot_name").unwrap(),
            TelegramCommand::Help
        ));

        assert!(matches!(
            TelegramCommand::parse("/print_team_report", "bot_name").unwrap(),
            TelegramCommand::PrintTeamReport
        ));

        let cmd = TelegramCommand::parse("/print_team_participation TeamA EpochB", "bot_name").unwrap();
        if let TelegramCommand::PrintTeamParticipation(team_name, epoch_name) = cmd {
            assert_eq!(team_name, "TeamA");
            assert_eq!(epoch_name, "EpochB");
        } else {
            panic!("Wrong command parsed");
        }

        let cmd = TelegramCommand::parse(
            "/create_epoch TestEpoch 2024-01-01 2024-12-31", 
            "bot_name"
        ).unwrap();
        if let TelegramCommand::CreateEpoch(name, start_date, end_date) = cmd {
            assert_eq!(name, "TestEpoch");
            assert_eq!(start_date, "2024-01-01");
            assert_eq!(end_date, "2024-12-31");
        } else {
            panic!("Wrong command parsed");
        }
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