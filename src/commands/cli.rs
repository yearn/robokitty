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

#[derive(Parser)]
#[command(name = "robokitty")]
#[command(about = "Budget system management CLI", long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Manage teams
    Team {
        #[command(subcommand)]
        command: TeamCommands,
    },
    
    /// Manage epochs  
    Epoch {
        #[command(subcommand)]
        command: EpochCommands, 
    },
    /// Manage proposals
    Proposal {
        #[command(subcommand)]
        command: ProposalCommands,
    },
    /// Manage voting
   Vote {
    #[command(subcommand)]
    command: VoteCommands,
    },
    /// Manage raffles 
    Raffle {
        #[command(subcommand)]
        command: RaffleCommands,
    },
    /// Print reports
    Report {
        #[command(subcommand)]
        command: ReportCommands,
    },
    /// Import existing raffles and votes
    Import {
        #[command(subcommand)]
        command: ImportCommands,
    },
    /// Run JSON script
    RunScript {
        script_file_path: Option<String>,
    }, 
}

#[derive(Subcommand)]
pub enum TeamCommands {
    /// Add a new team to the system
    Add {
        /// Team's display name
        #[arg(long, value_name = "NAME")]
        name: String,
        
        /// Team's representative contact
        #[arg(long, value_name = "REPRESENTATIVE")] 
        representative: String,
        
        /// Monthly revenue values (comma separated)
        #[arg(long, value_name = "REVENUE")]
        revenue: Option<String>,
        
        /// Ethereum payment address
        #[arg(long, value_name = "ADDRESS")]
        address: Option<String>,
    },

    /// Update an existing team
    Update {
        /// Current team name
        #[arg(value_name = "TEAM")]
        name: String,
        
        /// New team name
        #[arg(long, value_name = "NAME")]
        new_name: Option<String>,
        
        /// New representative
        #[arg(long, value_name = "REPRESENTATIVE")]
        representative: Option<String>,
        
        /// New status (Earner/Supporter/Inactive)
        #[arg(long, value_name = "STATUS")]
        status: Option<String>,
        
        /// New revenue values
        #[arg(long, value_name = "REVENUE")]
        revenue: Option<String>,
        
        /// New payment address 
        #[arg(long, value_name = "ADDRESS")]
        address: Option<String>,
    }
}

#[derive(Subcommand)] 
pub enum EpochCommands {
    /// Create a new epoch period
    Create {
        /// Epoch name/identifier
        #[arg(value_name = "NAME")]
        name: String,
        
        /// Start date (YYYY-MM-DD)
        #[arg(value_name = "START_DATE")]
        start_date: String,
        
        /// End date (YYYY-MM-DD)
        #[arg(value_name = "END_DATE")]
        end_date: String,
    },

    /// Activate an epoch for proposals
    Activate {
        /// Epoch name to activate
        #[arg(value_name = "NAME")]
        name: String,
    },

    /// Set epoch reward amount
    SetReward {
        /// Token symbol (e.g. ETH)
        #[arg(value_name = "TOKEN")]
        token: String,
        
        /// Reward amount
        #[arg(value_name = "AMOUNT")]
        amount: f64,
    },

    /// Close an epoch
    Close {
        /// Optional epoch name (uses active if omitted)
        #[arg(value_name = "NAME")]
        epoch_name: Option<String>,
    }
}

#[derive(Subcommand)]
pub enum ProposalCommands {
   /// Add a new proposal
   Add {
       /// Proposal title
       #[arg(long, value_name = "TITLE")]
       title: String,

       /// Proposal URL
       #[arg(long, value_name = "URL")] 
       url: Option<String>,
       
       /// Team name
       #[arg(long, value_name = "TEAM")]
       team: Option<String>,
       
       /// Request amounts (format: ETH:100.5,USD:1000)
       #[arg(long, value_name = "AMOUNTS")]
       amounts: Option<String>,
       
       /// Start date (YYYY-MM-DD)
       #[arg(long, value_name = "START")]
       start: Option<String>,
       
       /// End date (YYYY-MM-DD)
       #[arg(long, value_name = "END")]
       end: Option<String>,
       
       /// Is loan request
       #[arg(long)]
       loan: Option<bool>,
       
       /// Payment address
       #[arg(long, value_name = "ADDRESS")]
       address: Option<String>,
   },

