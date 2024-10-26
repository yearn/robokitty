use teloxide::utils::command::BotCommands;
use crate::core::budget_system::BudgetSystem;
use crate::commands::common::{Command, CommandExecutor, BudgetRequestDetailsCommand, UpdateTeamDetails};
use chrono::{NaiveDate, DateTime, Utc, TimeZone};
use std::collections::HashMap;

/// These commands are supported:
#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "snake_case",
)]
pub enum TelegramCommand {
    /// Display this text.
    Help,
    
    /// Display team information.
    PrintTeamReport,
    
    /// Show current epoch status.
    PrintEpochState,
    
    /// Activate an epoch. Usage: /activate_epoch <name>
    #[command(parse_with = "split")]
    ActivateEpoch {
        name: String
    },

    /// Set epoch reward. Usage: /set_epoch_reward <token> <amount>
    #[command(parse_with = "split")]
    SetEpochReward {
        token: String,
        amount: String
    },

    /// Display a team's vote participation. Usage: /print_team_participation <team_name> <epoch_name>
    #[command(parse_with = "split")]
    PrintTeamParticipation{
        team_name: String,
        epoch_name: String
    },

    /// Create a new epoch. Usage: /create_epoch <name> <start_date YYYY-MM-DD> <end_date YYYY-MM-DD>
    #[command(parse_with = "split")]
    CreateEpoch{
        name: String,
        start_date: String,
        end_date: String
    },

     /// Add a new team. Usage: /add_team <name> <representative> revenue1 revenue2 revenue3
     AddTeam {
         args: String,
     },

    /// Update a team's details. Usage: /update_team "Team Name" [name:"New Name"] [rep:"New Rep"] [status:Earner|Supporter|Inactive] [rev:1000,2000,3000]
        UpdateTeam {
        args: String,
    },

    /// Add a new proposal. Usage: /add_proposal "<title>" <url> [team:<name>] [amounts:<token>:<amount>,...] [start:<YYYY-MM-DD>] [end:<YYYY-MM-DD>] [ann:<YYYY-MM-DD>] [pub:<YYYY-MM-DD>]
    AddProposal {
        args: String,
    },

}

impl TelegramCommand {
    fn parse_date(date_str: &str) -> Result<NaiveDate, String> {
        NaiveDate::parse_from_str(date_str, "%Y-%m-%d")
            .map_err(|e| format!("Invalid date format (use YYYY-MM-DD): {}", e))
    }
    
    fn parse_start_date(date_str: &str) -> Result<DateTime<Utc>, String> {
        let date = Self::parse_date(date_str)?;
        Ok(Utc.from_utc_datetime(&date.and_hms_opt(0, 0, 0).unwrap()))
    }
    
    fn parse_end_date(date_str: &str) -> Result<DateTime<Utc>, String> {
        let date = Self::parse_date(date_str)?;
        Ok(Utc.from_utc_datetime(&date.and_hms_opt(23, 59, 59).unwrap()))
    }
    
    fn parse_command_args(input: &str) -> Result<Vec<String>, String> {
        let mut args = Vec::new();
        let mut current_arg = String::new();
        let mut chars = input.chars().peekable();
        let mut in_quotes = false;
    
        while let Some(c) = chars.next() {
            match c {
                '"' => {
                    if in_quotes {
                        // End of quoted string
                        if !current_arg.is_empty() {
                            args.push(current_arg.clone());
                            current_arg.clear();
                        }
                        in_quotes = false;
                    } else {
                        // Start of quoted string
                        if !current_arg.is_empty() {
                            return Err("Unexpected quote in middle of argument".to_string());
                        }
                        in_quotes = true;
                    }
                },
                ' ' if !in_quotes => {
                    if !current_arg.is_empty() {
                        args.push(current_arg.clone());
                        current_arg.clear();
                    }
                },
                _ => current_arg.push(c),
            }
        }
    
        if in_quotes {
            return Err("Unclosed quote".to_string());
        }
    
        if !current_arg.is_empty() {
            args.push(current_arg);
        }
    
        Ok(args)
    }

