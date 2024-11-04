// src/commands/cli.rs
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io::Write};
use std::{fs, error::Error};
use uuid::Uuid;
use tokio::time::Duration;

use crate::core::models::{
    BudgetRequestDetails, Resolution, TeamStatus, VoteChoice, VoteType, VoteParticipation, NameMatches
};
use crate::core::budget_system::BudgetSystem;
use crate::app_config::AppConfig;
use super::common::{BudgetRequestDetailsCommand, Command, CommandExecutor, UpdateTeamDetails, UpdateProposalDetails};
use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "add-team", about = "Add a new team to the system")]
struct AddTeamArgs {
    /// Name of the team
    #[arg(long, short)]
    name: String,
    
    /// Team representative's name
    #[arg(long, short)]
    representative: String,
    
    /// Optional monthly revenue values (1-3 comma-separated numbers)
    #[arg(long)]
    revenue: Option<String>,
    
    /// Optional Ethereum payment address
    #[arg(long, value_parser = parse_eth_address)]
    address: Option<String>,
}

fn parse_eth_address(addr: &str) -> Result<String, String> {
    if !addr.starts_with("0x") {
        return Err("Ethereum address must start with 0x".into());
    }
    if addr.len() != 42 {
        return Err("Ethereum address must be 42 characters long".into());
    }
    // Basic hex check
    if !addr[2..].chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("Invalid hex characters in address".into());
    }
    Ok(addr.to_string())
}

pub async fn execute_command<W: Write + Send + 'static>(
    budget_system: &mut BudgetSystem,
    command: Command,
    config: &AppConfig,
    output: &mut W
) -> Result<(), Box<dyn Error>> {
    match command {
        Command::RunScript { script_file_path } => {
            let script_path = script_file_path.unwrap_or_else(|| config.script_file.clone());
            let script_commands = read_script_commands(&script_path)?;
            for cmd in script_commands {
                budget_system.execute_command_with_streaming(cmd, output).await?;
            }
            Ok(())
        },
        _ => {
            budget_system.execute_command_with_streaming(command, output).await
        }
    }
}

