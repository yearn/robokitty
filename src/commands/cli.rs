// src/commands/cli.rs
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::error::Error;
use uuid::Uuid;
use tokio::time::Duration;

use crate::core::models::{
    BudgetRequestDetails, PaymentStatus, Resolution, TeamStatus, VoteChoice, VoteType, VoteParticipation, NameMatches
};
use crate::core::budget_system::BudgetSystem;
use crate::app_config::AppConfig;


#[derive(Debug, Deserialize, Clone)]
#[serde(tag = "type", content = "params")]
pub enum ScriptCommand {
    CreateEpoch { name: String, start_date: DateTime<Utc>, end_date: DateTime<Utc> },
    ActivateEpoch { name: String },
    SetEpochReward { token: String, amount: f64 },
    AddTeam { name: String, representative: String, trailing_monthly_revenue: Option<Vec<u64>> },
    AddProposal {
        title: String,
        url: Option<String>,
        budget_request_details: Option<BudgetRequestDetailsScript>,
        announced_at: Option<NaiveDate>,
        published_at: Option<NaiveDate>,
        is_historical: Option<bool>,
    },
    UpdateProposal {
        proposal_name: String,
        updates: UpdateProposalDetails,
    },
    ImportPredefinedRaffle {
        proposal_name: String,
        counted_teams: Vec<String>,
        uncounted_teams: Vec<String>,
        total_counted_seats: usize,
        max_earner_seats: usize,
    },
    ImportHistoricalVote {
        proposal_name: String,
        passed: bool,
        participating_teams: Vec<String>,
        non_participating_teams: Vec<String>,
        counted_points: Option<u32>,
        uncounted_points: Option<u32>,
    },
    ImportHistoricalRaffle {
        proposal_name: String,
        initiation_block: u64,
        randomness_block: u64,
        team_order: Option<Vec<String>>,
        excluded_teams: Option<Vec<String>>,
        total_counted_seats: Option<usize>,
        max_earner_seats: Option<usize>,
    },
    ChangeTeamStatus {
        team_name: String,
        new_status: String,
        trailing_monthly_revenue: Option<Vec<u64>>,
    },
    PrintTeamReport,
    PrintEpochState,
    PrintTeamVoteParticipation {
        team_name: String,
        epoch_name: Option<String> 
    },
    CloseProposal {
        proposal_name: String,
        resolution: String,
    },
    CreateRaffle {
        proposal_name: String,
        block_offset: Option<u64>,
        excluded_teams: Option<Vec<String>>,
    },
    CreateAndProcessVote {
        proposal_name: String,
        counted_votes: HashMap<String, VoteChoice>,
        uncounted_votes: HashMap<String, VoteChoice>,
        vote_opened: Option<NaiveDate>,
        vote_closed: Option<NaiveDate>,
    },
    GenerateReportsForClosedProposals { epoch_name: String },
    GenerateReportForProposal { proposal_name: String },
    PrintPointReport { epoch_name: Option<String> },
    CloseEpoch { epoch_name: Option<String> },
    GenerateEndOfEpochReport { epoch_name: String },
}