    fn parse_add_team(args: &Vec<String>) -> Result<(String, String, Option<Vec<u64>>), String> {
        if args.len() < 2 {
            return Err("Usage: /add_team <name> <representative> revenue1 revenue2 revenue3".to_string());
        }

        let name = args[0].clone();
        let representative = args[1].clone();
        
        let trailing_monthly_revenue = if args.len() > 2 {
            let mut revenues = Vec::new();
            for rev_str in &args[2..] {
                match rev_str.parse::<u64>() {
                    Ok(rev) => revenues.push(rev),
                    Err(_) => return Err(format!("Invalid revenue value: {}", rev_str)),
                }
            }
            if revenues.len() > 3 {
                return Err("Maximum of 3 revenue values allowed".to_string());
            }
            Some(revenues)
        } else {
            None
        };

        Ok((name, representative, trailing_monthly_revenue))
    }

    fn parse_update_team_args(args:&Vec<String>) -> Result<(String, UpdateTeamDetails), String> {
        if args.is_empty() {
            return Err("Usage: /update_team \"Team Name\" [name:\"New Name\"] [rep:\"New Rep\"] [status:Earner|Supporter|Inactive] [revenue:1000,2000,3000]".to_string());
        }
    
        let team_name = args[0].clone();
        let mut updates = UpdateTeamDetails {
            name: None,
            representative: None,
            status: None,
            trailing_monthly_revenue: None,
        };
    
        for arg in args.iter().skip(1) {
            if let Some((key, value)) = arg.split_once(':') {
                match key {
                    "name" => updates.name = Some(value.trim_matches('"').to_string()),
                    "rep" => updates.representative = Some(value.trim_matches('"').to_string()),
                    "status" => updates.status = Some(value.to_string()),
                    "rev" => {
                        updates.trailing_monthly_revenue = Some(
                            value.split(',')
                                .map(|v| v.parse::<u64>())
                                .collect::<Result<Vec<_>, _>>()
                                .map_err(|_| "Invalid revenue format. Expected numbers separated by commas")?
                        );
                    },
                    _ => return Err(format!("Unknown parameter: {}", key)),
                }
            } else {
                return Err(format!("Invalid parameter format: {}. Expected key:value", arg));
            }
        }
    
        Ok((team_name, updates))
    }

    fn parse_proposal_args(args: &[String]) -> Result<(String, String, Option<BudgetRequestDetailsCommand>, Option<NaiveDate>, Option<NaiveDate>), String> {
        if args.len() < 2 {
            return Err("Usage: /add_proposal \"<title>\" <url> [team:<name>] [amounts:<token>:<amount>,...] [start:<YYYY-MM-DD>] [end:<YYYY-MM-DD>] [ann:<YYYY-MM-DD>] [pub:<YYYY-MM-DD>]".to_string());
        }

        let title = args[0].clone();
        let url = args[1].clone();
        
        let mut budget_details = None;
        let mut announced_at = None;
        let mut published_at = None;
        let mut team_name = None;
        let mut amounts = None;
        let mut start_date = None;
        let mut end_date = None;

        let mut i = 2;
        while i < args.len() {
            let arg = &args[i];
            if let Some((key, value)) = arg.split_once(':') {
                match key {
                    "team" => team_name = Some(value.to_string()),
                    "amounts" => {
                        amounts = Some(Self::parse_amounts(value)?);
                    },
                    "start" => start_date = Some(NaiveDate::parse_from_str(value, "%Y-%m-%d")
                        .map_err(|e| format!("Invalid start date: {}", e))?),
                    "end" => end_date = Some(NaiveDate::parse_from_str(value, "%Y-%m-%d")
                        .map_err(|e| format!("Invalid end date: {}", e))?),
                    "ann" => announced_at = Some(NaiveDate::parse_from_str(value, "%Y-%m-%d")
                        .map_err(|e| format!("Invalid announcement date: {}", e))?),
                    "pub" => published_at = Some(NaiveDate::parse_from_str(value, "%Y-%m-%d")
                        .map_err(|e| format!("Invalid publication date: {}", e))?),
                    _ => return Err(format!("Unknown parameter: {}", key)),
                }
            }
            i += 1;
        }

        // If we have any budget-related details, create BudgetRequestDetailsCommand
        if team_name.is_some() || amounts.is_some() || start_date.is_some() || end_date.is_some() {
            budget_details = Some(BudgetRequestDetailsCommand {
                team: team_name,
                request_amounts: amounts,
                start_date,
                end_date,
                payment_status: None,
            });
        }

        Ok((title, url, budget_details, announced_at, published_at))
    }