pub fn parse_cli_args(args: &[String]) -> Result<Command, Box<dyn Error>> {
    if args.len() < 2 {
        return Err("Not enough arguments. Usage: robokitty_script <command> [args...]".into());
    }

    let command = &args[1];
    let args = &args[2..];

    match command.as_str() {
        "add-team" => {
            // Create a dummy args array just for the add-team command
            let mut cmd_args = vec!["add-team".to_string()];
            // Skip the first two args (program name and command)
            cmd_args.extend_from_slice(&args);
            
            // Parse just the add-team command
            let add_team_args = AddTeamArgs::try_parse_from(cmd_args)?;
            
            // Parse the revenue string if it exists
            let trailing_monthly_revenue = add_team_args.revenue.map(|rev| -> Result<Vec<u64>, String> {
                let values: Vec<u64> = rev
                    .split(',')
                    .map(|v| v.trim().parse::<u64>()
                        .map_err(|e| format!("Invalid revenue value: {}", e)))
                    .collect::<Result<Vec<_>, _>>()?;
                
                if values.is_empty() || values.len() > 3 {
                    return Err("Must provide 1-3 revenue values".into());
                }
                Ok(values)
            }).transpose()?;
            
            Ok(Command::AddTeam { 
                name: add_team_args.name,
                representative: add_team_args.representative,
                trailing_monthly_revenue,
                address: add_team_args.address,
            })
        },
        "create-epoch" => {
            if args.len() != 3 {
                return Err("Usage: create-epoch <name> <start_date> <end_date>".into());
            }
            let name = args[0].clone();
            let start_date = DateTime::parse_from_rfc3339(&args[1])?.with_timezone(&Utc);
            let end_date = DateTime::parse_from_rfc3339(&args[2])?.with_timezone(&Utc);
            Ok(Command::CreateEpoch { name, start_date, end_date })
        },
        "activate-epoch" => {
            if args.len() != 1 {
                return Err("Usage: activate-epoch <name>".into());
            }
            Ok(Command::ActivateEpoch { name: args[0].clone() })
        },
        "set-epoch-reward" => {
            if args.len() != 2 {
                return Err("Usage: set-epoch-reward <token> <amount>".into());
            }
            let token = args[0].clone();
            let amount = args[1].parse()?;
            Ok(Command::SetEpochReward { token, amount })
        },
        "update-team" => {
            if args.len() < 2 {
                return Err("Usage: update-team <name> [--new-name <name>] [--representative <name>] [--status <status>] [--revenue <rev1> <rev2> <rev3>]".into());
            }
            let team_name = args[0].clone();
            let mut updates = UpdateTeamDetails {
                name: None,
                representative: None,
                status: None,
                trailing_monthly_revenue: None,
            };
            let mut i = 1;
            while i < args.len() {
                match args[i].as_str() {
                    "--new-name" => {
                        updates.name = Some(args[i+1].clone());
                        i += 2;
                    },
                    "--representative" => {
                        updates.representative = Some(args[i+1].clone());
                        i += 2;
                    },
                    "--status" => {
                        updates.status = Some(args[i+1].clone());
                        i += 2;
                    },
                    "--revenue" => {
                        updates.trailing_monthly_revenue = Some(vec![
                            args[i+1].parse()?,
                            args[i+2].parse()?,
                            args[i+3].parse()?
                        ]);
                        i += 4;
                    },
                    _ => return Err(format!("Unknown option: {}", args[i]).into()),
                }
            }
            Ok(Command::UpdateTeam { team_name, updates })
        },
        "add-proposal" => {
            if args.len() < 2 {
                return Err("Usage: add-proposal <title> <url> [--team TeamName] [--amounts ETH:100.5,USD:1000] [--start 2024-01-01] [--end 2024-12-31] [--announced 2024-01-01] [--published 2024-01-01] [--loan true] [--address 0x...]".into());
            }

            let title = args[0].clone();
            let url = args[1].clone();
            let mut team = None;
            let mut amounts = None;
            let mut start_date = None;
            let mut end_date = None;
            let mut announced_at = None;
            let mut published_at = None;
            let mut is_loan = None;
            let mut payment_address = None;

            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--team" => {
                        team = Some(args[i + 1].clone());
                        i += 2;
                    },
                    "--amounts" => {
                        amounts = Some(args[i + 1].split(',')
                            .map(|pair| {
                                let parts: Vec<&str> = pair.split(':').collect();
                                if parts.len() != 2 {
                                    return Err(format!("Invalid amount format: {}. Expected token:amount", pair));
                                }
                                let amount = parts[1].parse::<f64>()
                                    .map_err(|_| format!("Invalid amount {}: {}", parts[1], pair))?;
                                Ok((parts[0].to_string(), amount))
                            })
                            .collect::<Result<HashMap<_, _>, String>>()?);
                        i += 2;
                    },
                    "--start" => {
                        start_date = Some(NaiveDate::parse_from_str(&args[i + 1], "%Y-%m-%d")?);
                        i += 2;
                    },
                    "--end" => {
                        end_date = Some(NaiveDate::parse_from_str(&args[i + 1], "%Y-%m-%d")?);
                        i += 2;
                    },
                    "--announced" => {
                        announced_at = Some(NaiveDate::parse_from_str(&args[i + 1], "%Y-%m-%d")?);
                        i += 2;
                    },
                    "--published" => {
                        published_at = Some(NaiveDate::parse_from_str(&args[i + 1], "%Y-%m-%d")?);
                        i += 2;
                    },
                    "--loan" => {
                        is_loan = Some(args[i + 1].parse()?);
                        i += 2;
                    },
                    "--address" => {
                        payment_address = Some(args[i + 1].clone());
                        i += 2;
                    },
                    _ => return Err(format!("Unknown option: {}", args[i]).into()),
                }
            }

            Ok(Command::AddProposal {
                title,
                url: Some(url),
                budget_request_details: if team.is_some() || amounts.is_some() {
                    Some(BudgetRequestDetailsCommand {
                        team,
                        request_amounts: amounts,
                        start_date,
                        end_date,
                        is_loan,
                        payment_address,
                    })
                } else {
                    None
                },
                announced_at,
                published_at,
                is_historical: None,
            })
        },
        "update-proposal" => {
            if args.len() < 2 {
                return Err("Usage: update-proposal <name> [--title <title>] [--url <url>] [--team <name>] \
                        [--amounts <token:amount>] [--start <date>] [--end <date>] [--announced <date>] \
                        [--published <date>] [--resolved <date>] [--loan <true/false>] [--address <eth_address>]".into());
            }
            let proposal_name = args[0].clone();
            let mut updates = UpdateProposalDetails {
                title: None,
                url: None,
                budget_request_details: None,
                announced_at: None,
                published_at: None,
                resolved_at: None,
            };
            
            let mut i = 1;
            let mut budget_details = BudgetRequestDetailsCommand {
                team: None,
                request_amounts: None,
                start_date: None,
                end_date: None,
                is_loan: None,
                payment_address: None,
            };
            let mut has_budget_changes = false;

            while i < args.len() {
                match args[i].as_str() {
                    // ... existing matches ...
                    "--loan" => {
                        budget_details.is_loan = Some(args[i+1].parse()
                            .map_err(|_| format!("Invalid loan value: {}", args[i+1]))?);
                        has_budget_changes = true;
                        i += 2;
                    },
                    "--address" => {
                        budget_details.payment_address = Some(args[i+1].clone());
                        has_budget_changes = true;
                        i += 2;
                    },
                    _ => return Err(format!("Unknown option: {}", args[i]).into()),
                }
            }

            if has_budget_changes {
                updates.budget_request_details = Some(budget_details);
            }

            Ok(Command::UpdateProposal { proposal_name, updates })
        },
        "import-predefined-raffle" => {
            if args.len() < 5 {
                return Err("Usage: import-predefined-raffle <proposal_name> <counted_teams> <uncounted_teams> <total_counted_seats> <max_earner_seats>".into());
            }
            let proposal_name = args[0].clone();
            let counted_teams: Vec<String> = args[1].split(',').map(String::from).collect();
            let uncounted_teams: Vec<String> = args[2].split(',').map(String::from).collect();
            let total_counted_seats = args[3].parse()?;
            let max_earner_seats = args[4].parse()?;
            Ok(Command::ImportPredefinedRaffle { proposal_name, counted_teams, uncounted_teams, total_counted_seats, max_earner_seats })
        },
        "import-historical-vote" => {
            if args.len() < 5 {
                return Err("Usage: import-historical-vote <proposal_name> <passed> <participating_teams> <non_participating_teams> [<counted_points> <uncounted_points>]".into());
            }
            let proposal_name = args[0].clone();
            let passed = args[1].parse()?;
            let participating_teams: Vec<String> = args[2].split(',').map(String::from).collect();
            let non_participating_teams: Vec<String> = args[3].split(',').map(String::from).collect();
            let counted_points = args.get(4).map(|s| s.parse()).transpose()?;
            let uncounted_points = args.get(5).map(|s| s.parse()).transpose()?;
            Ok(Command::ImportHistoricalVote { proposal_name, passed, participating_teams, non_participating_teams, counted_points, uncounted_points })
        },
        "import-historical-raffle" => {
            if args.len() < 4 {
                return Err("Usage: import-historical-raffle <proposal_name> <initiation_block> <randomness_block> [<team_order>] [<excluded_teams>] [<total_counted_seats>] [<max_earner_seats>]".into());
            }
            let proposal_name = args[0].clone();
            let initiation_block = args[1].parse()?;
            let randomness_block = args[2].parse()?;
            let team_order = args.get(3).map(|s| s.split(',').map(String::from).collect());
            let excluded_teams = args.get(4).map(|s| s.split(',').map(String::from).collect());
            let total_counted_seats = args.get(5).map(|s| s.parse()).transpose()?;
            let max_earner_seats = args.get(6).map(|s| s.parse()).transpose()?;
            Ok(Command::ImportHistoricalRaffle { proposal_name, initiation_block, randomness_block, team_order, excluded_teams, total_counted_seats, max_earner_seats })
        },
        "print-team-report" => Ok(Command::PrintTeamReport),
        "print-epoch-state" => Ok(Command::PrintEpochState),
        "print-team-vote-participation" => {
            if args.len() < 1 || args.len() > 2 {
                return Err("Usage: print-team-vote-participation <team_name> [epoch_name]".into());
            }
            let team_name = args[0].clone();
            let epoch_name = args.get(1).cloned();
            Ok(Command::PrintTeamVoteParticipation { team_name, epoch_name })
        },
        "close-proposal" => {
            if args.len() != 2 {
                return Err("Usage: close-proposal <proposal_name> <resolution>".into());
            }
            let proposal_name = args[0].clone();
            let resolution = args[1].clone();
            Ok(Command::CloseProposal { proposal_name, resolution })
        },
        "create-raffle" => {
            if args.len() < 1 || args.len() > 3 {
                return Err("Usage: create-raffle <proposal_name> [block_offset] [excluded_teams]".into());
            }
            let proposal_name = args[0].clone();
            let block_offset = args.get(1).map(|s| s.parse()).transpose()?;
            let excluded_teams = args.get(2).map(|s| s.split(',').map(String::from).collect());
            Ok(Command::CreateRaffle { proposal_name, block_offset, excluded_teams })
        },
        "create-and-process-vote" => {
            if args.len() < 3 {
                return Err("Usage: create-and-process-vote <proposal_name> <counted_votes> <uncounted_votes> [vote_opened] [vote_closed]".into());
            }
            let proposal_name = args[0].clone();
            let counted_votes: HashMap<String, VoteChoice> = args[1].split(',')
                .map(|s| {
                    let parts: Vec<&str> = s.split(':').collect();
                    (parts[0].to_string(), if parts[1] == "Yes" { VoteChoice::Yes } else { VoteChoice::No })
                })
                .collect();
            let uncounted_votes: HashMap<String, VoteChoice> = args[2].split(',')
                .map(|s| {
                    let parts: Vec<&str> = s.split(':').collect();
                    (parts[0].to_string(), if parts[1] == "Yes" { VoteChoice::Yes } else { VoteChoice::No })
                })
                .collect();
            let vote_opened = args.get(3).map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d")).transpose()?;
            let vote_closed = args.get(4).map(|s| NaiveDate::parse_from_str(s, "%Y-%m-%d")).transpose()?;
            Ok(Command::CreateAndProcessVote { proposal_name, counted_votes, uncounted_votes, vote_opened, vote_closed })
        },
        "generate-reports-for-closed-proposals" => {
            if args.len() != 1 {
                return Err("Usage: generate-reports-for-closed-proposals <epoch_name>".into());
            }
            let epoch_name = args[0].clone();
            Ok(Command::GenerateReportsForClosedProposals { epoch_name })
        },"generate-report-for-proposal" => {
            if args.len() != 1 {
                return Err("Usage: generate-report-for-proposal <proposal_name>".into());
            }
            let proposal_name = args[0].clone();
            Ok(Command::GenerateReportForProposal { proposal_name })
        },
        "print-point-report" => {
            let epoch_name = args.get(0).cloned();
            Ok(Command::PrintPointReport { epoch_name })
        },
        "close-epoch" => {
            let epoch_name = args.get(0).cloned();
            Ok(Command::CloseEpoch { epoch_name })
        },
        "generate-end-of-epoch-report" => {
            if args.len() != 1 {
                return Err("Usage: generate-end-of-epoch-report <epoch_name>".into());
            }
            let epoch_name = args[0].clone();
            Ok(Command::GenerateEndOfEpochReport { epoch_name })
        },
        "run-script" => {
            let script_file_path = args.get(0).cloned();
            Ok(Command::RunScript { script_file_path })
        },
        "generate-unpaid-requests-report" => {
            let output_path = args.get(0).cloned();
            let epoch_name = args.get(1).cloned();
            Ok(Command::GenerateUnpaidRequestsReport { output_path, epoch_name })
        },
        _ => Err(format!("Unknown command: {}", command).into()),
    }
}