   /// Update an existing proposal 
   Update {
       /// Proposal name to update
       #[arg(value_name = "NAME")]
       name: String,
       
       #[arg(long, value_name = "TITLE")]
       title: Option<String>,
       
       #[arg(long, value_name = "URL")]
       url: Option<String>,
       
       #[arg(long, value_name = "TEAM")]
       team: Option<String>,
       
       #[arg(long, value_name = "AMOUNTS")] 
       amounts: Option<String>,
       
       #[arg(long, value_name = "START")]
       start: Option<String>,
       
       #[arg(long, value_name = "END")]
       end: Option<String>,
       
       #[arg(long)]
       loan: Option<bool>,
       
       #[arg(long, value_name = "ADDRESS")]
       address: Option<String>,
   },

   /// Close a proposal
   Close {
       /// Proposal name
       name: String,
       
       /// Resolution (Approved/Rejected/Invalid/Duplicate/Retracted)
       resolution: String,
   },
}

#[derive(Subcommand)]
pub enum VoteCommands {
   /// Process a vote
   Process {
       /// Proposal name
       name: String,
       
       /// Counted votes (format: Team1:Yes,Team2:No)
       #[arg(long, value_name = "COUNTED")]
       counted: String,
       
       /// Uncounted votes (format: Team3:Yes,Team4:No)
       #[arg(long, value_name = "UNCOUNTED")]  
       uncounted: String,
       
       /// Vote opened date (YYYY-MM-DD)
       #[arg(long, value_name = "OPENED")]
       opened: Option<String>,
       
       /// Vote closed date (YYYY-MM-DD)
       #[arg(long, value_name = "CLOSED")]
       closed: Option<String>,
   }
}

#[derive(Subcommand)]
pub enum RaffleCommands {
   /// Create a new raffle
   Create {
       /// Proposal name
       name: String,
       
       /// Block offset
       #[arg(long, value_name = "OFFSET")]
       block_offset: Option<u64>,
       
       /// Excluded teams (comma separated)
       #[arg(long, value_name = "EXCLUDED")]
       excluded: Option<String>,
   }
}

#[derive(Subcommand)]
pub enum ReportCommands {
   /// Print team report
   Team,

   /// Print epoch state report
   EpochState,

   /// Print team vote participation
   TeamParticipation {
       team_name: String,
       epoch_name: Option<String>,
   },

   /// Print point report
   Points {
       #[arg(long, value_name = "EPOCH")]
       epoch_name: Option<String>,
   },

   /// Generate closed proposals report
   ClosedProposals {
       #[arg(value_name = "EPOCH")]
       epoch_name: String,
   },

   /// Generate end of epoch report
   EndOfEpoch {
       #[arg(value_name = "EPOCH")] 
       epoch_name: String,
   },

   /// Generate unpaid requests report
   UnpaidRequests {
       #[arg(long, value_name = "PATH")]
       output_path: Option<String>,
       #[arg(long, value_name = "EPOCH")]
       epoch_name: Option<String>,
   },

   /// Generate report for specific proposal
   ForProposal {
       #[arg(value_name = "PROPOSAL")]
       proposal_name: String,
   },
}


#[derive(Subcommand)]
pub enum ImportCommands {
   /// Import a predefined raffle
   PredefinedRaffle {
       proposal_name: String,
       counted_teams: Vec<String>,
       uncounted_teams: Vec<String>, 
       total_counted_seats: usize,
       max_earner_seats: usize,
   },

   /// Import historical vote
   HistoricalVote {
       proposal_name: String,
       passed: bool,
       participating_teams: Vec<String>,
       non_participating_teams: Vec<String>,
       counted_points: Option<u32>,
       uncounted_points: Option<u32>,
   },