    fn parse_amounts(amounts_str: &str) -> Result<HashMap<String, f64>, String> {
        let mut amounts = HashMap::new();
        for pair in amounts_str.trim_matches(|c| c == '[' || c == ']').split(',') {
            if let Some((token, amount_str)) = pair.split_once(':') {
                let amount = amount_str.parse::<f64>()
                    .map_err(|e| format!("Invalid amount {}: {}", amount_str, e))?;
                amounts.insert(token.to_string(), amount);
            } else {
                return Err(format!("Invalid amount format: {}", pair));
            }
        }
        Ok(amounts)
    }
}

pub async fn execute_command(
    telegram_cmd: TelegramCommand,
    budget_system: &mut BudgetSystem,
) -> Result<String, Box<dyn std::error::Error>> {
    match telegram_cmd {
        TelegramCommand::Help => {
            Ok(format!("{}", TelegramCommand::descriptions()))
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
            let start_date = TelegramCommand::parse_start_date(&start_date)
                .map_err(|e| format!("Invalid start date: {}", e))?;
            let end_date = TelegramCommand::parse_end_date(&end_date)
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
        TelegramCommand::AddTeam { args } => {
            let args = TelegramCommand::parse_command_args(&args)
                .map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, e)))?;
            
            let (name, representative, trailing_monthly_revenue) = TelegramCommand::parse_add_team(&args)
                .map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, e)))?;
            
            budget_system.execute_command(Command::AddTeam { 
                name, 
                representative, 
                trailing_monthly_revenue 
            }).await
        },
        TelegramCommand::UpdateTeam { args } => {
            let args = TelegramCommand::parse_command_args(&args)
                .map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, e)))?;

            let (team_name, updates) = TelegramCommand::parse_update_team_args(&args)
                .map_err(|e| format!("Failed to parse update team command: {}", e))?;

            budget_system.execute_command(Command::UpdateTeam {
                team_name: team_name.clone(),
                updates: updates.clone(),
            }).await?;

            // Enhanced feedback about what was updated
            let mut feedback = format!("Updated team '{}'", team_name);
            if let Some(name) = updates.name {
                feedback.push_str(&format!("\nNew name: {}", name));
            }
            if let Some(rep) = updates.representative {
                feedback.push_str(&format!("\nNew representative: {}", rep));
            }
            if let Some(status) = updates.status {
                feedback.push_str(&format!("\nNew status: {}", status));
            }
            if let Some(revenue) = updates.trailing_monthly_revenue {
                feedback.push_str(&format!("\nNew revenue: {:?}", revenue));
            }

            Ok(feedback)
        },
        TelegramCommand::AddProposal { args } => {
            let args = TelegramCommand::parse_command_args(&args)
                .map_err(|e| format!("Failed to parse proposal arguments: {}", e))?;
                
            let (title, url, budget_details, announced_at, published_at) = 
                TelegramCommand::parse_proposal_args(&args)?;
    
            budget_system.execute_command(Command::AddProposal {
                title,
                url: Some(url),
                budget_request_details: budget_details,
                announced_at,
                published_at,
                is_historical: Some(false),
            }).await
        },

    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use teloxide::utils::command::BotCommands;
    use chrono::TimeZone;

    use crate::core::budget_system::BudgetSystem;
    use crate::app_config::AppConfig;
    use crate::services::ethereum::MockEthereumService;
    use std::sync::Arc;

    async fn create_test_budget_system() -> BudgetSystem {
        let config = AppConfig::default();
        let ethereum_service = Arc::new(MockEthereumService);
        let mut budget_system = BudgetSystem::new(config, ethereum_service, None).await.unwrap();
        
        // Create a test team for update operations
        budget_system.create_team(
            "Test Team".to_string(),
            "Old Rep".to_string(),
            Some(vec![1000, 1000, 1000])
        ).unwrap();
        
        budget_system
    }

    #[test]
    fn test_parse_command_args() {
        // Basic args
        assert_eq!(
            TelegramCommand::parse_command_args("arg1 arg2 arg3").unwrap(),
            vec!["arg1", "arg2", "arg3"]
        );

        // Quoted strings
        assert_eq!(
            TelegramCommand::parse_command_args("arg1 \"John Doe\" arg3").unwrap(),
            vec!["arg1", "John Doe", "arg3"]
        );

        // Multiple quoted strings
        assert_eq!(
            TelegramCommand::parse_command_args("\"Team Name\" \"John Doe\" 1000").unwrap(),
            vec!["Team Name", "John Doe", "1000"]
        );

        // Error cases
        assert!(TelegramCommand::parse_command_args("arg1 \"unclosed").is_err());
        assert!(TelegramCommand::parse_command_args("arg1 qu\"ote arg2").is_err());
    }

    #[test]
    fn test_telegram_command_parsing() {
        // Basic team addition
        let cmd = TelegramCommand::parse("/add_team TeamName \"John Doe\"", "bot_name").unwrap();
        if let TelegramCommand::AddTeam { args } = cmd {
            let args = TelegramCommand::parse_command_args(&args).unwrap();
            let (name, rep, rev) = TelegramCommand::parse_add_team(&args).unwrap();
            assert_eq!(name, "TeamName");
            assert_eq!(rep, "John Doe");
            assert_eq!(rev, None);
        }

        // Team with revenue
        let cmd = TelegramCommand::parse("/add_team \"Team Name\" \"John Doe\" 1000 2000", "bot_name").unwrap();
        if let TelegramCommand::AddTeam { args } = cmd {
            let args = TelegramCommand::parse_command_args(&args).unwrap();
            let (name, rep, rev) = TelegramCommand::parse_add_team(&args).unwrap();
            assert_eq!(name, "Team Name");
            assert_eq!(rep, "John Doe");
            assert_eq!(rev, Some(vec![1000, 2000]));
        }
    }

    #[test]
    fn test_parse_command_args_with_complex_quotes() {
        assert_eq!(
            TelegramCommand::parse_command_args("\"Ghost Busters\" \"Egon Spenglar\" 3852 124981 1221").unwrap(),
            vec!["Ghost Busters", "Egon Spenglar", "3852", "124981", "1221"]
        );
    }
    

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
        assert!(TelegramCommand::parse_date("2024-02-29").is_ok());
        assert!(TelegramCommand::parse_date("2023-02-29").is_err());
        
        // Test invalid days
        assert!(TelegramCommand::parse_date("2024-04-31").is_err());
        assert!(TelegramCommand::parse_date("2024-06-31").is_err());
        
        // Test boundary dates
        assert!(TelegramCommand::parse_date("9999-12-31").is_ok());
        assert!(TelegramCommand::parse_date("0000-01-01").is_ok());
    }

    #[test]
    fn test_date_parsing() {
        // Test start date parsing (00:00:00 UTC)
        let start_result = TelegramCommand::parse_start_date("2024-01-01").unwrap();
        assert_eq!(
            start_result,
            Utc.ymd(2024, 1, 1).and_hms(0, 0, 0)
        );

        // Test end date parsing (23:59:59 UTC)
        let end_result = TelegramCommand::parse_end_date("2024-01-01").unwrap();
        assert_eq!(
            end_result,
            Utc.ymd(2024, 1, 1).and_hms(23, 59, 59)
        );

        // Test invalid dates
        assert!(TelegramCommand::parse_start_date("2024-13-01").is_err()); // Invalid month
        assert!(TelegramCommand::parse_end_date("01/01/2024").is_err()); // Wrong format
    }

    #[test]
    fn test_date_boundaries() {
        let start = TelegramCommand::parse_start_date("2024-01-01").unwrap();
        let end = TelegramCommand::parse_end_date("2024-01-01").unwrap();
        
        assert_eq!(start.time(), chrono::NaiveTime::from_hms_opt(0, 0, 0).unwrap());
        assert_eq!(end.time(), chrono::NaiveTime::from_hms_opt(23, 59, 59).unwrap());
        
        // Test day difference
        assert_eq!((end - start).num_seconds(), 86399); // 23:59:59 worth of seconds
    }

    #[test]
    fn test_parse_update_team_args() {
        // Test basic update
        let text = "\"Test Team\" name:\"New Name\" rep:\"New Rep\" status:Earner rev:1000,2000,3000".to_string();
        let args = TelegramCommand::parse_command_args(&text)
        .map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, e))).unwrap();

        let result = TelegramCommand::parse_update_team_args(&args);
        assert!(result.is_ok());
        let (team_name, updates) = result.unwrap();
        assert_eq!(team_name, "Test Team");
        assert_eq!(updates.name, Some("New Name".to_string()));
        assert_eq!(updates.representative, Some("New Rep".to_string()));
        assert_eq!(updates.status, Some("Earner".to_string()));
        assert_eq!(updates.trailing_monthly_revenue, Some(vec![1000, 2000, 3000]));

        // Test partial update
        let text = "\"Test Team\" status:Supporter".to_string();
        let args = TelegramCommand::parse_command_args(&text)
        .map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, e))).unwrap();

        let result = TelegramCommand::parse_update_team_args(&args);
        assert!(result.is_ok());
        let (_, updates) = result.unwrap();
        assert_eq!(updates.status, Some("Supporter".to_string()));
        assert!(updates.name.is_none());
        assert!(updates.representative.is_none());
        assert!(updates.trailing_monthly_revenue.is_none());

        // Test invalid format
        let text = "\"Test Team\" invalid_param".to_string();
        let args = TelegramCommand::parse_command_args(&text)
        .map_err(|e| Box::new(std::io::Error::new(std::io::ErrorKind::InvalidInput, e))).unwrap();

        let result = TelegramCommand::parse_update_team_args(&args);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_update_team_command() {
        let mut budget_system = create_test_budget_system().await;

        // Test full update
        let cmd = TelegramCommand::UpdateTeam {
            args: "\"Test Team\" name:\"New Name\" rep:\"New Rep\" status:Earner rev:2000,3000,4000".to_string()
        };
        let result = execute_command(cmd, &mut budget_system).await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.contains("New name: New Name"));
        assert!(response.contains("New representative: New Rep"));

        // Test partial update
        let cmd = TelegramCommand::UpdateTeam {
            args: "\"New Name\" status:Supporter".to_string()
        };
        let result = execute_command(cmd, &mut budget_system).await;
        assert!(result.is_ok());
        let response = result.unwrap();
        assert!(response.contains("New status: Supporter"));

        // Test update non-existent team
        let cmd = TelegramCommand::UpdateTeam {
            args: "\"Non Existent Team\" status:Supporter".to_string()
        };
        let result = execute_command(cmd, &mut budget_system).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_proposal_args() {
        // Test basic proposal
        let args = vec![
            "Test Proposal".to_string(),
            "https://example.com".to_string(),
        ];
        let result = TelegramCommand::parse_proposal_args(&args);
        assert!(result.is_ok());
        let (title, url, budget_details, announced, published) = result.unwrap();
        assert_eq!(title, "Test Proposal");
        assert_eq!(url, "https://example.com");
        assert!(budget_details.is_none());
        assert!(announced.is_none());
        assert!(published.is_none());

        // Test full proposal with all optional parameters
        let args = vec![
            "Test Proposal".to_string(),
            "https://example.com".to_string(),
            "team:TestTeam".to_string(),
            "amounts:[USD:1000,ETH:2.5]".to_string(),
            "start:2024-10-01".to_string(),
            "end:2024-12-31".to_string(),
            "ann:2024-09-30".to_string(),
            "pub:2024-10-01".to_string(),
        ];
        let result = TelegramCommand::parse_proposal_args(&args);
        assert!(result.is_ok());
        let (_, _, budget_details, announced, published) = result.unwrap();
        assert!(budget_details.is_some());
        let budget = budget_details.unwrap();
        assert_eq!(budget.team, Some("TestTeam".to_string()));
        assert!(budget.request_amounts.is_some());
        assert!(announced.is_some());
        assert!(published.is_some());
    }

    #[test]
    fn test_parse_amounts() {
        let amounts = "[USD:1000,ETH:2.5]";
        let result = TelegramCommand::parse_amounts(amounts);
        assert!(result.is_ok());
        let amounts = result.unwrap();
        assert_eq!(amounts.get("USD"), Some(&1000.0));
        assert_eq!(amounts.get("ETH"), Some(&2.5));

        // Test invalid amount
        let amounts = "[USD:invalid]";
        assert!(TelegramCommand::parse_amounts(amounts).is_err());
    }

    #[test]
    fn test_parse_proposal_args_with_unquoted_url() {
        let args = vec![
            "My proposal".to_string(),
            "https://google.com".to_string(),
        ];
        let result = TelegramCommand::parse_proposal_args(&args);
        assert!(result.is_ok());
        let (title, url, budget_details, announced, published) = result.unwrap();
        assert_eq!(title, "My proposal");
        assert_eq!(url, "https://google.com");
        assert!(budget_details.is_none());
        assert!(announced.is_none());
        assert!(published.is_none());

        // Also test with optional parameters
        let args = vec![
            "My proposal".to_string(),
            "https://google.com".to_string(),
            "team:MyTeam".to_string(),
        ];
        let result = TelegramCommand::parse_proposal_args(&args);
        assert!(result.is_ok());
        let (_, _, budget_details, _, _) = result.unwrap();
        assert!(budget_details.is_some());
        assert_eq!(budget_details.unwrap().team, Some("MyTeam".to_string()));
    }

}