pub fn read_script_commands(script_file_path: &str) -> Result<Vec<Command>, Box<dyn Error>> {
    let script_content = fs::read_to_string(script_file_path)?;
    let commands: Vec<Command> = serde_json::from_str(&script_content)?;
    Ok(commands)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::{path::Path, io};
    use tokio::time::timeout;
    use tempfile::TempDir;
    use crate::app_config::TelegramConfig;
    use crate::services::ethereum::MockEthereumService;
    use crate::core::models::VoteResult;

    async fn create_test_budget_system() -> (BudgetSystem, AppConfig) {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("test_state.json").to_str().unwrap().to_string();
    
        let config = AppConfig {
            state_file,
            ipc_path: "/tmp/test_reth.ipc".to_string(),
            future_block_offset: 0,
            script_file: "test_script.json".to_string(),
            default_total_counted_seats: 7,
            default_max_earner_seats: 5,
            default_qualified_majority_threshold: 0.7,
            counted_vote_points: 5,
            uncounted_vote_points: 2,
            telegram: TelegramConfig {
                chat_id: "test_chat_id".to_string(),
                token: "test_token".to_string(),
            },
        };
    
        let ethereum_service = Arc::new(MockEthereumService::new());
        let budget_system = BudgetSystem::new(config.clone(), ethereum_service, None).await.unwrap();
    
        (budget_system, config)
    }

    async fn create_test_budget_system_with_proposal() -> (BudgetSystem, AppConfig, Uuid) {
        let (mut budget_system, config) = create_test_budget_system().await;
    
        // Create and activate an epoch
        let start_date = Utc::now();
        let end_date = start_date + chrono::Duration::days(30);
        let epoch_id = budget_system.create_epoch("Test Epoch", start_date, end_date).unwrap();
        budget_system.activate_epoch(epoch_id).unwrap();
    
        // Create a proposal
        let proposal_id = budget_system.add_proposal(
            "Test Proposal".to_string(),
            Some("http://example.com".to_string()),
            None,
            Some(Utc::now().date_naive()),
            Some(Utc::now().date_naive()),
            None
        ).unwrap();
    
        (budget_system, config, proposal_id)
    }
    
    #[tokio::test]
    async fn test_create_epoch_command() {
        let (mut budget_system, config) = create_test_budget_system().await;
    
        let start_date = Utc::now();
        let end_date = start_date + chrono::Duration::days(30);
    
        let command = Command::CreateEpoch {
            name: "Test Epoch".to_string(),
            start_date,
            end_date,
        };
        
        let mut stdout = io::sink();
        let result = execute_command(&mut budget_system, command, &config, &mut stdout).await;
        assert!(result.is_ok());
        assert_eq!(budget_system.state().epochs().len(), 1);
    }
    
    #[tokio::test]
    async fn test_activate_epoch_command() {
        let (mut budget_system, config) = create_test_budget_system().await;
    
        let start_date = Utc::now();
        let end_date = start_date + chrono::Duration::days(30);
    
        let epoch_id = budget_system.create_epoch("Test Epoch", start_date, end_date).unwrap();
    
        let command = Command::ActivateEpoch {
            name: "Test Epoch".to_string(),
        };
        
        let mut stdout = io::sink();
        let result = execute_command(&mut budget_system, command, &config, &mut stdout).await;
        assert!(result.is_ok());
        assert_eq!(budget_system.state().current_epoch(), Some(epoch_id));
        assert!(budget_system.state().epochs().get(&epoch_id).unwrap().is_active());
    }
    
    #[tokio::test]
    async fn test_set_epoch_reward_command() {
        let (mut budget_system, config) = create_test_budget_system().await;
    
        let start_date = Utc::now();
        let end_date = start_date + chrono::Duration::days(30);
    
        let epoch_id = budget_system.create_epoch("Test Epoch", start_date, end_date).unwrap();
        budget_system.activate_epoch(epoch_id).unwrap();
    
        let command = Command::SetEpochReward {
            token: "ETH".to_string(),
            amount: 100.0,
        };
        
        let mut stdout = io::sink();
        let result = execute_command(&mut budget_system, command, &config, &mut stdout).await;
        assert!(result.is_ok());
    
        let epoch = budget_system.state().epochs().get(&epoch_id).unwrap();
        assert_eq!(epoch.reward().unwrap().token(), "ETH");
        assert_eq!(epoch.reward().unwrap().amount(), 100.0);
    }
    
    #[tokio::test]
    async fn test_add_team_command() {
        let (mut budget_system, config) = create_test_budget_system().await;
    
        let command = Command::AddTeam {
            name: "Test Team".to_string(),
            representative: "John Doe".to_string(),
            trailing_monthly_revenue: Some(vec![1000, 2000, 3000]),
            address: None
        };
        
        let mut stdout = io::sink();
        let result = execute_command(&mut budget_system, command, &config, &mut stdout).await;
        assert!(result.is_ok());
        assert_eq!(budget_system.state().current_state().teams().len(), 1);
    
        let team = budget_system.state().current_state().teams().values().next().unwrap();
        assert_eq!(team.name(), "Test Team");
        assert_eq!(team.representative(), "John Doe");
        assert!(matches!(team.status(), TeamStatus::Earner { .. }));
    }
    
    #[tokio::test]
    async fn test_update_team_command() {
        let temp_dir = TempDir::new().unwrap();
        let (mut budget_system, config) = create_test_budget_system().await;
        
        // Create a team
        budget_system.create_team("Test Team".to_string(), "John Doe".to_string(), Some(vec![1000]), None).unwrap();

        let command = Command::UpdateTeam {
            team_name: "Test Team".to_string(),
            updates: UpdateTeamDetails {
                name: Some("Updated Team".to_string()),
                representative: Some("Jane Doe".to_string()),
                status: Some("Supporter".to_string()),
                trailing_monthly_revenue: None,
            },
        };

        let mut stdout = io::sink();
        let result = execute_command(&mut budget_system, command, &config, &mut stdout).await;
        assert!(result.is_ok());

        let team_id = budget_system.get_team_id_by_name("Updated Team").unwrap();
        let updated_team = budget_system.get_team(&team_id).unwrap();
        assert_eq!(updated_team.name(), "Updated Team");
        assert_eq!(updated_team.representative(), "Jane Doe");
        assert!(matches!(updated_team.status(), TeamStatus::Supporter));
    }

    #[tokio::test]
    async fn test_invalid_command_execution() {
        let (mut budget_system, config) = create_test_budget_system().await;
    
        let command = Command::ActivateEpoch {
            name: "Non-existent Epoch".to_string(),
        };
        
        let mut stdout = io::sink();
        let result = execute_command(&mut budget_system, command, &config, &mut stdout).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_add_proposal_command() {
        let args = vec![
            "robokitty".to_string(),
            "add-proposal".to_string(),
            "Test Proposal".to_string(),
            "https://example.com".to_string(),
            "--team".to_string(),
            "Team1".to_string(),
            "--amounts".to_string(),
            "ETH:100,USD:1000".to_string(),
            "--loan".to_string(),
            "true".to_string(),
            "--address".to_string(),
            "0x742d35Cc6634C0532925a3b844Bc454e4438f44e".to_string(),
        ];

        let command = parse_cli_args(&args).unwrap();
        match command {
            Command::AddProposal { title, url, budget_request_details, .. } => {
                assert_eq!(title, "Test Proposal");
                assert_eq!(url, Some("https://example.com".to_string()));
                
                let details = budget_request_details.unwrap();
                assert_eq!(details.team, Some("Team1".to_string()));
                assert!(details.is_loan.unwrap());
                assert_eq!(details.payment_address, 
                    Some("0x742d35Cc6634C0532925a3b844Bc454e4438f44e".to_string()));
                
                let amounts = details.request_amounts.unwrap();
                assert_eq!(amounts.get("ETH").unwrap(), &100.0);
                assert_eq!(amounts.get("USD").unwrap(), &1000.0);
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[tokio::test]
    async fn test_update_proposal_command() {
        let (mut budget_system, config, proposal_id) = create_test_budget_system_with_proposal().await;

        let command = Command::UpdateProposal {
            proposal_name: "Test Proposal".to_string(),
            updates: UpdateProposalDetails {
                title: Some("Updated Proposal".to_string()),
                url: None,
                budget_request_details: None,
                announced_at: None,
                published_at: None,
                resolved_at: None,
            },
        };

        let mut stdout = io::sink();
        let result = execute_command(&mut budget_system, command, &config, &mut stdout).await;
        assert!(result.is_ok());

        let updated_proposal = budget_system.state().proposals().get(&proposal_id).unwrap();
        assert_eq!(updated_proposal.title(), "Updated Proposal");
    }

    #[tokio::test]
    async fn test_close_proposal_command() {
        let (mut budget_system, config, proposal_id) = create_test_budget_system_with_proposal().await;

        let command = Command::CloseProposal {
            proposal_name: "Test Proposal".to_string(),
            resolution: "Approved".to_string(),
        };

        let mut stdout = io::sink();
        let result = execute_command(&mut budget_system, command, &config, &mut stdout).await;
        assert!(result.is_ok());

        let closed_proposal = budget_system.state().proposals().get(&proposal_id).unwrap();
        assert!(closed_proposal.is_closed());
        assert_eq!(closed_proposal.resolution(), Some(Resolution::Approved));
    }

    #[tokio::test]
    async fn test_create_raffle_command() {
        let (mut budget_system, config, _) = create_test_budget_system_with_proposal().await;

        let command = Command::CreateRaffle {
            proposal_name: "Test Proposal".to_string(),
            block_offset: None,  // Remove block offset
            excluded_teams: None,
        };

        let mut stdout = io::sink();
        let result = execute_command(&mut budget_system, command, &config, &mut stdout).await;
        assert!(result.is_ok());
        assert_eq!(budget_system.state().raffles().len(), 1);

        // Verify that the raffle has been finalized immediately
        let raffle = budget_system.state().raffles().values().next().unwrap();
        assert!(raffle.is_completed());
    }

    #[tokio::test]
    async fn test_import_predefined_raffle_command() {
        let (mut budget_system, config, _) = create_test_budget_system_with_proposal().await;

        // Add some teams
        budget_system.create_team("Team 1".to_string(), "Rep 1".to_string(), Some(vec![1000]), None).unwrap();
        budget_system.create_team("Team 2".to_string(), "Rep 2".to_string(), Some(vec![2000]), None).unwrap();

        let command = Command::ImportPredefinedRaffle {
            proposal_name: "Test Proposal".to_string(),
            counted_teams: vec!["Team 1".to_string()],
            uncounted_teams: vec!["Team 2".to_string()],
            total_counted_seats: 1,
            max_earner_seats: 1,
        };

        let mut stdout = io::sink();
        let result = execute_command(&mut budget_system, command, &config, &mut stdout).await;
        assert!(result.is_ok());
        assert_eq!(budget_system.state().raffles().len(), 1);

        let raffle = budget_system.state().raffles().values().next().unwrap();
        assert_eq!(raffle.result().unwrap().counted().len(), 1);
        assert_eq!(raffle.result().unwrap().uncounted().len(), 1);
    }


    #[tokio::test]
    async fn test_create_and_process_vote_command() {
        let (mut budget_system, config, _) = create_test_budget_system_with_proposal().await;

        // Create a raffle first
        let create_raffle_command = Command::CreateRaffle {
            proposal_name: "Test Proposal".to_string(),
            block_offset: None,
            excluded_teams: None,
        };
        let mut stdout = io::sink();
        execute_command(&mut budget_system, create_raffle_command, &config, &mut stdout).await.unwrap();

        // Add some teams
        budget_system.create_team("Team 1".to_string(), "Rep 1".to_string(), Some(vec![1000]), None).unwrap();
        budget_system.create_team("Team 2".to_string(), "Rep 2".to_string(), Some(vec![2000]), None).unwrap();

        // Get the raffle result to determine which teams are counted and uncounted
        let raffle = budget_system.state().raffles().values().next().unwrap();
        let raffle_result = raffle.result().unwrap();
        let counted_teams: Vec<String> = raffle_result.counted().iter()
            .filter_map(|&id| budget_system.state().current_state().teams().get(&id))
            .map(|team| team.name().to_string())
            .collect();
        let uncounted_teams: Vec<String> = raffle_result.uncounted().iter()
            .filter_map(|&id| budget_system.state().current_state().teams().get(&id))
            .map(|team| team.name().to_string())
            .collect();

        println!("Counted teams: {:?}", counted_teams);
        println!("Uncounted teams: {:?}", uncounted_teams);

        let command = Command::CreateAndProcessVote {
            proposal_name: "Test Proposal".to_string(),
            counted_votes: counted_teams.into_iter().map(|name| (name, VoteChoice::Yes)).collect(),
            uncounted_votes: uncounted_teams.into_iter().map(|name| (name, VoteChoice::No)).collect(),
            vote_opened: Some(Utc::now().date_naive()),
            vote_closed: Some(Utc::now().date_naive()),
        };

        let mut stdout = io::sink();
        let result = execute_command(&mut budget_system, command, &config, &mut stdout).await;
        assert!(result.is_ok(), "Failed to execute CreateAndProcessVote: {:?}", result);
        assert_eq!(budget_system.state().votes().len(), 1);

        let vote = budget_system.state().votes().values().next().unwrap();
        assert!(vote.is_closed());
    }

    #[tokio::test]
    async fn test_integration_complete_workflow() {
        let (mut budget_system, config) = create_test_budget_system().await;

        // Stage 1: Create and activate an epoch
        let start_date = Utc::now();
        let end_date = start_date + chrono::Duration::days(30);
        let create_epoch_command = Command::CreateEpoch {
            name: "Test Epoch".to_string(),
            start_date,
            end_date,
        };
        let mut stdout = io::sink();
        execute_command(&mut budget_system, create_epoch_command, &config, &mut stdout).await.unwrap();

        let activate_epoch_command = Command::ActivateEpoch {
            name: "Test Epoch".to_string(),
        };
        execute_command(&mut budget_system, activate_epoch_command, &config, &mut stdout).await.unwrap();

        // Stage 2: Add teams (5 earners and 5 supporters)
        for i in 1..=10 {
            let team_type = if i <= 5 { "Earner" } else { "Supporter" };
            let add_team_command = Command::AddTeam {
                name: format!("Team {}", i),
                representative: format!("Rep {}", i),
                trailing_monthly_revenue: if i <= 5 { Some(vec![1000 * i, 2000 * i, 3000 * i]) } else { None },
                address: None,
            };
            execute_command(&mut budget_system, add_team_command, &config, &mut stdout).await
                .unwrap_or_else(|e| panic!("Failed to add team {}: {}", i, e));
        }

        // Verify teams were created correctly
        assert_eq!(budget_system.state().current_state().teams().len(), 10, "Expected 10 teams to be created");

        // Stage 3: Create a proposal
        let add_proposal_command = Command::AddProposal {
            title: "Test Proposal".to_string(),
            url: Some("http://example.com".to_string()),
            budget_request_details: None,
            announced_at: Some(start_date.date_naive()),
            published_at: Some(start_date.date_naive()),
            is_historical: None,
        };
        execute_command(&mut budget_system, add_proposal_command, &config, &mut stdout).await.unwrap();

        // Stage 4: Create and verify raffle
        let create_raffle_command = Command::CreateRaffle {
            proposal_name: "Test Proposal".to_string(),
            block_offset: None,
            excluded_teams: None,
        };
        execute_command(&mut budget_system, create_raffle_command, &config, &mut stdout).await.unwrap();

        let raffle = budget_system.state().raffles().values().next()
            .expect("Raffle should exist after creation");
        let raffle_result = raffle.result()
            .expect("Raffle should have results");

        // Get team assignments from raffle
        let counted_teams: Vec<String> = raffle_result.counted().iter()
            .filter_map(|&id| budget_system.state().current_state().teams().get(&id))
            .map(|team| team.name().to_string())
            .collect();
        let uncounted_teams: Vec<String> = raffle_result.uncounted().iter()
            .filter_map(|&id| budget_system.state().current_state().teams().get(&id))
            .map(|team| team.name().to_string())
            .collect();

        // Stage 5: Create and process vote
        let vote_command = Command::CreateAndProcessVote {
            proposal_name: "Test Proposal".to_string(),
            counted_votes: counted_teams.into_iter().map(|name| (name, VoteChoice::Yes)).collect(),
            uncounted_votes: uncounted_teams.into_iter().map(|name| (name, VoteChoice::No)).collect(),
            vote_opened: Some(start_date.date_naive()),
            vote_closed: Some(end_date.date_naive()),
        };
        execute_command(&mut budget_system, vote_command, &config, &mut stdout).await.unwrap();

        // Stage 6: Generate reports
        let generate_report_command = Command::GenerateReportForProposal {
            proposal_name: "Test Proposal".to_string(),
        };
        execute_command(&mut budget_system, generate_report_command, &config, &mut stdout).await.unwrap();

        let print_point_report_command = Command::PrintPointReport { epoch_name: None };
        execute_command(&mut budget_system, print_point_report_command, &config, &mut stdout).await.unwrap();

        // Stage 7: Verify final state
        assert_eq!(budget_system.state().epochs().len(), 1, "Should have exactly one epoch");
        assert_eq!(budget_system.state().current_state().teams().len(), 10, "Should have 10 teams");
        assert_eq!(budget_system.state().proposals().len(), 1, "Should have one proposal");
        assert_eq!(budget_system.state().raffles().len(), 1, "Should have one raffle");
        assert_eq!(budget_system.state().votes().len(), 1, "Should have one vote");

        let proposal = budget_system.state().proposals().values().next()
            .expect("Proposal should exist");
        assert!(proposal.is_closed(), "Proposal should be closed after voting");

        // Verify vote results
        let vote = budget_system.state().votes().values().next()
            .expect("Vote should exist");
        
        assert!(vote.is_closed(), "Vote should be closed");
        
        match vote.result() {
            Some(VoteResult::Formal { counted, uncounted, passed }) => {
                assert_eq!(counted.yes(), config.default_total_counted_seats as u32, 
                    "Expected {} 'Yes' votes from counted teams", config.default_total_counted_seats);
                assert_eq!(counted.no(), 0, "Expected 0 'No' votes from counted teams");
                assert_eq!(uncounted.yes(), 0, "Expected 0 'Yes' votes from uncounted teams");
                assert_eq!(uncounted.no(), 3, "Expected 3 'No' votes from uncounted teams");
                assert!(passed, "Vote should have passed");
            }
            _ => panic!("Expected a formal vote result"),
        }

        match vote.vote_type() {
            VoteType::Formal { total_eligible_seats, threshold, .. } => {
                let (counted, _) = vote.vote_counts()
                    .expect("Vote counts should be available");
                let yes_percentage = counted.yes() as f64 / *total_eligible_seats as f64;
                let expected_resolution = if yes_percentage >= *threshold {
                    Resolution::Approved
                } else {
                    Resolution::Rejected
                };
                assert_eq!(proposal.resolution(), Some(expected_resolution), 
                    "Proposal resolution should match the voting outcome");
            }
            _ => panic!("Expected a formal vote type"),
        }
    }

    #[tokio::test]
    async fn test_print_point_report_command() {
        let (mut budget_system, config, _) = create_test_budget_system_with_proposal().await;

        let command = Command::PrintPointReport { epoch_name: None };

        let mut stdout = io::sink();
        let result = execute_command(&mut budget_system, command, &config, &mut stdout).await;
        assert!(result.is_ok());
        // The actual content of the report is printed to stdout, so we can't easily verify it in this test
    }

    #[tokio::test]
    async fn test_non_existent_entity_commands() {
        let (mut budget_system, config) = create_test_budget_system().await;

        // Test activating non-existent epoch
        let command = Command::ActivateEpoch {
            name: "Non-existent Epoch".to_string(),
        };  
        let mut stdout = io::sink();
        let result = execute_command(&mut budget_system, command, &config, &mut stdout).await;
        assert!(result.is_err());

        // Test updating non-existent proposal
        let command = Command::UpdateProposal {
            proposal_name: "Non-existent Proposal".to_string(),
            updates: UpdateProposalDetails {
                title: Some("Updated Title".to_string()),
                url: None,
                budget_request_details: None,
                announced_at: None,
                published_at: None,
                resolved_at: None,
            },
        };  
        let mut stdout = io::sink();
        let result = execute_command(&mut budget_system, command, &config, &mut stdout).await;
        assert!(result.is_err());

    }

    #[tokio::test]
    async fn test_invalid_parameter_commands() {
        let (mut budget_system, config) = create_test_budget_system().await;

        // Test creating epoch with end date before start date
        let end_date = Utc::now();
        let start_date = end_date + chrono::Duration::days(1);
        let command = Command::CreateEpoch {
            name: "Invalid Epoch".to_string(),
            start_date,
            end_date,
        };  
        let mut stdout = io::sink();
        let result = execute_command(&mut budget_system, command, &config, &mut stdout).await;
        assert!(result.is_err());

        // Test creating team with invalid status
        let command = Command::AddTeam {
            name: "Invalid Team".to_string(),
            representative: "John Doe".to_string(),
            trailing_monthly_revenue: Some(vec![]),
            address: None,
        };  
        let mut stdout = io::sink();
        let result = execute_command(&mut budget_system, command, &config, &mut stdout).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_incorrect_command_order() {
        let (mut budget_system, config) = create_test_budget_system().await;

        // Try to create a proposal before creating and activating an epoch
        let command = Command::AddProposal {
            title: "Invalid Proposal".to_string(),
            url: None,
            budget_request_details: None,
            announced_at: None,
            published_at: None,
            is_historical: None,
        };  
        let mut stdout = io::sink();
        let result = execute_command(&mut budget_system, command, &config, &mut stdout).await;
        assert!(result.is_err());

        // Try to create a raffle before creating a proposal
        let command = Command::CreateRaffle {
            proposal_name: "Non-existent Proposal".to_string(),
            block_offset: None,
            excluded_teams: None,
        };  
        let mut stdout = io::sink();
        let result = execute_command(&mut budget_system, command, &config, &mut stdout).await;
        assert!(result.is_err());
    }

    #[test]
    fn test_add_proposal_with_loan_and_address() {
        let args = vec![
            "robokitty".to_string(),
            "add-proposal".to_string(),
            "\"Test Proposal\"".to_string(),
            "https://example.com".to_string(),
            "--team".to_string(),
            "Team1".to_string(),
            "--amounts".to_string(),
            "ETH:100".to_string(),
            "--loan".to_string(),
            "true".to_string(),
            "--address".to_string(),
            "0x742d35Cc6634C0532925a3b844Bc454e4438f44e".to_string(),
        ];

        let command = parse_cli_args(&args).unwrap();
        match command {
            Command::AddProposal { budget_request_details, .. } => {
                let details = budget_request_details.unwrap();
                assert_eq!(details.team, Some("Team1".to_string()));
                assert!(details.is_loan.unwrap());
                assert_eq!(details.payment_address, 
                    Some("0x742d35Cc6634C0532925a3b844Bc454e4438f44e".to_string()));
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_update_proposal_with_loan_and_address() {
        let args = vec![
            "robokitty".to_string(),
            "update-proposal".to_string(),
            "Test Proposal".to_string(),
            "--loan".to_string(),
            "true".to_string(),
            "--address".to_string(),
            "0x742d35Cc6634C0532925a3b844Bc454e4438f44e".to_string(),
        ];

        let command = parse_cli_args(&args).unwrap();
        match command {
            Command::UpdateProposal { updates, .. } => {
                let details = updates.budget_request_details.unwrap();
                assert!(details.is_loan.unwrap());
                assert_eq!(details.payment_address,
                    Some("0x742d35Cc6634C0532925a3b844Bc454e4438f44e".to_string()));
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_add_proposal_invalid_inputs() {
        // Test invalid loan value
        let args = vec![
            "robokitty".to_string(),
            "add-proposal".to_string(),
            "Test".to_string(),
            "https://example.com".to_string(),
            "--loan".to_string(),
            "invalid".to_string(),
        ];
        assert!(parse_cli_args(&args).is_err());

        // Test invalid date
        let args = vec![
            "robokitty".to_string(),
            "add-proposal".to_string(),
            "Test".to_string(),
            "https://example.com".to_string(),
            "--start".to_string(),
            "invalid-date".to_string(),
        ];
        assert!(parse_cli_args(&args).is_err());
    }

    #[test]
    fn test_parse_eth_address() {
        assert!(parse_eth_address("0x742d35Cc6634C0532925a3b844Bc454e4438f44e").is_ok());
        assert!(parse_eth_address("742d35Cc6634C0532925a3b844Bc454e4438f44e").is_err()); // no 0x
        assert!(parse_eth_address("0x742d35").is_err()); // too short
        assert!(parse_eth_address("0x742d35Cc6634C0532925a3b844Bc454e4438f44eXX").is_err()); // invalid hex
    }

    #[test]
    fn test_add_team_command_basic() {
        let args = vec![
            "robokitty".to_string(),
            "add-team".to_string(),
            "--name".to_string(),
            "Test Team".to_string(),
            "--representative".to_string(),
            "John Doe".to_string(),
        ];
        
        let cmd = parse_cli_args(&args).unwrap();
        match cmd {
            Command::AddTeam { name, representative, trailing_monthly_revenue, address } => {
                assert_eq!(name, "Test Team");
                assert_eq!(representative, "John Doe");
                assert!(trailing_monthly_revenue.is_none());
                assert!(address.is_none());
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_add_team_command_full() {
        let args = vec![
            "robokitty".to_string(),
            "add-team".to_string(),
            "--name".to_string(),
            "Test Team".to_string(),
            "--representative".to_string(),
            "John Doe".to_string(),
            "--revenue".to_string(),
            "1000,2000,3000".to_string(),
            "--address".to_string(),
            "0x742d35Cc6634C0532925a3b844Bc454e4438f44e".to_string(),
        ];
        
        let cmd = parse_cli_args(&args).unwrap();
        match cmd {
            Command::AddTeam { name, representative, trailing_monthly_revenue, address } => {
                assert_eq!(name, "Test Team");
                assert_eq!(representative, "John Doe");
                assert_eq!(trailing_monthly_revenue, Some(vec![1000, 2000, 3000]));
                assert_eq!(address, Some("0x742d35Cc6634C0532925a3b844Bc454e4438f44e".to_string()));
            },
            _ => panic!("Wrong command type"),
        }
    }

    #[test]
    fn test_add_team_command_invalid_revenue() {
        let args = vec![
            "robokitty".to_string(),
            "add-team".to_string(),
            "-n".to_string(),
            "Test Team".to_string(),
            "-r".to_string(),
            "John Doe".to_string(),
            "--revenue".to_string(),
            "1000,2000,3000,4000".to_string(),
        ];
        
        assert!(parse_cli_args(&args).is_err());
    }

    #[test]
    fn test_add_team_command_invalid_address() {
        let args = vec![
            "robokitty".to_string(),
            "add-team".to_string(),
            "-n".to_string(),
            "Test Team".to_string(),
            "-r".to_string(),
            "John Doe".to_string(),
            "--address".to_string(),
            "invalid".to_string(),
        ];
        
        assert!(parse_cli_args(&args).is_err());
    }

}