   /// Import historical raffle
   HistoricalRaffle {
       proposal_name: String,
       initiation_block: u64,
       randomness_block: u64,
       team_order: Option<Vec<String>>,
       excluded_teams: Option<Vec<String>>,
       total_counted_seats: Option<usize>,
       max_earner_seats: Option<usize>,
   }
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

fn parse_votes(votes_str: &str) -> Result<HashMap<String, VoteChoice>, Box<dyn Error>> {
    votes_str
        .split(',')
        .map(|vote| {
            let parts: Vec<&str> = vote.split(':').collect();
            if parts.len() != 2 {
                return Err("Invalid vote format. Expected Team:Choice".into());
            }
            let choice = match parts[1].to_lowercase().as_str() {
                "yes" => VoteChoice::Yes,
                "no" => VoteChoice::No,
                _ => return Err(format!("Invalid vote choice: {}. Must be Yes or No", parts[1]).into()),
            };
            Ok((parts[0].to_string(), choice))
        })
        .collect()
}


impl Cli {
    pub fn into_command(self) -> Result<Command, Box<dyn Error>> {
        match self.command {

            Commands::Team { command } => match command {
                TeamCommands::Add { name, representative, revenue, address } => {
                    Ok(Command::AddTeam {
                        name,
                        representative, 
                        trailing_monthly_revenue: revenue.map(|rev| {
                            rev.split(',')
                               .map(|v| v.parse::<u64>().unwrap())
                               .collect()
                        }),
                        address
                    })
                },
                TeamCommands::Update { name, new_name, representative, status, revenue, address } => {
                    Ok(Command::UpdateTeam {
                        team_name: name,
                        updates: UpdateTeamDetails {
                            name: new_name,
                            representative,
                            status,
                            trailing_monthly_revenue: revenue.map(|rev| {
                                rev.split(',')
                                   .map(|v| v.parse::<u64>().unwrap())
                                   .collect()
                            }),
                            address
                        }
                    })
                }
            },

            Commands::Epoch { command } => match command {
                EpochCommands::Create { name, start_date, end_date } => {
                    let start = DateTime::parse_from_rfc3339(&start_date)?
                        .with_timezone(&Utc);
                    let end = DateTime::parse_from_rfc3339(&end_date)?
                        .with_timezone(&Utc);
                    Ok(Command::CreateEpoch { name, start_date: start, end_date: end })
                },
                EpochCommands::Activate { name } => {
                    Ok(Command::ActivateEpoch { name })
                },
                EpochCommands::SetReward { token, amount } => {
                    Ok(Command::SetEpochReward { token, amount }) 
                },
                EpochCommands::Close { epoch_name } => {
                    Ok(Command::CloseEpoch { epoch_name })
                }
            },

            Commands::Proposal { command } => match command {
                ProposalCommands::Add { title, url, team, amounts, start, end, loan, address } => {
                    let budget_details = if team.is_some() || amounts.is_some() {
                        Some(BudgetRequestDetailsCommand {
                            team,
                            request_amounts: amounts.map(|a| parse_amounts(&a).unwrap()), //TODO remove the unwrap
                            start_date: start.map(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d")).transpose()?,
                            end_date: end.map(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d")).transpose()?,
                            is_loan: loan,
                            payment_address: address,
                        })
                    } else {
                        None
                    };

                    Ok(Command::AddProposal {
                        title,
                        url,
                        budget_request_details: budget_details,
                        announced_at: None,
                        published_at: None,
                        is_historical: None,
                    })
                },
                ProposalCommands::Close { name, resolution } => {
                    Ok(Command::CloseProposal { proposal_name: name, resolution })
                },
                ProposalCommands::Update { 
                    name, title, url, team, amounts, start, end, loan, address 
                } => {
                    let budget_details = if team.is_some() || amounts.is_some() {
                        Some(BudgetRequestDetailsCommand {
                            team,
                            request_amounts: amounts.map(|a| parse_amounts(&a).unwrap()), //TODO remove unwrap
                            start_date: start.map(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d")).transpose()?,
                            end_date: end.map(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d")).transpose()?,
                            is_loan: loan,
                            payment_address: address,
                        })
                    } else {
                        None
                    };

                    Ok(Command::UpdateProposal {
                        proposal_name: name,
                        updates: UpdateProposalDetails {
                            title,
                            url,
                            budget_request_details: budget_details,
                            announced_at: None,
                            published_at: None,
                            resolved_at: None,
                        }
                    })
                },
            },

            Commands::Vote { command } => match command {
                VoteCommands::Process { name, counted, uncounted, opened, closed } => {
                    Ok(Command::CreateAndProcessVote {
                        proposal_name: name,
                        counted_votes: parse_votes(&counted)?,
                        uncounted_votes: parse_votes(&uncounted)?,
                        vote_opened: opened.map(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d")).transpose()?,
                        vote_closed: closed.map(|d| NaiveDate::parse_from_str(&d, "%Y-%m-%d")).transpose()?,
                    })
                }
            },

            Commands::Raffle { command } => match command {
                RaffleCommands::Create { name, block_offset, excluded } => {
                    Ok(Command::CreateRaffle {
                        proposal_name: name,
                        block_offset,
                        excluded_teams: excluded.map(|e| e.split(',').map(String::from).collect()),
                    })
                }
            },

            Commands::Report { command } => match command {
                ReportCommands::Team => {
                    Ok(Command::PrintTeamReport)
                },
                ReportCommands::EpochState => {
                    Ok(Command::PrintEpochState)
                },
                ReportCommands::TeamParticipation { team_name, epoch_name } => {
                    Ok(Command::PrintTeamVoteParticipation { team_name, epoch_name })
                },
                ReportCommands::Points { epoch_name } => {
                    Ok(Command::PrintPointReport { epoch_name })
                },
                ReportCommands::EndOfEpoch { epoch_name } => {
                    Ok(Command::GenerateEndOfEpochReport { epoch_name })
                },
                ReportCommands::UnpaidRequests { output_path, epoch_name } => {
                    Ok(Command::GenerateUnpaidRequestsReport { output_path, epoch_name })
                },
                ReportCommands::ForProposal { proposal_name } => {
                    Ok(Command::GenerateReportForProposal { proposal_name })
                },
                ReportCommands::ClosedProposals { epoch_name } => {
                    Ok(Command::GenerateReportsForClosedProposals { epoch_name })
                },
            },

            Commands::Import { command } => match command {
                ImportCommands::PredefinedRaffle { 
                    proposal_name, 
                    counted_teams,
                    uncounted_teams,
                    total_counted_seats,
                    max_earner_seats 
                } => {
                    Ok(Command::ImportPredefinedRaffle {
                        proposal_name,
                        counted_teams,
                        uncounted_teams, 
                        total_counted_seats,
                        max_earner_seats
                    })
                },
                ImportCommands::HistoricalVote {
                    proposal_name,
                    passed,
                    participating_teams,
                    non_participating_teams,
                    counted_points,
                    uncounted_points
                } => {
                    Ok(Command::ImportHistoricalVote {
                        proposal_name,
                        passed,
                        participating_teams,
                        non_participating_teams,
                        counted_points,
                        uncounted_points
                    })
                },
                ImportCommands::HistoricalRaffle {
                    proposal_name,
                    initiation_block,
                    randomness_block,
                    team_order,
                    excluded_teams,
                    total_counted_seats,
                    max_earner_seats
                } => {
                    Ok(Command::ImportHistoricalRaffle {
                        proposal_name,
                        initiation_block,
                        randomness_block,
                        team_order,
                        excluded_teams,
                        total_counted_seats,
                        max_earner_seats
                    })
                }
            },

            Commands::RunScript { script_file_path } => {
                Ok(Command::RunScript { script_file_path })
            },
        }
    }
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
    let cli = Cli::parse_from(args);
    cli.into_command()
}

fn parse_amounts(amounts_str: &str) -> Result<HashMap<String, f64>, Box<dyn Error>> {
    amounts_str
        .split(',')
        .map(|pair| {
            let parts: Vec<&str> = pair.split(':').collect();
            if parts.len() != 2 {
                return Err("Invalid amount format. Expected token:amount".into());
            }
            let amount = parts[1].parse::<f64>()
                .map_err(|_| format!("Invalid amount: {}", parts[1]))?;
            Ok((parts[0].to_string(), amount))
        })
        .collect()
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
    use sha2::digest::typenum::assert_type;
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
                address: None,
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
    fn test_parse_eth_address() {
        assert!(parse_eth_address("0x742d35Cc6634C0532925a3b844Bc454e4438f44e").is_ok());
        assert!(parse_eth_address("742d35Cc6634C0532925a3b844Bc454e4438f44e").is_err()); // no 0x
        assert!(parse_eth_address("0x742d35").is_err()); // too short
        assert!(parse_eth_address("0x742d35Cc6634C0532925a3b844Bc454e4438f44eXX").is_err()); // invalid hex
    }

    #[test]
   fn test_vote_process_command() {
       let args = vec![
           "robokitty".to_string(),
           "vote".to_string(), 
           "process".to_string(),
           "Test Proposal".to_string(),
           "--counted".to_string(), "Team1:Yes,Team2:No".to_string(),
           "--uncounted".to_string(), "Team3:Yes".to_string(),
           "--opened".to_string(), "2024-01-01".to_string(),
           "--closed".to_string(), "2024-01-02".to_string()
       ];

       let command = parse_cli_args(&args).unwrap();
       match command {
           Command::CreateAndProcessVote { 
               proposal_name,
               counted_votes,
               uncounted_votes,
               vote_opened,
               vote_closed
           } => {
               assert_eq!(proposal_name, "Test Proposal");
               assert_eq!(counted_votes.len(), 2);
               assert_eq!(uncounted_votes.len(), 1);
               assert_eq!(vote_opened, Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()));
               assert_eq!(vote_closed, Some(NaiveDate::from_ymd_opt(2024, 1, 2).unwrap()));
           },
           _ => panic!("Wrong command type")
       }
   }

   #[test]
   fn test_raffle_create_command() {
       let args = vec![
           "robokitty".to_string(),
           "raffle".to_string(),
           "create".to_string(),
           "Test Proposal".to_string(),
           "--block-offset".to_string(), "10".to_string(),
           "--excluded".to_string(), "Team1,Team2".to_string()
       ];

       let command = parse_cli_args(&args).unwrap();
       match command {
           Command::CreateRaffle {
               proposal_name,
               block_offset,
               excluded_teams
           } => {
               assert_eq!(proposal_name, "Test Proposal");
               assert_eq!(block_offset, Some(10));
               assert_eq!(excluded_teams, Some(vec!["Team1".to_string(), "Team2".to_string()]));
           },
           _ => panic!("Wrong command type")
       }
   }

   #[test]
    fn test_activate_epoch() {
        let args = vec![
            "robokitty".to_string(),
            "epoch".to_string(),
            "activate".to_string(),
            "Test Epoch".to_string()
        ];

        let command = parse_cli_args(&args).unwrap();
        match command {
            Command::ActivateEpoch { name } => {
                assert_eq!(name, "Test Epoch");
            },
            _ => panic!("Wrong command type")
        }
    }

    #[test]
    fn test_update_team() {
        let args = vec![
            "robokitty".to_string(),
            "team".to_string(),
            "update".to_string(),
            "Old Team".to_string(),
            "--new-name".to_string(), "New Team".to_string(),
            "--status".to_string(), "Supporter".to_string(),
            "--revenue".to_string(), "1000,2000,3000".to_string()
        ];

        let command = parse_cli_args(&args).unwrap();
        match command {
            Command::UpdateTeam { team_name, updates } => {
                assert_eq!(team_name, "Old Team");
                assert_eq!(updates.name, Some("New Team".to_string()));
                assert_eq!(updates.status, Some("Supporter".to_string()));
                assert_eq!(updates.trailing_monthly_revenue, Some(vec![1000, 2000, 3000]));
            },
            _ => panic!("Wrong command type") 
        }
    }

    #[test]
    fn test_proposal_with_loan_and_address() {
        // Test add proposal
        let cli = Cli::parse_from([
            "robokitty", 
            "proposal",
            "add",
            "--title", "Test Proposal",
            "--url", "https://test.com",
            "--team", "Team1",
            "--amounts", "ETH:100.5",
            "--loan", "true",
            "--address", "0x742d35Cc6634C0532925a3b844Bc454e4438f44e"
        ].iter());

        let cmd = cli.into_command().unwrap();
        match cmd {
            Command::AddProposal { 
                title,
                url,
                budget_request_details,
                ..
            } => {
                assert_eq!(title, "Test Proposal");
                if let Some(details) = budget_request_details {
                    assert!(details.is_loan.unwrap());
                    assert_eq!(details.payment_address, Some("0x742d35Cc6634C0532925a3b844Bc454e4438f44e".to_string()));
                } else {
                    panic!("Expected budget details");
                }
            },
            _ => panic!("Wrong command type")
        }

        // Test update proposal
        let cli = Cli::parse_from([
            "robokitty",
            "proposal", 
            "update",
            "Test Proposal",
            "--loan", "true",
            "--address", "0x742d35Cc6634C0532925a3b844Bc454e4438f44e"
        ].iter());

        let cmd = cli.into_command().unwrap();
        match cmd {
            Command::UpdateProposal {
                proposal_name,
                updates
            } => {
                assert_eq!(proposal_name, "Test Proposal");
                if let Some(details) = updates.budget_request_details {
                    assert!(details.is_loan.unwrap());
                    assert_eq!(details.payment_address, Some("0x742d35Cc6634C0532925a3b844Bc454e4438f44e".to_string()));
                } else {
                    panic!("Expected budget details");
                }
            },
            _ => panic!("Wrong command type")
        }
    }

    #[test]
    fn test_team_report_command() {
        let cli = Cli::parse_from([
            "robokitty", 
            "report",
            "team"
        ].iter());
 
        let cmd = cli.into_command().unwrap();
        assert!(matches!(cmd, Command::PrintTeamReport));
    }
 
    #[test] 
    fn test_epoch_state_report_command() {
        let cli = Cli::parse_from([
            "robokitty",
            "report", 
            "epoch-state"
        ].iter());
 
        let cmd = cli.into_command().unwrap();
        assert!(matches!(cmd, Command::PrintEpochState));
    }
 
    #[test]
    fn test_team_participation_report_command() {
        let cli = Cli::parse_from([
            "robokitty",
            "report",
            "team-participation",
            "Test Team",
            "--epoch-name",
            "Test Epoch"
        ].iter());
 
        let cmd = cli.into_command().unwrap();
        match cmd {
            Command::PrintTeamVoteParticipation { team_name, epoch_name } => {
                assert_eq!(team_name, "Test Team");
                assert_eq!(epoch_name, Some("Test Epoch".to_string()));
            },
            _ => panic!("Wrong command type")
        }
    }
 
    #[test]
    fn test_points_report_command() {
        let cli = Cli::parse_from([
            "robokitty",
            "report",
            "points",
            "--epoch-name",
            "Test Epoch"
        ].iter());
 
        let cmd = cli.into_command().unwrap();
        match cmd {
            Command::PrintPointReport { epoch_name } => {
                assert_eq!(epoch_name, Some("Test Epoch".to_string()));
            },
            _ => panic!("Wrong command type")
        }
    }
 
    #[test]
    fn test_end_of_epoch_report_command() {
        let cli = Cli::parse_from([
            "robokitty",
            "report",
            "end-of-epoch",
            "Test Epoch"
        ].iter());
 
        let cmd = cli.into_command().unwrap();
        match cmd {
            Command::GenerateEndOfEpochReport { epoch_name } => {
                assert_eq!(epoch_name, "Test Epoch");
            },
            _ => panic!("Wrong command type")
        }
    }
 
    #[test]
    fn test_unpaid_requests_report_command() {
        let cli = Cli::parse_from([
            "robokitty",
            "report",
            "unpaid-requests",
            "--output-path",
            "test.json",
            "--epoch-name",
            "Test Epoch"
        ].iter());
 
        let cmd = cli.into_command().unwrap();
        match cmd {
            Command::GenerateUnpaidRequestsReport { output_path, epoch_name } => {
                assert_eq!(output_path, Some("test.json".to_string()));
                assert_eq!(epoch_name, Some("Test Epoch".to_string()));
            },
            _ => panic!("Wrong command type")
        }
    }
 
    #[test]
    fn test_proposal_report_command() {
        let cli = Cli::parse_from([
            "robokitty",
            "report",
            "for-proposal",
            "--proposal-name",
            "Test Proposal"
        ].iter());
 
        let cmd = cli.into_command().unwrap();
        match cmd {
            Command::GenerateReportForProposal { proposal_name } => {
                assert_eq!(proposal_name, "Test Proposal");
            },
            _ => panic!("Wrong command type")
        }
    }
 
    #[test]
    fn test_closed_proposals_report_command() {
        let cli = Cli::parse_from([
            "robokitty",
            "report",
            "closed-proposals",
            "Test Epoch"
        ].iter());
 
        let cmd = cli.into_command().unwrap();
        match cmd {
            Command::GenerateReportsForClosedProposals { epoch_name } => {
                assert_eq!(epoch_name, "Test Epoch");
            },
            _ => panic!("Wrong command type")
        }
    }
 
    #[test]
    fn test_report_command_invalid_args() {
        let result = Cli::parse_from([
            "robokitty",
            "report",
            "team-participation",
            "--epoch-name",
            "Test Epoch"
        ].iter()).into_command();
 
        assert!(result.is_err());
    }

    // Epoch Command Tests
    #[test]
    fn test_close_epoch_command() {
    let cli = Cli::parse_from([
        "robokitty",
        "epoch",
        "close",
        "Test Epoch"
    ].iter());

    let cmd = cli.into_command().unwrap();
    match cmd {
        Command::CloseEpoch { epoch_name } => {
            assert_eq!(epoch_name, Some("Test Epoch".to_string()));
        },
        _ => panic!("Wrong command type")
    }
    }

    #[test]
    fn test_close_epoch_without_name() {
    let cli = Cli::parse_from([
        "robokitty",
        "epoch",
        "close"
    ].iter());

    let cmd = cli.into_command().unwrap();
    match cmd {
        Command::CloseEpoch { epoch_name } => {
            assert_eq!(epoch_name, None);
        },
        _ => panic!("Wrong command type")
    }
    }

    // Script Command Tests
    #[test]
    fn test_run_script_command() {
    let cli = Cli::parse_from([
        "robokitty",
        "run-script",
        "--script-file", "test_script.json"
    ].iter());

    let cmd = cli.into_command().unwrap();
    match cmd {
        Command::RunScript { script_file_path } => {
            assert_eq!(script_file_path, Some("test_script.json".to_string()));
        },
        _ => panic!("Wrong command type")
    }
    }

    #[test]
    fn test_run_script_without_path() {
    let cli = Cli::parse_from([
        "robokitty",
        "run-script"
    ].iter());

    let cmd = cli.into_command().unwrap();
    match cmd {
        Command::RunScript { script_file_path } => {
            assert_eq!(script_file_path, None);
        },
        _ => panic!("Wrong command type")
    }
    }

    #[test]
    fn test_report_command_missing_required_args() {
        // Team participation missing team name
        let cli = Cli::parse_from([
            "robokitty",
            "report",
            "team-participation"
        ].iter());
        assert!(cli.into_command().is_err());

        // End of epoch missing epoch name
        let cli = Cli::parse_from([
            "robokitty", 
            "report",
            "end-of-epoch"
        ].iter());
        assert!(cli.into_command().is_err());
    }

    #[test]
    fn test_close_epoch_invalid_state() {
        let cli = Cli::parse_from([
            "robokitty",
            "epoch",
            "close",
        ].iter());
        assert!(cli.into_command().is_err());
    }

    #[test]
    #[should_panic]
    fn test_script_file_validation() {
        // Invalid file path characters
        let cli = Cli::parse_from([
            "robokitty",
            "run-script",
            "--script-file", "/invalid/*/path"
        ].iter());
        cli.into_command().unwrap();
    }

    #[test]
    fn test_epoch_commands_validation() {
        // Test invalid dates
        let cmd = Cli::parse_from([
            "robokitty",
            "epoch", 
            "create",
            "Test",
            "invalid-date",
            "2024-01-01"
        ]);
        assert!(cmd.into_command().is_err());

        // Test end date before start
        let cmd = Cli::parse_from([
            "robokitty",
            "epoch",
            "create", 
            "Test",
            "2024-02-01",
            "2024-01-01"
        ]);
        assert!(cmd.into_command().is_err());
    }

    #[test]
    fn test_proposal_validation() {
        // Test invalid eth address
        let cmd = Cli::parse_from([
            "robokitty", "proposal", "add",
            "--title", "Test",
            "--address", "invalid_address"
        ]);
        assert!(cmd.into_command().is_err());

        // Test invalid amounts format
        let cmd = Cli::parse_from([
            "robokitty", "proposal", "add",
            "--title", "Test", 
            "--amounts", "ETH:invalid"
        ]);
        assert!(cmd.into_command().is_err());
    }

    #[test]
    fn test_proposal_resolution_validation() {
        let cmd = Cli::parse_from([
            "robokitty", "proposal", "close",
            "Test",
            "--resolution", "Invalid_Status"
        ]);
        assert!(cmd.into_command().is_err());
    }

    #[test]
    fn test_vote_validation() {
        // Test invalid vote format
        let cmd = Cli::parse_from([
            "robokitty", "vote", "process",
            "TestVote",
            "--counted", "Team1:Maybe", // Invalid choice
        ]);
        assert!(cmd.into_command().is_err());

        // Test invalid date format
        let cmd = Cli::parse_from([
            "robokitty", "vote", "process", 
            "TestVote",
            "--counted", "Team1:Yes",
            "--opened", "invalid-date"
        ]);
        assert!(cmd.into_command().is_err());
    }

    #[test]
    fn test_raffle_validation() {
        // Test invalid block offset
        let cmd = Cli::parse_from([
            "robokitty", "raffle", "create",
            "TestRaffle", 
            "--block-offset", "invalid"
        ]);
        assert!(cmd.into_command().is_err());

        // Test excluded teams format
        let cmd = Cli::parse_from([
            "robokitty", "raffle", "create",
            "TestRaffle",
            "--excluded", "Team1,,Team2" // Invalid format
        ]);
        assert!(cmd.into_command().is_err());
    }

    #[test]
    fn test_report_commands() {
        // Test missing required args
        let cmd = Cli::parse_from([
            "robokitty", "report", "team-participation" 
        ]);
        assert!(cmd.into_command().is_err());

        // Test invalid output path
        let cmd = Cli::parse_from([
            "robokitty", "report", "unpaid-requests",
            "--output-path", "/invalid/*/path"
        ]);
        assert!(cmd.into_command().is_err());
    }

    #[test]
    fn test_import_commands() {
        // Test invalid seats configuration
        let cmd = Cli::parse_from([
            "robokitty", "import", "predefined-raffle",
            "Test",
            "--total-counted-seats", "3",
            "--max-earner-seats", "5" // Invalid: max > total
        ]);
        assert!(cmd.into_command().is_err());

        // Test invalid block numbers
        let cmd = Cli::parse_from([
            "robokitty", "import", "historical-raffle",
            "Test",
            "--initiation-block", "200",
            "--randomness-block", "100" // Invalid: random < init
        ]);
        assert!(cmd.into_command().is_err());
    }

    #[test]
    fn test_import_command_validation() {
        // Test invalid seat counts
        let cli = Cli::parse_from([
            "robokitty",
            "import",
            "predefined-raffle",
            "--proposal-name", "Test",
            "--counted-teams", "Team1",
            "--uncounted-teams", "Team2", 
            "--max-earner-seats", "8", // Greater than total seats
            "--total-counted-seats", "7"
        ].iter());
        assert!(cli.into_command().is_err());

        // Test invalid block numbers
        let cli = Cli::parse_from([
            "robokitty",
            "import",
            "historical-raffle",
            "--proposal-name", "Test",
            "--initiation-block", "200",
            "--randomness-block", "100", // Before initiation block
            "--team-order", "Team1,Team2"
        ].iter());
        assert!(cli.into_command().is_err());
    }

    #[test]
    fn test_import_historical_vote_command() {
        let cli = Cli::parse_from([
            "robokitty",
            "import",
            "historical-vote",
            "--proposal-name", "Test Proposal",
            "--passed", "true",
            "--participating-teams", "Team1,Team2",
            "--non-participating-teams", "Team3,Team4",
            "--counted-points", "5",
            "--uncounted-points", "2"
        ].iter());

        let cmd = cli.into_command().unwrap();
        match cmd {
            Command::ImportHistoricalVote {
                proposal_name,
                passed,
                participating_teams,
                non_participating_teams,
                counted_points,
                uncounted_points
            } => {
                assert_eq!(proposal_name, "Test Proposal");
                assert!(passed);
                assert_eq!(participating_teams, vec!["Team1", "Team2"]);
                assert_eq!(non_participating_teams, vec!["Team3", "Team4"]);
                assert_eq!(counted_points, Some(5));
                assert_eq!(uncounted_points, Some(2));
            },
            _ => panic!("Wrong command type")
        }
    }

    #[test]
    fn test_import_historical_raffle_command() {
        let cli = Cli::parse_from([
            "robokitty",
            "import", 
            "historical-raffle",
            "--proposal-name", "Test Proposal",
            "--initiation-block", "100",
            "--randomness-block", "110",
            "--team-order", "Team1,Team2",
            "--excluded-teams", "Team3,Team4",
            "--total-counted-seats", "7",
            "--max-earner-seats", "5"
        ].iter());

        let cmd = cli.into_command().unwrap();
        match cmd {
            Command::ImportHistoricalRaffle {
                proposal_name,
                initiation_block,
                randomness_block,
                team_order,
                excluded_teams,
                total_counted_seats,
                max_earner_seats
            } => {
                assert_eq!(proposal_name, "Test Proposal");
                assert_eq!(initiation_block, 100);
                assert_eq!(randomness_block, 110);
                assert_eq!(team_order, Some(vec!["Team1".to_string(), "Team2".to_string()]));
                assert_eq!(excluded_teams, Some(vec!["Team3".to_string(), "Team4".to_string()]));
                assert_eq!(total_counted_seats, Some(7));
                assert_eq!(max_earner_seats, Some(5));
            },
            _ => panic!("Wrong command type")
        }
    }

    #[test]
    fn test_import_predefined_raffle_command() {
        let cli = Cli::parse_from([
            "robokitty",
            "import",
            "predefined-raffle",
            "--proposal-name", "Test Proposal",
            "--counted-teams", "Team1,Team2",
            "--uncounted-teams", "Team3,Team4",
            "--total-counted-seats", "7",
            "--max-earner-seats", "5"
        ].iter());

        let cmd = cli.into_command().unwrap();
        match cmd {
            Command::ImportPredefinedRaffle { 
                proposal_name,
                counted_teams,
                uncounted_teams,
                total_counted_seats,
                max_earner_seats
            } => {
                assert_eq!(proposal_name, "Test Proposal");
                assert_eq!(counted_teams, vec!["Team1", "Team2"]);
                assert_eq!(uncounted_teams, vec!["Team3", "Team4"]);
                assert_eq!(total_counted_seats, 7);
                assert_eq!(max_earner_seats, 5);
            },
            _ => panic!("Wrong command type")
        }
    }

}