// src/commands/cli.rs
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, io::Write};
use std::{fs, error::Error};
use uuid::Uuid;
use tokio::time::Duration;

use crate::core::models::{
    BudgetRequestDetails, PaymentStatus, Resolution, TeamStatus, VoteChoice, VoteType, VoteParticipation, NameMatches
};
use crate::core::budget_system::BudgetSystem;
use crate::app_config::AppConfig;
use super::common::{BudgetRequestDetailsCommand, Command, CommandExecutor, UpdateTeamDetails, UpdateProposalDetails};

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
        "add-team" => {
            if args.len() < 2 {
                return Err("Usage: add-team <name> <representative> [revenue1 revenue2 revenue3]".into());
            }
            let name = args[0].clone();
            let representative = args[1].clone();
            let trailing_monthly_revenue = if args.len() > 2 {
                Some(args[2..].iter().map(|s| s.parse().unwrap()).collect())
            } else {
                None
            };
            Ok(Command::AddTeam { name, representative, trailing_monthly_revenue })
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
                return Err("Usage: add-proposal <title> <url> [--budget-request <team> <amount> <token> <start_date> <end_date>] [--announced <date>] [--published <date>] [--historical]".into());
            }
            let title = args[0].clone();
            let url = Some(args[1].clone());
            let mut budget_request_details = None;
            let mut announced_at = None;
            let mut published_at = None;
            let mut is_historical = None;
            let mut i = 2;
            while i < args.len() {
                match args[i].as_str() {
                    "--budget-request" => {
                        budget_request_details = Some(BudgetRequestDetailsCommand {
                            team: Some(args[i+1].clone()),
                            request_amounts: Some(HashMap::from([(args[i+3].clone(), args[i+2].parse()?)])),
                            start_date: Some(NaiveDate::parse_from_str(&args[i+4], "%Y-%m-%d")?),
                            end_date: Some(NaiveDate::parse_from_str(&args[i+5], "%Y-%m-%d")?),
                            payment_status: None,
                        });
                        i += 6;
                    },
                    "--announced" => {
                        announced_at = Some(NaiveDate::parse_from_str(&args[i+1], "%Y-%m-%d")?);
                        i += 2;
                    },
                    "--published" => {
                        published_at = Some(NaiveDate::parse_from_str(&args[i+1], "%Y-%m-%d")?);
                        i += 2;
                    },
                    "--historical" => {
                        is_historical = Some(true);
                        i += 1;
                    },
                    _ => return Err(format!("Unknown option: {}", args[i]).into()),
                }
            }
            Ok(Command::AddProposal { title, url, budget_request_details, announced_at, published_at, is_historical })
        },
        "update-proposal" => {
            if args.len() < 2 {
                return Err("Usage: update-proposal <name> [--title <title>] [--url <url>] [--budget-request <team> <amount> <token> <start_date> <end_date>] [--announced <date>] [--published <date>] [--resolved <date>]".into());
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
            while i < args.len() {
                match args[i].as_str() {
                    "--title" => {
                        updates.title = Some(args[i+1].clone());
                        i += 2;
                    },
                    "--url" => {
                        updates.url = Some(args[i+1].clone());
                        i += 2;
                    },
                    "--budget-request" => {
                        updates.budget_request_details = Some(BudgetRequestDetailsCommand {
                            team: Some(args[i+1].clone()),
                            request_amounts: Some(HashMap::from([(args[i+3].clone(), args[i+2].parse()?)])),
                            start_date: Some(NaiveDate::parse_from_str(&args[i+4], "%Y-%m-%d")?),
                            end_date: Some(NaiveDate::parse_from_str(&args[i+5], "%Y-%m-%d")?),
                            payment_status: None,
                        });
                        i += 6;
                    },
                    "--announced" => {
                        updates.announced_at = Some(NaiveDate::parse_from_str(&args[i+1], "%Y-%m-%d")?);
                        i += 2;
                    },
                    "--published" => {
                        updates.published_at = Some(NaiveDate::parse_from_str(&args[i+1], "%Y-%m-%d")?);
                        i += 2;
                    },
                    "--resolved" => {
                        updates.resolved_at = Some(NaiveDate::parse_from_str(&args[i+1], "%Y-%m-%d")?);
                        i += 2;
                    },
                    _ => return Err(format!("Unknown option: {}", args[i]).into()),
                }
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
    
        let ethereum_service = Arc::new(MockEthereumService);
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
        
        let mut stdout = io::stdout();
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
        
        let mut stdout = io::stdout();
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
        
        let mut stdout = io::stdout();
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
        };
        
        let mut stdout = io::stdout();
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
        budget_system.create_team("Test Team".to_string(), "John Doe".to_string(), Some(vec![1000])).unwrap();

        let command = Command::UpdateTeam {
            team_name: "Test Team".to_string(),
            updates: UpdateTeamDetails {
                name: Some("Updated Team".to_string()),
                representative: Some("Jane Doe".to_string()),
                status: Some("Supporter".to_string()),
                trailing_monthly_revenue: None,
            },
        };

        let mut stdout = io::stdout();
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
        
        let mut stdout = io::stdout();
        let result = execute_command(&mut budget_system, command, &config, &mut stdout).await;
        assert!(result.is_err());
    }


    #[tokio::test]
    async fn test_add_proposal_command() {
        let (mut budget_system, config) = create_test_budget_system().await;

        // Create and activate an epoch
        let start_date = Utc::now();
        let end_date = start_date + chrono::Duration::days(30);
        let epoch_id = budget_system.create_epoch("Test Epoch", start_date, end_date).unwrap();
        budget_system.activate_epoch(epoch_id).unwrap();

        let command = Command::AddProposal {
            title: "New Proposal".to_string(),
            url: Some("http://example.com".to_string()),
            budget_request_details: None,
            announced_at: Some(Utc::now().date_naive()),
            published_at: Some(Utc::now().date_naive()),
            is_historical: Some(false),
        };

        let mut stdout = io::stdout();
        let result = execute_command(&mut budget_system, command, &config, &mut stdout).await;
        assert!(result.is_ok());
        assert_eq!(budget_system.state().proposals().len(), 1);

        let proposal = budget_system.state().proposals().values().next().unwrap();
        assert_eq!(proposal.title(), "New Proposal");
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

        let mut stdout = io::stdout();
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

        let mut stdout = io::stdout();
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

        let mut stdout = io::stdout();
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
        budget_system.create_team("Team 1".to_string(), "Rep 1".to_string(), Some(vec![1000])).unwrap();
        budget_system.create_team("Team 2".to_string(), "Rep 2".to_string(), Some(vec![2000])).unwrap();

        let command = Command::ImportPredefinedRaffle {
            proposal_name: "Test Proposal".to_string(),
            counted_teams: vec!["Team 1".to_string()],
            uncounted_teams: vec!["Team 2".to_string()],
            total_counted_seats: 1,
            max_earner_seats: 1,
        };

        let mut stdout = io::stdout();
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
        let mut stdout = io::stdout();
        execute_command(&mut budget_system, create_raffle_command, &config, &mut stdout).await.unwrap();

        // Add some teams
        budget_system.create_team("Team 1".to_string(), "Rep 1".to_string(), Some(vec![1000])).unwrap();
        budget_system.create_team("Team 2".to_string(), "Rep 2".to_string(), Some(vec![2000])).unwrap();

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

        let mut stdout = io::stdout();
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

        let mut stdout = io::stdout();
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
        let mut stdout = io::stdout();
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
        let mut stdout = io::stdout();
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
        let mut stdout = io::stdout();
        let result = execute_command(&mut budget_system, command, &config, &mut stdout).await;
        assert!(result.is_err());

        // Test creating team with invalid status
        let command = Command::AddTeam {
            name: "Invalid Team".to_string(),
            representative: "John Doe".to_string(),
            trailing_monthly_revenue: Some(vec![]),  // Empty revenue for Earner status
        };  
        let mut stdout = io::stdout();
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
        let mut stdout = io::stdout();
        let result = execute_command(&mut budget_system, command, &config, &mut stdout).await;
        assert!(result.is_err());

        // Try to create a raffle before creating a proposal
        let command = Command::CreateRaffle {
            proposal_name: "Non-existent Proposal".to_string(),
            block_offset: None,
            excluded_teams: None,
        };  
        let mut stdout = io::stdout();
        let result = execute_command(&mut budget_system, command, &config, &mut stdout).await;
        assert!(result.is_err());
    }

}