#[derive(Debug, Deserialize, Clone)]
pub struct UpdateProposalDetails {
    pub title: Option<String>,
    pub url: Option<String>,
    pub budget_request_details: Option<BudgetRequestDetailsScript>,
    pub announced_at: Option<NaiveDate>,
    pub published_at: Option<NaiveDate>,
    pub resolved_at: Option<NaiveDate>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct BudgetRequestDetailsScript {
    pub team: Option<String>,
    pub request_amounts: Option<HashMap<String, f64>>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub payment_status: Option<PaymentStatus>,
}

pub async fn execute_command(budget_system: &mut BudgetSystem, command: ScriptCommand, config: &AppConfig) -> Result<(), Box<dyn Error>> {
    match command {
        ScriptCommand::CreateEpoch { name, start_date, end_date } => {
            let epoch_id = budget_system.create_epoch(&name, start_date, end_date)?;
            println!("Created epoch: {} ({})", name, epoch_id);
        },
        ScriptCommand::ActivateEpoch { name } => {
            let epoch_id = budget_system.get_epoch_id_by_name(&name)
                .ok_or_else(|| format!("Epoch not found: {}", name))?;
            budget_system.activate_epoch(epoch_id)?;
            println!("Activated epoch: {} ({})", name, epoch_id);
        },
        ScriptCommand::SetEpochReward { token, amount } => {
            budget_system.set_epoch_reward(&token, amount)?;
            println!("Set epoch reward: {} {}", amount, token);
        },
        ScriptCommand::AddTeam { name, representative, trailing_monthly_revenue } => {
            let team_id = budget_system.create_team(name.clone(), representative, trailing_monthly_revenue)?;
            println!("Added team: {} ({})", name, team_id);
        },
        ScriptCommand::AddProposal { title, url, budget_request_details, announced_at, published_at, is_historical } => {
            let budget_request_details = if let Some(details) = budget_request_details {
                let team_id = details.team.as_ref()
                    .and_then(|name| budget_system.get_team_id_by_name(name));
                
                Some(BudgetRequestDetails::new(
                    team_id,
                    details.request_amounts.unwrap_or_default(),
                    details.start_date,
                    details.end_date,
                    details.payment_status
                )?)
            } else {
                None
            };
            
            let proposal_id = budget_system.add_proposal(title.clone(), url, budget_request_details, announced_at, published_at, is_historical)?;
            println!("Added proposal: {} ({})", title, proposal_id);
        },
        ScriptCommand::UpdateProposal { proposal_name, updates } => {
            budget_system.update_proposal(&proposal_name, updates)?;
            println!("Updated proposal: {}", proposal_name);
        },
        ScriptCommand::ImportPredefinedRaffle { 
            proposal_name, 
            counted_teams, 
            uncounted_teams, 
            total_counted_seats, 
            max_earner_seats 
        } => {
            let raffle_id = budget_system.import_predefined_raffle(
                &proposal_name, 
                counted_teams.clone(), 
                uncounted_teams.clone(), 
                total_counted_seats, 
                max_earner_seats
            )?;
            
            let raffle = budget_system.state().raffles().get(&raffle_id).unwrap();

            println!("Imported predefined raffle for proposal '{}' (Raffle ID: {})", proposal_name, raffle_id);
            println!("  Counted teams: {:?}", counted_teams);
            println!("  Uncounted teams: {:?}", uncounted_teams);
            println!("  Total counted seats: {}", total_counted_seats);
            println!("  Max earner seats: {}", max_earner_seats);

            // Print team snapshots
            println!("\nTeam Snapshots:");
            for snapshot in raffle.team_snapshots() {
                println!("  {} ({}): {:?}", snapshot.name(), snapshot.id(), snapshot.status());
            }

            // Print raffle result
            if let Some(result) = raffle.result() {
                println!("\nRaffle Result:");
                println!("  Counted teams: {:?}", result.counted());
                println!("  Uncounted teams: {:?}", result.uncounted());
            } else {
                println!("\nRaffle result not available");
            }
        },
        ScriptCommand::ImportHistoricalVote { 
            proposal_name, 
            passed, 
            participating_teams,
            non_participating_teams,
            counted_points,
            uncounted_points,
        } => {
            let vote_id = budget_system.import_historical_vote(
                &proposal_name,
                passed,
                participating_teams.clone(),
                non_participating_teams.clone(),
                counted_points,
                uncounted_points
            )?;

            let vote = budget_system.state().votes().get(&vote_id).unwrap();
            let proposal = budget_system.state().proposals().get(&vote.proposal_id()).unwrap();

            println!("Imported historical vote for proposal '{}' (Vote ID: {})", proposal_name, vote_id);
            println!("Vote passed: {}", passed);

            println!("\nNon-participating teams:");
            for team_name in &non_participating_teams {
                println!("  {}", team_name);
            }

            if let VoteType::Formal { raffle_id, .. } = vote.vote_type() {
                if let Some(raffle) = budget_system.state().raffles().get(&raffle_id) {
                    if let VoteParticipation::Formal { counted, uncounted } = vote.participation() {
                        println!("\nCounted seats:");
                        for &team_id in counted {
                            if let Some(team) = raffle.team_snapshots().iter().find(|s| s.id() == team_id) {
                                println!("  {} (+{} points)", team.name(), config.counted_vote_points);
                            }
                        }

                        println!("\nUncounted seats:");
                        for &team_id in uncounted {
                            if let Some(team) = raffle.team_snapshots().iter().find(|s| s.id() == team_id) {
                                println!("  {} (+{} points)", team.name(), config.uncounted_vote_points);
                            }
                        }
                    }
                } else {
                    println!("\nAssociated raffle not found. Cannot display seat breakdowns.");
                }
            } else {
                println!("\nThis is an informal vote, no counted/uncounted breakdown available.");
            }

            println!("\nNote: Detailed vote counts are not available for historical votes.");
        },
        ScriptCommand::ImportHistoricalRaffle { 
            proposal_name, 
            initiation_block, 
            randomness_block, 
            team_order, 
            excluded_teams,
            total_counted_seats, 
            max_earner_seats 
        } => {
            let (raffle_id, raffle) = budget_system.import_historical_raffle(
                &proposal_name,
                initiation_block,
                randomness_block,
                team_order.clone(),
                excluded_teams.clone(),
                total_counted_seats.or(Some(budget_system.config().default_total_counted_seats)),
                max_earner_seats.or(Some(budget_system.config().default_max_earner_seats)),
            ).await?;

            println!("Imported historical raffle for proposal '{}' (Raffle ID: {})", proposal_name, raffle_id);
            println!("Randomness: {}", raffle.config().block_randomness());

            // Print excluded teams
            if let Some(excluded) = excluded_teams {
                println!("Excluded teams: {:?}", excluded);
            }

            // Print ballot ID ranges for each team
            for snapshot in raffle.team_snapshots() {
                let tickets: Vec<_> = raffle.tickets().iter()
                    .filter(|t| t.team_id() == snapshot.id())
                    .collect();
                
                if !tickets.is_empty() {
                    let start = tickets.first().unwrap().index();
                    let end = tickets.last().unwrap().index();
                    println!("Team '{}' ballot range: {} - {}", snapshot.name(), start, end);
                }
            }

            // Print raffle results
            if let Some(result) = raffle.result() {
                println!("Counted seats:");
                println!("Earner seats:");
                let mut earner_count = 0;
                for &team_id in result.counted() {
                    if let Some(snapshot) = raffle.team_snapshots().iter().find(|s| s.id() == team_id) {
                        if let TeamStatus::Earner { .. } = snapshot.status() {
                            earner_count += 1;
                            let best_score = raffle.tickets().iter()
                                .filter(|t| t.team_id() == team_id)
                                .map(|t| t.score())
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .unwrap_or(0.0);
                            println!("  {} (score: {})", snapshot.name(), best_score);
                        }
                    }
                }
                println!("Supporter seats:");
                for &team_id in result.counted() {
                    if let Some(snapshot) = raffle.team_snapshots().iter().find(|s| s.id() == team_id) {
                        if let TeamStatus::Supporter = snapshot.status() {
                            let best_score = raffle.tickets().iter()
                                .filter(|t| t.team_id() == team_id)
                                .map(|t| t.score())
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .unwrap_or(0.0);
                            println!("  {} (score: {})", snapshot.name(), best_score);
                        }
                    }
                }
                println!("Total counted seats: {} (Earners: {}, Supporters: {})", 
                         result.counted().len(), earner_count, result.counted().len() - earner_count);

                println!("Uncounted seats:");
                println!("Earner seats:");
                for &team_id in result.uncounted() {
                    if let Some(snapshot) = raffle.team_snapshots().iter().find(|s| s.id() == team_id) {
                        if let TeamStatus::Earner { .. } = snapshot.status() {
                            let best_score = raffle.tickets().iter()
                                .filter(|t| t.team_id() == team_id)
                                .map(|t| t.score())
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .unwrap_or(0.0);
                            println!("  {} (score: {})", snapshot.name(), best_score);
                        }
                    }
                }
                println!("Supporter seats:");
                for &team_id in result.uncounted() {
                    if let Some(snapshot) = raffle.team_snapshots().iter().find(|s| s.id() == team_id) {
                        if let TeamStatus::Supporter = snapshot.status() {
                            let best_score = raffle.tickets().iter()
                                .filter(|t| t.team_id() == team_id)
                                .map(|t| t.score())
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .unwrap_or(0.0);
                            println!("  {} (score: {})", snapshot.name(), best_score);
                        }
                    }
                }
            } else {
                println!("Raffle result not available");
            }
        },
        ScriptCommand::ChangeTeamStatus { team_name, new_status, trailing_monthly_revenue } => {
            let team_id = budget_system.get_team_id_by_name(&team_name)
                .ok_or_else(|| format!("Team not found: {}", team_name))?;
            
            let new_status = match new_status.to_lowercase().as_str() {
                "earner" => {
                    let revenue = trailing_monthly_revenue
                        .ok_or("Trailing monthly revenue is required for Earner status")?;
                    TeamStatus::Earner { trailing_monthly_revenue: revenue }
                },
                "supporter" => TeamStatus::Supporter,
                "inactive" => TeamStatus::Inactive,
                _ => return Err(format!("Invalid status: {}", new_status).into()),
            };

            budget_system.update_team_status(team_id, &new_status)?;
            
            println!("Changed status of team '{}' to {:?}", team_name, new_status);
        },
        ScriptCommand::PrintTeamReport => {
            let report = budget_system.print_team_report();
            println!("{}", report);
        },
        ScriptCommand::PrintEpochState => {
            match budget_system.print_epoch_state() {
                Ok(report) => println!("{}", report),
                Err(e) => println!("Error printing epoch state: {}", e),
            }
        },
        ScriptCommand::PrintTeamVoteParticipation { team_name, epoch_name } => {
            match budget_system.print_team_vote_participation(&team_name, epoch_name.as_deref()) {
                Ok(report) => println!("{}", report),
                Err(e) => println!("Error printing team vote participation: {}", e),
            }
        },
        ScriptCommand::CloseProposal { proposal_name, resolution } => {
            let proposal_id = budget_system.get_proposal_id_by_name(&proposal_name)
                .ok_or_else(|| format!("Proposal not found: {}", proposal_name))?;
            
            let resolution = match resolution.to_lowercase().as_str() {
                "approved" => Resolution::Approved,
                "rejected" => Resolution::Rejected,
                "invalid" => Resolution::Invalid,
                "duplicate" => Resolution::Duplicate,
                "retracted" => Resolution::Retracted,
                _ => return Err(format!("Invalid resolution type: {}", resolution).into()),
            };
        
            budget_system.close_with_reason(proposal_id, &resolution)?;
            println!("Closed proposal '{}' with resolution: {:?}", proposal_name, resolution);
        },
        ScriptCommand::CreateRaffle { proposal_name, block_offset, excluded_teams } => {
            println!("Preparing raffle for proposal: {}", proposal_name);

            // PREPARATION PHASE
            let (raffle_id, tickets) = budget_system.prepare_raffle(&proposal_name, excluded_teams.clone(), &config)?;

            println!("Generated RaffleTickets:");
            for (team_name, start, end) in budget_system.group_tickets_by_team(&tickets) {
                println!("  {} ballot range [{}..{}]", team_name, start, end);
            }

            if let Some(excluded) = excluded_teams {
                println!("Excluded teams: {:?}", excluded);
            }

            let current_block = budget_system.ethereum_service().get_current_block().await?;
            println!("Current block number: {}", current_block);

            let initiation_block = current_block;

            let target_block = current_block + block_offset.unwrap_or(config.future_block_offset);
            println!("Target block for randomness: {}", target_block);

            // Wait for target block
            println!("Waiting for target block...");
            let mut last_observed_block = current_block;
            while budget_system.ethereum_service().get_current_block().await? < target_block {
                tokio::time::sleep(Duration::from_secs(1)).await;
                let new_block = budget_system.ethereum_service().get_current_block().await?;
                if new_block != last_observed_block {
                    println!("Latest observed block: {}", new_block);
                    last_observed_block = new_block;
                }
            }

            // FINALIZATION PHASE
            let randomness = budget_system.ethereum_service().get_randomness(target_block).await?;
            println!("Block randomness: {}", randomness);
            println!("Etherscan URL: https://etherscan.io/block/{}#consensusinfo", target_block);

            let raffle = budget_system.finalize_raffle(raffle_id, initiation_block, target_block, randomness).await?;

            // Print results (similar to ImportHistoricalRaffle)
            println!("Raffle results for proposal '{}' (Raffle ID: {})", proposal_name, raffle_id);

            // Print raffle results
            if let Some(result) = raffle.result() {
                println!("**Counted voters:**");
                println!("Earner teams:");
                let mut earner_count = 0;
                for &team_id in result.counted() {
                    if let Some(snapshot) = raffle.team_snapshots().iter().find(|s| s.id() == team_id) {
                        if let TeamStatus::Earner { .. } = snapshot.status() {
                            earner_count += 1;
                            let best_score = raffle.tickets().iter()
                                .filter(|t| t.team_id() == team_id)
                                .map(|t| t.score())
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .unwrap_or(0.0);
                            println!("  {} (score: {})", snapshot.name(), best_score);
                        }
                    }
                }
                println!("Supporter teams:");
                for &team_id in result.counted() {
                    if let Some(snapshot) = raffle.team_snapshots().iter().find(|s| s.id() == team_id) {
                        if let TeamStatus::Supporter = snapshot.status() {
                            let best_score = raffle.tickets().iter()
                                .filter(|t| t.team_id() == team_id)
                                .map(|t| t.score())
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .unwrap_or(0.0);
                            println!("  {} (score: {})", snapshot.name(), best_score);
                        }
                    }
                }
                println!("Total counted voters: {} (Earners: {}, Supporters: {})", 
                         result.counted().len(), earner_count, result.counted().len() - earner_count);

                println!("**Uncounted voters:**");
                println!("Earner teams:");
                for &team_id in result.uncounted() {
                    if let Some(snapshot) = raffle.team_snapshots().iter().find(|s| s.id() == team_id) {
                        if let TeamStatus::Earner { .. } = snapshot.status() {
                            let best_score = raffle.tickets().iter()
                                .filter(|t| t.team_id() == team_id)
                                .map(|t| t.score())
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .unwrap_or(0.0);
                            println!("  {} (score: {})", snapshot.name(), best_score);
                        }
                    }
                }
                println!("Supporter teams:");
                for &team_id in result.uncounted() {
                    if let Some(snapshot) = raffle.team_snapshots().iter().find(|s| s.id() == team_id) {
                        if let TeamStatus::Supporter = snapshot.status() {
                            let best_score = raffle.tickets().iter()
                                .filter(|t| t.team_id() == team_id)
                                .map(|t| t.score())
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .unwrap_or(0.0);
                            println!("  {} (score: {})", snapshot.name(), best_score);
                        }
                    }
                }
            } else {
                println!("Raffle result not available");
            }
        },
        ScriptCommand::CreateAndProcessVote { proposal_name, counted_votes, uncounted_votes, vote_opened, vote_closed } => {
            println!("Executing CreateAndProcessVote command for proposal: {}", proposal_name);
            match budget_system.create_and_process_vote(
                &proposal_name,
                counted_votes,
                uncounted_votes,
                vote_opened,
                vote_closed
            ) {
                Ok(report) => {
                    println!("Vote processed successfully for proposal: {}", proposal_name);
                    println!("Vote report:\n{}", report);
                
                    // Print point credits
                    if let Some(vote_id) = budget_system.state().votes().values()
                        .find(|v| v.proposal_id() == budget_system.get_proposal_id_by_name(&proposal_name).unwrap())
                        .map(|v| v.id())
                    {
                        let vote = budget_system.state().votes().get(&vote_id).unwrap();
                        
                        println!("\nPoints credited:");
                        if let VoteParticipation::Formal { counted, uncounted } = &vote.participation() {
                            for &team_id in counted {
                                if let Some(team) = budget_system.state().current_state().teams().get(&team_id) {
                                    println!("  {} (+{} points)", team.name(), config.counted_vote_points);
                                }
                            }
                            for &team_id in uncounted {
                                if let Some(team) = budget_system.state().current_state().teams().get(&team_id) {
                                    println!("  {} (+{} points)", team.name(), config.uncounted_vote_points);
                                }
                            }
                        }
                    } else {
                        println!("Warning: Vote not found after processing");
                    }
                },
                Err(e) => {
                    println!("Error: Failed to process vote for proposal '{}'. Reason: {}", proposal_name, e);
                }
            }
        },
        ScriptCommand::GenerateReportsForClosedProposals { epoch_name } => {
            let epoch_id = budget_system.get_epoch_id_by_name(&epoch_name)
                .ok_or_else(|| format!("Epoch not found: {}", epoch_name))?;
            
            let closed_proposals: Vec<_> = budget_system.get_proposals_for_epoch(epoch_id)
                .into_iter()
                .filter(|p| p.is_closed())
                .collect();

            for proposal in closed_proposals {
                match budget_system.generate_and_save_proposal_report(proposal.id(), &epoch_name) {
                    Ok(file_path) => println!("Report generated for proposal '{}' at {:?}", proposal.title(), file_path),
                    Err(e) => println!("Failed to generate report for proposal '{}': {}", proposal.title(), e),
                }
            }
        },
        ScriptCommand::GenerateReportForProposal { proposal_name } => {
            let current_epoch = budget_system.get_current_epoch()
                .ok_or("No active epoch")?;
            
            let proposal = budget_system.get_proposals_for_epoch(current_epoch.id())
                .into_iter()
                .find(|p| p.name_matches(&proposal_name))
                .ok_or_else(|| format!("Proposal not found in current epoch: {}", proposal_name))?;

            match budget_system.generate_and_save_proposal_report(proposal.id(), &current_epoch.name()) {
                Ok(file_path) => println!("Report generated for proposal '{}' at {:?}", proposal.title(), file_path),
                Err(e) => println!("Failed to generate report for proposal '{}': {}", proposal.title(), e),
            }
        },
        ScriptCommand::PrintPointReport { epoch_name } => {
            match budget_system.generate_point_report(epoch_name.as_deref()) {
                Ok(report) => {
                    println!("Point Report:");
                    println!("{}", report);
                },
                Err(e) => println!("Error generating point report: {}", e),
            }
        },
        ScriptCommand::CloseEpoch { epoch_name } => {
            let epoch_name_clone = epoch_name.clone(); // Clone here
            match budget_system.close_epoch(epoch_name.as_deref()) {
                Ok(_) => {
                    let epoch_info = epoch_name_clone.clone().unwrap_or("Active epoch".to_string());
                    println!("Successfully closed epoch: {}", epoch_info);
                    if let Some(epoch) = budget_system.state().epochs().values().find(|e| e.name() == epoch_name_clone.as_deref().unwrap_or("")) {
                        if let Some(reward) = epoch.reward() {
                            println!("Rewards allocated:");
                            for (team_id, team_reward) in epoch.team_rewards() {
                                if let Some(team) = budget_system.state().current_state().teams().get(team_id) {
                                    println!("  {}: {} {} ({:.2}%)", team.name(), team_reward.amount(), reward.token(), team_reward.percentage() * 100.0);
                                }
                            }
                        } else {
                            println!("No rewards were set for this epoch.");
                        }
                    }
                },
                Err(e) => println!("Failed to close epoch: {}", e),
            }
        },
        ScriptCommand::GenerateEndOfEpochReport { epoch_name } => {
            budget_system.generate_end_of_epoch_report(&epoch_name)?;
            println!("Generated End of Epoch Report for epoch: {}", epoch_name);
        },

    }
    Ok(())
}