// src/core/budget_system.rs

use crate::core::state::BudgetSystemState;
use crate::core::models::{
    Team, TeamStatus, Epoch, EpochStatus, EpochReward, TeamReward,
    Proposal, ProposalStatus, Resolution, PaymentStatus, BudgetRequestDetails,
    Raffle, RaffleConfig, RaffleResult, RaffleTicket,
    Vote, VoteType, VoteStatus, VoteChoice, VoteCount, VoteParticipation, VoteResult, get_id_by_name
};
use crate::services::ethereum::EthereumServiceTrait;
use crate::UpdateProposalDetails;
use crate::AppConfig;
use crate::core::file_system::FileSystem;
use crate::{escape_markdown};

use chrono::{DateTime, NaiveDate, Utc, TimeZone};
use uuid::Uuid;
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fs,
    path::{Path, PathBuf},
    str,
    sync::Arc,
};
use log::{info, debug, error};

pub struct BudgetSystem {
    state: BudgetSystemState,
    ethereum_service: Arc<dyn EthereumServiceTrait>,
    config: AppConfig,
}

impl BudgetSystem {
    pub async fn new(
        config: AppConfig, 
        ethereum_service: Arc<dyn EthereumServiceTrait>,
        state: Option<BudgetSystemState>
    ) -> Result<Self, Box<dyn Error>> {
        let state = state.unwrap_or_else(BudgetSystemState::new);
        Ok(Self {
            state,
            ethereum_service,
            config,
        })
    }

    pub fn state(&self) -> &BudgetSystemState {
        &self.state
    }

    pub fn config(&self) -> &AppConfig {
        &self.config
    }

    pub fn set_config(&mut self, config: AppConfig) {
        self.config = config;
    }

    pub fn get_team(&self, id: &Uuid) -> Option<&Team> {
        self.state.current_state().teams().get(id)
    }

    pub fn get_proposal(&self, id: &Uuid) -> Option<&Proposal> {
        self.state.proposals().get(id)
    }

    pub fn get_epoch(&self, id: &Uuid) -> Option<&Epoch> {
        self.state.epochs().get(id)
    }

    pub fn get_raffle(&self, id: &Uuid) -> Option<&Raffle> {
        self.state.raffles().get(id)
    }

    pub fn get_vote(&self, id: &Uuid) -> Option<&Vote> {
        self.state.votes().get(id)
    }

    pub fn create_team(&mut self, name: String, representative: String, trailing_monthly_revenue: Option<Vec<u64>>) -> Result<Uuid, Box<dyn Error>> {
        let team = Team::new(name, representative, trailing_monthly_revenue)?;
        let id = self.state.add_team(team);
        self.save_state()?;
        Ok(id)
    }

    pub fn remove_team(&mut self, team_id: Uuid) -> Result<(), Box<dyn Error>> {
        self.state.remove_team(team_id).ok_or("Team not found")?;
        self.save_state()?;
        Ok(())
    }

    pub fn update_team_status(&mut self, team_id: Uuid, new_status: &TeamStatus) -> Result<(), Box<dyn Error>> {
        let team = self.state.get_team_mut(&team_id).ok_or("Team not found")?;
        team.set_status(new_status.clone())?;
        self.save_state()?;
        Ok(())
    }

    pub fn ethereum_service(&self) -> &Arc<dyn EthereumServiceTrait> {
        &self.ethereum_service
    }

    pub async fn get_current_block(&self) -> Result<u64, Box<dyn Error>> {
        self.ethereum_service.get_current_block().await
    }

    pub async fn get_randomness(&self, block_number: u64) -> Result<String, Box<dyn Error>> {
        self.ethereum_service.get_randomness(block_number).await
    }

    pub async fn get_raffle_randomness(&self) -> Result<(u64, u64, String), Box<dyn Error>> {
        self.ethereum_service.get_raffle_randomness().await
    }

    pub fn save_state(&self) -> Result<(), Box<dyn std::error::Error>> {
        FileSystem::save_state(&self.state, &self.config.state_file)
    }

    pub fn add_proposal(&mut self, title: String, url: Option<String>, budget_request_details: Option<BudgetRequestDetails>, announced_at: Option<NaiveDate>, published_at: Option<NaiveDate>, is_historical: Option<bool>) -> Result<Uuid, &'static str> {
        let current_epoch_id = self.state.current_epoch().ok_or("No active epoch")?;
    
        let proposal = Proposal::new(current_epoch_id, title, url, budget_request_details, announced_at, published_at, is_historical);
        let proposal_id = self.state.add_proposal(&proposal);

        if let Some(epoch) = self.state.get_epoch_mut(&current_epoch_id) {
            epoch.add_proposal(proposal_id);
        } else {
            return Err("Current epoch not found");
        }
        self.save_state();
        Ok(proposal_id)
    }

    pub fn close_with_reason(&mut self, id: Uuid, resolution: &Resolution) -> Result<(), &'static str> {
        if let Some(proposal) = self.state.get_proposal_mut(&id) {
            if proposal.is_closed() {
                return Err("Proposal is already closed");
            }
            if let Some(details) = &proposal.budget_request_details() {
                if details.is_paid() {
                    return Err("Cannot close: Proposal is already paid");
                }
            }
            proposal.set_resolution(Some(resolution.clone()));
            proposal.set_status(ProposalStatus::Closed);
            self.save_state();
            Ok(())
        } else {
            Err("Proposal not found")
        }
    }

    pub fn generate_and_save_proposal_report(&self, proposal_id: Uuid, epoch_name: &str) -> Result<PathBuf, Box<dyn Error>> {
        let proposal = self.get_proposal(&proposal_id)
            .ok_or_else(|| format!("Proposal not found: {:?}", proposal_id))?;

        let report_content = self.generate_proposal_report(proposal_id)?;
        
        FileSystem::generate_and_save_proposal_report(
            proposal,
            &report_content,
            epoch_name,
            Path::new(&self.config.state_file)
        )
    }

    pub fn create_formal_vote(&mut self, proposal_id: Uuid, raffle_id: Uuid, threshold: Option<f64>) -> Result<Uuid, &'static str> {
        let proposal = self.state.get_proposal_mut(&proposal_id)
            .ok_or("Proposal not found")?;

        if !proposal.is_actionable() {
            return Err("Proposal is not in a votable state");
        }

        let epoch_id = proposal.epoch_id();

        let raffle = self.state.get_raffle(&raffle_id)
            .ok_or("Raffle not found")?;

        if raffle.result().is_none() {
            return Err("Raffle results have not been generated");
        }

        let config = raffle.config();

        let vote_type = VoteType::Formal { 
            raffle_id,
            total_eligible_seats: config.total_counted_seats() as u32,
            threshold: self.config.default_qualified_majority_threshold,
            counted_points: self.config.counted_vote_points,
            uncounted_points: self.config.uncounted_vote_points
        };

        let vote = Vote::new(proposal_id, epoch_id, vote_type, false);


        let vote_id = self.state.add_vote(&vote);
        self.save_state();
        Ok(vote_id)
    }

    pub fn create_informal_vote(&mut self, proposal_id: Uuid) -> Result<Uuid, &'static str> {
        let proposal = self.state.get_proposal_mut(&proposal_id)
            .ok_or("Proposal not found")?;

        if !proposal.is_actionable() {
            return Err("Proposal is not in a votable state");
        }

        let epoch_id = proposal.epoch_id();

        let vote = Vote::new(proposal_id, epoch_id, VoteType::Informal, false);

        let vote_id = self.state.add_vote(&vote);
        self.save_state();
        Ok(vote_id)
    }

    pub fn cast_votes(&mut self, vote_id: Uuid, votes: Vec<(Uuid, VoteChoice)>) -> Result<(), &'static str> {
        let raffle_result = {
            let vote = self.state.get_vote(&vote_id).ok_or("Vote not found")?;
            match vote.vote_type() {
                VoteType::Formal { raffle_id, .. } => {
                    self.state.get_raffle(&raffle_id)
                        .and_then(|raffle| raffle.result().cloned())
                },
                VoteType::Informal => None,
            }
        };
    
        {
            let vote = self.state.get_vote_mut(&vote_id).ok_or("Vote not found")?;
            for (team_id, choice) in votes {
                vote.cast_vote(team_id, choice, raffle_result.as_ref())?;
            }
        }
    
        self.save_state();
        Ok(())
    }

    pub fn close_vote(&mut self, vote_id: Uuid) -> Result<bool, &'static str> {
        let vote = self.state.get_vote_mut(&vote_id).ok_or("Vote not found")?;
        
        if vote.is_closed() {
            return Err("Vote is already closed");
        }

        vote.close()?;

        let result = match vote.result() {
            Some(VoteResult::Formal { passed, .. }) => *passed,
            Some(VoteResult::Informal { .. }) => false,
            None => return Err("Vote result not available"),
        };

        self.save_state();
        Ok(result)
    }

    pub fn create_epoch(&mut self, name: &str, start_date:DateTime<Utc>, end_date: DateTime<Utc>) -> Result<Uuid, &'static str> {
        let new_epoch = Epoch::new(name.to_string(), start_date, end_date)?;

        // Check for overlapping epochs
        for epoch in self.state.epochs().values() {
            if (start_date < epoch.end_date() && end_date > epoch.start_date()) ||
            (epoch.start_date() < end_date && epoch.end_date() > start_date) {
                return Err("New epoch overlaps with an existing epoch");
            }
        }

        let epoch_id = self.state.add_epoch(&new_epoch);
        self.save_state();
        Ok(epoch_id)
    }

    pub fn activate_epoch(&mut self, epoch_id: Uuid) -> Result<(), &'static str> {
        if self.state.current_epoch().is_some() {
            return Err("Another epoch is currently active");
        }

        let epoch = self.state.get_epoch_mut(&epoch_id).ok_or("Epoch not found")?;

        epoch.activate();
        self.state.set_current_epoch(Some(epoch_id));
        self.save_state();
        Ok(())
    }

    pub fn set_epoch_reward(&mut self, token: &str, amount: f64) -> Result<(), &'static str> {
        let epoch_id = self.state.current_epoch().ok_or("No active epoch")?;
        let epoch = self.state.get_epoch_mut(&epoch_id).ok_or("Epoch not found")?;
        
        epoch.set_reward(token.to_string(), amount);
        self.save_state();
        Ok(())
    }

    pub fn get_current_epoch(&self) -> Option<&Epoch> {
        self.state.current_epoch().and_then(|id| self.state.epochs().get(&id))
    }

    pub fn get_proposals_for_epoch(&self, epoch_id: Uuid) -> Vec<&Proposal> {
        if let Some(epoch) = self.state.epochs().get(&epoch_id) {
            epoch.associated_proposals().iter()
                .filter_map(|&id| self.state.proposals().get(&id))
                .collect()
        } else {
            vec![]
        }
    }

    pub fn update_epoch_dates(&mut self, epoch_id: Uuid, new_start: DateTime<Utc>, new_end: DateTime<Utc>) -> Result<(), &'static str> {
        // Check for overlaps with other epochs
        for other_epoch in self.state.epochs().values() {
            if other_epoch.id() != epoch_id &&
               ((new_start < other_epoch.end_date() && new_end > other_epoch.start_date()) ||
                (other_epoch.start_date() < new_end && other_epoch.end_date() > new_start)) {
                return Err("New dates overlap with an existing epoch");
            }
        }
        
        let epoch = self.state.get_epoch_mut(&epoch_id).ok_or("Epoch not found")?;

        if !epoch.is_planned() {
            return Err("Can only modify dates of planned epochs");
        }

        epoch.set_dates(new_start, new_end);

        Ok(())
    }

    pub fn get_team_id_by_name(&self, name: &str) -> Option<Uuid> {
        get_id_by_name(&self.state.current_state().teams(), name)
    }

    pub fn get_epoch_id_by_name(&self, name: &str) -> Option<Uuid> {
        get_id_by_name(&self.state.epochs(), name)
    }

    pub fn get_proposal_id_by_name(&self, name: &str) -> Option<Uuid> {
        get_id_by_name(&self.state.proposals(), name)
    } 

    pub fn import_predefined_raffle(
        &mut self,
        proposal_name: &str,
        counted_teams: Vec<String>,
        uncounted_teams: Vec<String>,
        total_counted_seats: usize,
        max_earner_seats: usize
    ) -> Result<Uuid, Box<dyn Error>> {
        let proposal_id = self.get_proposal_id_by_name(proposal_name)
            .ok_or_else(|| format!("Proposal not found: {}", proposal_name))?;
        
        let epoch_id = self.state.current_epoch()
            .ok_or("No active epoch")?;

        let counted_team_ids: Vec<Uuid> = counted_teams.iter()
            .filter_map(|name| self.get_team_id_by_name(name))
            .collect();
        
        let uncounted_team_ids: Vec<Uuid> = uncounted_teams.iter()
            .filter_map(|name| self.get_team_id_by_name(name))
            .collect();

        // Check if total_counted_seats matches the number of counted teams
        if counted_team_ids.len() != total_counted_seats {
            return Err(format!(
                "Mismatch between specified total_counted_seats ({}) and actual number of counted teams ({})",
                total_counted_seats, counted_team_ids.len()
            ).into());
        }

        // Additional check to ensure max_earner_seats is not greater than total_counted_seats
        if max_earner_seats > total_counted_seats {
            return Err(format!(
                "max_earner_seats ({}) cannot be greater than total_counted_seats ({})",
                max_earner_seats, total_counted_seats
            ).into());
        }

        let raffle_config = RaffleConfig::new(
            proposal_id,
            epoch_id,
            total_counted_seats,
            max_earner_seats,
            Some(0),
            Some(0),
            Some("N/A".to_string()),
            Some(Vec::new()),
            None,
            Some(counted_team_ids.iter().chain(uncounted_team_ids.iter()).cloned().collect()),
            true,
        );

        let mut raffle = Raffle::new(raffle_config, self.state.current_state().teams())?;
        raffle.set_result(RaffleResult::new(counted_team_ids, uncounted_team_ids));

        let raffle_id = self.state.add_raffle(&raffle);
        self.save_state()?;

        Ok(raffle_id)
    }

    pub fn import_historical_vote(
        &mut self,
        proposal_name: &str,
        passed: bool,
        participating_teams: Vec<String>,
        non_participating_teams: Vec<String>,
        counted_points: Option<u32>,
        uncounted_points: Option<u32>
    ) -> Result<Uuid, Box<dyn Error>> {
        let proposal_id = self.get_proposal_id_by_name(proposal_name)
            .ok_or_else(|| format!("Proposal not found: {}", proposal_name))?;
    
        let raffle_id = self.state.raffles().iter()
            .find(|(_, raffle)| raffle.config().proposal_id() == proposal_id)
            .map(|(id, _)| *id)
            .ok_or_else(|| format!("No raffle found for proposal: {}", proposal_name))?;

        let raffle = self.state.get_raffle(&raffle_id)
            .ok_or_else(|| format!("Raffle not found: {}", raffle_id))?;
    
        let epoch_id = raffle.config().epoch_id();
    
        let vote_type = VoteType::Formal {
            raffle_id,
            total_eligible_seats: raffle.config().total_counted_seats() as u32,
            threshold: self.config.default_qualified_majority_threshold,
            counted_points: counted_points.unwrap_or(self.config.counted_vote_points),
            uncounted_points: uncounted_points.unwrap_or(self.config.uncounted_vote_points)
        };
    
        let mut vote = Vote::new(proposal_id, epoch_id, vote_type, true);
    
        // Determine participation
        let (participating_ids, _) = self.determine_participation(
            raffle,
            &participating_teams,
            &non_participating_teams
        )?;
    
        let raffle_result = raffle.result().ok_or("Raffle result not found")?;
    
        // Set participation without casting actual votes
        for &team_id in &participating_ids {
            if raffle_result.counted().contains(&team_id) {
                vote.add_participant(team_id, true)?;
            } else if raffle_result.uncounted().contains(&team_id) {
                vote.add_participant(team_id, false)?;
            }
        }
    
        // Close the vote
        vote.close()?;
    
        // Set the result manually for historical votes
        let result = VoteResult::Formal {
            counted: VoteCount::new(),  // All zeros
            uncounted: VoteCount::new(),  // All zeros
            passed,
        };
        vote.set_result(Some(result));
    
        // Set dates (using current time as a placeholder)
        let now = Utc::now();
        vote.set_opened_at(now);
        vote.set_closed_at(Some(now));
    
        let vote_id = self.state.add_vote(&vote);
    
        // Update proposal status based on vote result
        let proposal = self.state.get_proposal_mut(&proposal_id)
            .ok_or_else(|| format!("Proposal not found: {}", proposal_id))?;
        
        if passed {
            proposal.approve()?;
        } else {
            proposal.reject()?;
        }
        proposal.set_status(ProposalStatus::Closed);
    
        self.save_state()?;
    
        Ok(vote_id)
    }

    pub fn determine_participation(
        &self,
        raffle: &Raffle,
        participating_teams: &[String],
        non_participating_teams: &[String]
    ) -> Result<(Vec<Uuid>, Vec<Uuid>), Box<dyn Error>> {
        let raffle_result = raffle.result()
            .ok_or("Raffle result not found")?;

        let all_team_ids: Vec<Uuid> = raffle_result.counted().iter()
            .chain(raffle_result.uncounted().iter())
            .cloned()
            .collect();

        if !participating_teams.is_empty() {
            let participating_ids: Vec<Uuid> = participating_teams.iter()
                .filter_map(|name| self.get_team_id_by_name(name))
                .collect();
            let non_participating_ids: Vec<Uuid> = all_team_ids.into_iter()
                .filter(|id| !participating_ids.contains(id))
                .collect();
            Ok((participating_ids, non_participating_ids))
        } else if !non_participating_teams.is_empty() {
            let non_participating_ids: Vec<Uuid> = non_participating_teams.iter()
                .filter_map(|name| self.get_team_id_by_name(name))
                .collect();
            let participating_ids: Vec<Uuid> = all_team_ids.into_iter()
                .filter(|id| !non_participating_ids.contains(id))
                .collect();
            Ok((participating_ids, non_participating_ids))
        } else {
            Ok((all_team_ids, Vec::new()))
        }
    }

    pub fn print_team_report(&self) -> String {
        let mut teams: Vec<&Team> = self.state.current_state().teams().values().collect();
        teams.sort_by(|a, b| a.name().cmp(&b.name()));

        let mut report = String::from("Team Report:\n\n");

        for team in teams {
            report.push_str(&format!("Name: {}\n", team.name()));
            report.push_str(&format!("ID: {}\n", team.id()));
            report.push_str(&format!("Representative: {}\n", team.representative()));
            report.push_str(&format!("Status: {:?}\n", team.status()));

            if let TeamStatus::Earner { trailing_monthly_revenue } = &team.status() {
                report.push_str(&format!("Trailing Monthly Revenue: {:?}\n", trailing_monthly_revenue));
            }

            // Add a breakdown of points per epoch
            report.push_str("Points per Epoch:\n");
            for epoch in self.state.epochs().values() {
                let epoch_points = self.get_team_points_for_epoch(team.id(), epoch.id()).unwrap_or(0);
                report.push_str(&format!("  {}: {} points\n", epoch.name(), epoch_points));
            }

            report.push_str("\n");
        }

        report
    }

    pub fn print_epoch_state(&self) -> Result<String, Box<dyn Error>> {
        let epoch = self.get_current_epoch().ok_or("No active epoch")?;
        let proposals = self.get_proposals_for_epoch(epoch.id());

        let mut report = String::new();

        // Epoch overview
        report.push_str(&format!("*State of Epoch {}*\n\n", escape_markdown(&epoch.name())));
        report.push_str("🌍 *Overview*\n");
        report.push_str(&format!("ID: `{}`\n", epoch.id()));
        report.push_str(&format!("Start Date: `{}`\n", epoch.start_date().format("%Y-%m-%d %H:%M:%S UTC")));
        report.push_str(&format!("End Date: `{}`\n", epoch.end_date().format("%Y-%m-%d %H:%M:%S UTC")));
        report.push_str(&format!("Status: `{:?}`\n", epoch.status()));

        if let Some(reward) = epoch.reward() {
            report.push_str(&format!("Epoch Reward: `{} {}`\n", reward.amount(), escape_markdown(reward.token())));
        } else {
            report.push_str("Epoch Reward: `Not set`\n");
        }

        report.push_str("\n");

        // Proposal counts
        let mut open_proposals = Vec::new();
        let mut approved_count = 0;
        let mut rejected_count = 0;
        let mut retracted_count = 0;

        for proposal in &proposals {
            match proposal.resolution() {
                Some(Resolution::Approved) => approved_count += 1,
                Some(Resolution::Rejected) => rejected_count += 1,
                Some(Resolution::Retracted) => retracted_count += 1,
                _ => {
                    if proposal.is_actionable() {
                        open_proposals.push(proposal);
                    }
                }
            }
        }

        report.push_str("📊 *Proposals*\n");
        report.push_str(&format!("Total: `{}`\n", proposals.len()));
        report.push_str(&format!("Open: `{}`\n", open_proposals.len()));
        report.push_str(&format!("Approved: `{}`\n", approved_count));
        report.push_str(&format!("Rejected: `{}`\n", rejected_count));
        report.push_str(&format!("Retracted: `{}`\n", retracted_count));

        report.push_str("\n");

        // Open proposals
        if !open_proposals.is_empty() {
            report.push_str("📬 *Open proposals*\n\n");
        
            for proposal in open_proposals {
                report.push_str(&format!("*{}*\n", escape_markdown(proposal.title())));
                if let Some(url) = proposal.url() {
                    report.push_str(&format!("🔗 {}\n", escape_markdown(url)));
                }
                if let Some(details) = proposal.budget_request_details() {
                    if let (Some(start), Some(end)) = (details.start_date(), details.end_date()) {
                        report.push_str(&format!("📆 {} \\- {}\n", 
                            escape_markdown(&start.format("%b %d").to_string()),
                            escape_markdown(&end.format("%b %d").to_string())
                        ));
                    }
                    if !details.request_amounts().is_empty() {
                        let amounts: Vec<String> = details.request_amounts().iter()
                            .map(|(token, amount)| format!("{} {}", 
                                escape_markdown(&amount.to_string()), 
                                escape_markdown(token)
                            ))
                            .collect();
                        report.push_str(&format!("💰 {}\n", amounts.join(", ")));
                    }
                }
                let days_open = self.days_open(proposal);
                report.push_str(&format!("⏳ _{} days open_\n\n", escape_markdown(&days_open.to_string())));
            }
        }

        Ok(report)
    }

    pub fn print_team_vote_participation(&self, team_name: &str, epoch_name: Option<&str>) -> Result<String, Box<dyn Error>> {
        let team_id = self.get_team_id_by_name(team_name)
            .ok_or_else(|| format!("Team not found: {}", team_name))?;
    
        let epoch = if let Some(name) = epoch_name {
            self.state.epochs().values()
                .find(|e| e.name() == name)
                .ok_or_else(|| format!("Epoch not found: {}", name))?
        } else {
            self.get_current_epoch()
                .ok_or("No active epoch and no epoch specified")?
        };
    
        let mut report = format!("Vote Participation Report for Team: {}\n", team_name);
        report.push_str(&format!("Epoch: {} ({})\n\n", epoch.name(), epoch.id()));
        let mut vote_reports = Vec::new();
        let mut total_points = 0;
    
        for vote_id in epoch.associated_proposals().iter()
            .filter_map(|proposal_id| self.state.votes().values()
                .find(|v| v.proposal_id() == *proposal_id)
                .map(|v| v.id())) 
        {
            let vote = self.state.get_vote(&vote_id).expect("Could not get Vote");
            let (participation_status, points) = match (vote.vote_type(), vote.participation()) {
                (VoteType::Formal { counted_points, uncounted_points, .. }, VoteParticipation::Formal { counted, uncounted }) => {
                    if counted.contains(&team_id) {
                        (Some("Counted"), *counted_points)
                    } else if uncounted.contains(&team_id) {
                        (Some("Uncounted"), *uncounted_points)
                    } else {
                        (None, 0)
                    }
                },
                (VoteType::Informal, VoteParticipation::Informal(participants)) => {
                    if participants.contains(&team_id) {
                        (Some("N/A (Informal)"), 0)
                    } else {
                        (None, 0)
                    }
                },
                _ => (None, 0),
            };
    
            if let Some(status) = participation_status {
                let proposal = self.state.proposals().get(&vote.proposal_id())
                    .ok_or_else(|| format!("Proposal not found for vote: {}", vote_id))?;
    
                let vote_type = match vote.vote_type() {
                    VoteType::Formal { .. } => "Formal",
                    VoteType::Informal => "Informal",
                };
    
                let result = match vote.result() {
                    Some(VoteResult::Formal { passed, .. }) => if *passed { "Passed" } else { "Failed" },
                    Some(VoteResult::Informal { .. }) => "N/A (Informal)",
                    None => "Pending",
                };
    
                total_points += points;
    
                vote_reports.push((
                    vote.opened_at(),
                    format!(
                        "Vote ID: {}\n\
                        Proposal: {}\n\
                        Type: {}\n\
                        Participation: {}\n\
                        Result: {}\n\
                        Points Earned: {}\n\n",
                        vote_id, proposal.title(), vote_type, status, result, points
                    )
                ));
            }
        }
    
        // Sort vote reports by date, most recent first
        vote_reports.sort_by(|a, b| b.0.cmp(&a.0));
    
        // Add total points to the report
        report.push_str(&format!("Total Points Earned: {}\n\n", total_points));
    
        // Add individual vote reports
        for (_, vote_report) in &vote_reports {
            report.push_str(vote_report);
        }
    
        if vote_reports.is_empty() {
            report.push_str("This team has not participated in any votes during this epoch.\n");
        }
    
        Ok(report)
    }

    pub fn days_open(&self, proposal: &Proposal) -> i64 {
        let announced_date = proposal.announced_at()
            .unwrap_or_else(|| Utc::now().date_naive());
        Utc::now().date_naive().signed_duration_since(announced_date).num_days()
    }

    pub fn prepare_raffle(&mut self, proposal_name: &str, excluded_teams: Option<Vec<String>>, app_config: &AppConfig) -> Result<(Uuid, Vec<RaffleTicket>), Box<dyn Error>> {
        let proposal_id = self.get_proposal_id_by_name(proposal_name)
            .ok_or_else(|| format!("Proposal not found: {}", proposal_name))?;
        let epoch_id = self.state.current_epoch()
            .ok_or("No active epoch")?;

        let excluded_team_ids = excluded_teams.map(|names| {
            names.into_iter()
                .filter_map(|name| self.get_team_id_by_name(&name))
                .collect::<Vec<Uuid>>()
        }).unwrap_or_else(Vec::new);

        let raffle_config = RaffleConfig::new(
            proposal_id,
            epoch_id,
            app_config.default_total_counted_seats,
            app_config.default_max_earner_seats,
            Some(0),
            Some(0),
            Some(String::new()),
            Some(excluded_team_ids),
            None,
            None,
            false
        );

        let raffle = Raffle::new(raffle_config, &self.state.current_state().teams())?;
        let tickets = raffle.tickets().to_vec();
        let raffle_id = self.state.add_raffle(&raffle);
        self.save_state()?;

        Ok((raffle_id, tickets))
    }

    pub async fn import_historical_raffle(
        &mut self,
        proposal_name: &str,
        initiation_block: u64,
        randomness_block: u64,
        team_order: Option<Vec<String>>,
        excluded_teams: Option<Vec<String>>,
        total_counted_seats: Option<usize>,
        max_earner_seats: Option<usize>
    ) -> Result<(Uuid, Raffle), Box<dyn Error>> {
        let proposal_id = self.get_proposal_id_by_name(proposal_name)
            .ok_or_else(|| format!("Proposal not found: {}", proposal_name))?;
    
        let epoch_id = self.state.current_epoch()
            .ok_or("No active epoch")?;
    
        let randomness = self.ethereum_service.get_randomness(randomness_block).await?;
    
        let custom_team_order = team_order.map(|order| {
            order.into_iter()
                .filter_map(|name| self.get_team_id_by_name(&name))
                .collect::<Vec<Uuid>>()
        });
    
        let excluded_team_ids = excluded_teams.map(|names| {
            names.into_iter()
                .filter_map(|name| self.get_team_id_by_name(&name))
                .collect::<Vec<Uuid>>()
        }).unwrap_or_else(Vec::new);
    
        let total_counted_seats = total_counted_seats.unwrap_or(self.config.default_total_counted_seats);
        let max_earner_seats = max_earner_seats.unwrap_or(self.config.default_max_earner_seats);
    
        if max_earner_seats > total_counted_seats {
            return Err("max_earner_seats cannot be greater than total_counted_seats".into());
        }

        let raffle_config = RaffleConfig::new(
            proposal_id,
            epoch_id,
            total_counted_seats,
            max_earner_seats,
            Some(initiation_block),
            Some(randomness_block),
            Some(randomness),
            Some(excluded_team_ids),
            None,
            custom_team_order,
            true
        );
    
        let mut raffle = Raffle::new(raffle_config, self.state.current_state().teams())?;
        raffle.generate_ticket_scores()?;
        raffle.select_deciding_teams();
    
        let raffle_id = self.state.add_raffle(&raffle);
        self.save_state()?;
    
        Ok((raffle_id, raffle))
    }

    pub async fn finalize_raffle(&mut self, raffle_id: Uuid, initiation_block: u64, randomness_block: u64, randomness: String) -> Result<Raffle, Box<dyn Error>> {
        let raffle = self.state.get_raffle_mut(&raffle_id)
            .ok_or_else(|| format!("Raffle not found: {}", raffle_id))?;
    
        raffle.config_mut().set_initiation_block(initiation_block);
        raffle.config_mut().set_randomness_block(randomness_block);
        raffle.config_mut().set_block_randomness(randomness);
    
        raffle.generate_ticket_scores()?;
        raffle.select_deciding_teams();
    
        let raffle_clone = raffle.clone();
        self.save_state()?;
    
        Ok(raffle_clone)
    }

    pub fn group_tickets_by_team(&self, tickets: &[RaffleTicket]) -> Vec<(String, u64, u64)> {
        let mut grouped_tickets: Vec<(String, u64, u64)> = Vec::new();
        let mut current_team: Option<(String, u64, u64)> = None;

        for ticket in tickets {
            let team_name = self.state.current_state().teams().get(&ticket.team_id())
                .map(|team| team.name().to_string())
                .unwrap_or_else(|| format!("Unknown Team ({})", ticket.team_id()));

            match &mut current_team {
                Some((name, _, end)) if *name == team_name => {
                    *end = ticket.index();
                }
                _ => {
                    if let Some(team) = current_team.take() {
                        grouped_tickets.push(team);
                    }
                    current_team = Some((team_name, ticket.index(), ticket.index()));
                }
            }
        }

        if let Some(team) = current_team {
            grouped_tickets.push(team);
        }

        grouped_tickets
    }

    pub fn create_and_process_vote(
        &mut self,
        proposal_name: &str,
        counted_votes: HashMap<String, VoteChoice>,
        uncounted_votes: HashMap<String, VoteChoice>,
        vote_opened: Option<NaiveDate>,
        vote_closed: Option<NaiveDate>,
    ) -> Result<String, Box<dyn Error>> {
        // Find proposal and raffle
        let (proposal_id, raffle_id) = self.find_proposal_and_raffle(proposal_name)
            .map_err(|e| format!("Failed to find proposal or raffle: {}", e))?;
        
        // Check if the proposal already has a resolution
        let proposal = self.state.get_proposal_mut(&proposal_id)
            .ok_or_else(|| "Proposal not found after ID lookup".to_string())?;
        if proposal.resolution().is_some() {
            return Err("Cannot create vote: Proposal already has a resolution".into());
        }

        // Validate votes
        self.validate_votes(raffle_id, &counted_votes, &uncounted_votes)
            .map_err(|e| format!("Vote validation failed: {}", e))?;
    
        // Create vote
        let vote_id = self.create_formal_vote(proposal_id, raffle_id, None)
            .map_err(|e| format!("Failed to create formal vote: {}", e))?;
    
        // Cast votes
        let all_votes: Vec<(Uuid, VoteChoice)> = counted_votes.into_iter()
            .chain(uncounted_votes)
            .filter_map(|(team_name, choice)| {
                self.get_team_id_by_name(&team_name).map(|id| (id, choice))
            })
            .collect();
        self.cast_votes(vote_id, all_votes)
            .map_err(|e| format!("Failed to cast votes: {}", e))?;
    
        // Update vote dates
        self.update_vote_dates(vote_id, vote_opened, vote_closed)
            .map_err(|e| format!("Failed to update vote dates: {}", e))?;
    
        // Close vote and update proposal
        let passed = self.close_vote_and_update_proposal(vote_id, proposal_id, vote_closed)
            .map_err(|e| format!("Failed to close vote or update proposal: {}", e))?;

        // Generate report
        self.generate_vote_report(vote_id)
    }
    
    pub fn find_proposal_and_raffle(&self, proposal_name: &str) -> Result<(Uuid, Uuid), Box<dyn Error>> {
        let proposal_id = self.get_proposal_id_by_name(proposal_name)
            .ok_or_else(|| format!("Proposal not found: {}", proposal_name))?;
        
        let raffle_id = self.state.raffles().iter()
            .find(|(_, raffle)| raffle.config().proposal_id() == proposal_id)
            .map(|(id, _)| *id)
            .ok_or_else(|| format!("No raffle found for proposal: {}", proposal_name))?;
        Ok((proposal_id, raffle_id))
    }
    
    pub fn validate_votes(
        &self,
        raffle_id: Uuid,
        counted_votes: &HashMap<String, VoteChoice>,
        uncounted_votes: &HashMap<String, VoteChoice>,
    ) -> Result<(), Box<dyn Error>> {
        let raffle = self.state.raffles().get(&raffle_id)
            .ok_or_else(|| format!("Raffle not found: {}", raffle_id))?;
    
        if !raffle.is_completed() {
            return Err("Raffle has not been conducted yet".into());
        }
    
        self.validate_votes_against_raffle(raffle, counted_votes, uncounted_votes)
    }
    
    pub fn update_vote_dates(
        &mut self,
        vote_id: Uuid,
        vote_opened: Option<NaiveDate>,
        vote_closed: Option<NaiveDate>,
    ) -> Result<(), Box<dyn Error>> {
        let vote = self.state.get_vote_mut(&vote_id).ok_or("Vote not found")?;
        
        if let Some(opened) = vote_opened {
            let opened_datetime = opened.and_hms_opt(0, 0, 0)
                .map(|naive| Utc.from_utc_datetime(&naive))
                .ok_or("Invalid opened date")?;
            vote.set_opened_at(opened_datetime);
        }
        
        if let Some(closed) = vote_closed {
            let closed_datetime = closed.and_hms_opt(23, 59, 59)
                .map(|naive| Utc.from_utc_datetime(&naive))
                .ok_or("Invalid closed date")?;
            vote.set_closed_at(Some(closed_datetime));
        }
        
        Ok(())
    }
    
    pub fn close_vote_and_update_proposal(
        &mut self,
        vote_id: Uuid,
        proposal_id: Uuid,
        vote_closed: Option<NaiveDate>,
    ) -> Result<bool, Box<dyn Error>> {
        let passed = self.close_vote(vote_id)?;
        
        let proposal = self.state.get_proposal_mut(&proposal_id)
            .ok_or_else(|| format!("Proposal not found: {}", proposal_id))?;
        
        println!("Proposal status before update: {:?}", proposal.status());
        println!("Proposal resolution before update: {:?}", proposal.resolution());
        
        let result = if passed {
            proposal.approve()
        } else {
            proposal.reject()
        };
    
        match result {
            Ok(()) => {
                if let Some(closed) = vote_closed {
                    proposal.set_resolved_at(Some(closed));
                }
                println!("Proposal status after update: {:?}", proposal.status());
                println!("Proposal resolution after update: {:?}", proposal.resolution());
                self.save_state()?;
                Ok(passed)
            },
            Err(e) => {
                println!("Error updating proposal: {}", e);
                println!("Current proposal state: {:?}", proposal);
                Err(format!("Failed to update proposal: {}", e).into())
            }
        }
    }

    pub fn generate_vote_report(&self, vote_id: Uuid) -> Result<String, Box<dyn Error>> {
        let vote = self.state.get_vote(&vote_id).ok_or("Vote not found")?;
        let proposal = self.state.proposals().get(&vote.proposal_id()).ok_or("Proposal not found")?;
        let raffle = self.state.raffles().values()
            .find(|r| r.config().proposal_id() == vote.proposal_id())
            .ok_or("Associated raffle not found")?;
    
        let (counted, uncounted) = vote.vote_counts().ok_or("Vote counts not available")?;
        let counted_yes = counted.yes();
        let counted_no = counted.no();
        let total_counted_votes = counted_yes + counted_no;
        
        let total_eligible_seats = match vote.vote_type() {
            VoteType::Formal { total_eligible_seats, .. } => total_eligible_seats,
            _ => &0,
        };
    
        // Calculate absent votes for counted seats only
        let absent = total_eligible_seats.saturating_sub(total_counted_votes as u32);

        let status = match vote.result() {
            Some(VoteResult::Formal { passed, .. }) => if *passed { "Approved" } else { "Not Approved" },
            Some(VoteResult::Informal { .. }) => "N/A (Informal)",
            None => "Pending",
        };
    
        let deciding_teams: Vec<String> = raffle.deciding_teams().iter()
            .filter_map(|&team_id| {
                self.state.current_state().teams().get(&team_id).map(|team| team.name().to_string())
            })
            .collect();
    
        // Calculate uncounted votes
        let total_uncounted_votes = uncounted.yes() + uncounted.no();
        let total_uncounted_seats = raffle.result()
            .map(|result| result.uncounted().len())
            .unwrap_or(0) as u32;

        let (counted_votes_info, uncounted_votes_info) = if let VoteParticipation::Formal { counted, uncounted } = &vote.participation() {
            let absent_counted: Vec<String> = raffle.result().expect("Raffle result not found").counted().iter()
                .filter(|&team_id| !counted.contains(team_id))
                .filter_map(|&team_id| self.state.current_state().teams().get(&team_id).map(|team| team.name().to_string()))
                .collect();

            let absent_uncounted: Vec<String> = raffle.result().expect("Raffle result not found").uncounted().iter()
                .filter(|&team_id| !uncounted.contains(team_id))
                .filter_map(|&team_id| self.state.current_state().teams().get(&team_id).map(|team| team.name().to_string()))
                .collect();

            let counted_info = if absent_counted.is_empty() {
                format!("Counted votes cast: {}/{}", total_counted_votes, total_eligible_seats)
            } else {
                format!("Counted votes cast: {}/{} ({} absent)", total_counted_votes, total_eligible_seats, absent_counted.join(", "))
            };

            let uncounted_info = if absent_uncounted.is_empty() {
                format!("Uncounted votes cast: {}/{}", total_uncounted_votes, total_uncounted_seats)
            } else {
                format!("Uncounted votes cast: {}/{} ({} absent)", total_uncounted_votes, total_uncounted_seats, absent_uncounted.join(", "))
            };

            (counted_info, uncounted_info)
        } else {
            (
                format!("Counted votes cast: {}/{}", total_counted_votes, total_eligible_seats),
                format!("Uncounted votes cast: {}/{}", total_uncounted_votes, total_uncounted_seats)
            )
        };
    
    
        let report = format!(
            "**{}**\n{}\n\n**Status: {}**\n__{} in favor, {} against, {} absent__\n\n**Deciding teams**\n`{:?}`\n\n{}\n{}",
            proposal.title(),
            proposal.url().as_deref().unwrap_or(""),
            status,
            counted_yes,
            counted_no,
            absent,
            deciding_teams,
            counted_votes_info,
            uncounted_votes_info
        );
    
        Ok(report)
    }

    pub fn validate_votes_against_raffle(
        &self,
        raffle: &Raffle,
        counted_votes: &HashMap<String, VoteChoice>,
        uncounted_votes: &HashMap<String, VoteChoice>,
    ) -> Result<(), Box<dyn Error>> {
        let raffle_result = raffle.result().ok_or("Raffle result not found")?;
    
        let counted_team_ids: HashSet<_> = raffle_result.counted().iter().cloned().collect();
        let uncounted_team_ids: HashSet<_> = raffle_result.uncounted().iter().cloned().collect();
    
        for team_name in counted_votes.keys() {
            let team_id = self.get_team_id_by_name(team_name)
                .ok_or_else(|| format!("Team not found: {}", team_name))?;
            if !counted_team_ids.contains(&team_id) {
                return Err(format!("Team {} is not eligible for counted vote", team_name).into());
            }
        }
    
        for team_name in uncounted_votes.keys() {
            let team_id = self.get_team_id_by_name(team_name)
                .ok_or_else(|| format!("Team not found: {}", team_name))?;
            if !uncounted_team_ids.contains(&team_id) {
                return Err(format!("Team {} is not eligible for uncounted vote", team_name).into());
            }
        }
    
        Ok(())
    }

    pub fn update_proposal(&mut self, proposal_name: &str, updates: UpdateProposalDetails) -> Result<(), &'static str> {
        // Find the team_id if it's needed
        let team_id = if let Some(budget_details) = &updates.budget_request_details {
            if let Some(team_name) = &budget_details.team {
                self.get_team_id_by_name(team_name)
            } else {
                None
            }
        } else {
            None
        };
    
        // Update the proposal
        let proposal_id = self.get_proposal_id_by_name(proposal_name).ok_or("Name not matching a proposal")?;
        let proposal = self.state.get_proposal_mut(&proposal_id).ok_or("Proposal not found")?;
    
        proposal.update(updates, team_id)?;
    
        self.save_state();
        Ok(())
    }

    pub fn generate_markdown_test(&self) -> String {
        let test_message = r#"
*Bold text*
_Italic text_
__Underline__
~Strikethrough~
*Bold _italic bold ~italic bold strikethrough~ __underline italic bold___ bold*
[inline URL](http://www.example.com/)
[inline mention of a user](tg://user?id=123456789)
`inline fixed-width code`
```python
def hello_world():
    print("Hello, World!")
```
"#;
        test_message.to_string()
    }

    pub fn generate_proposal_report(&self, proposal_id: Uuid) -> Result<String, Box<dyn Error>> {
        debug!("Generating proposal report for ID: {:?}", proposal_id);
    
        let proposal = self.state.get_proposal(&proposal_id)
            .ok_or_else(|| format!("Proposal not found: {:?}", proposal_id))?;
    
        debug!("Found proposal: {:?}", proposal.title());
    
        let mut report = String::new();
    
        // Main title (moved outside of Summary)
        report.push_str(&format!("# Proposal Report: {}\n\n", proposal.title()));
    
        // Summary
        report.push_str("## Summary\n\n");
        if let (Some(announced), Some(resolved)) = (proposal.announced_at(), proposal.resolved_at()) {
            let resolution_days = self.calculate_days_between(announced, resolved);
            report.push_str(&format!("This proposal was resolved in {} days from its announcement date. ", resolution_days));
        }
    
        if let Some(vote) = self.state.votes().values().find(|v| v.proposal_id() == proposal_id) {
            if let Some(result) = vote.result() {
                match result {
                    VoteResult::Formal { counted, uncounted, passed } => {
                        report.push_str(&format!("The proposal was {} with {} votes in favor and {} votes against. ", 
                            if *passed { "approved" } else { "not approved" }, 
                            counted.yes(), counted.yes() + uncounted.yes()));
                    },
                    VoteResult::Informal { count } => {
                        report.push_str(&format!("This was an informal vote with {} votes in favor and {} votes against. ", 
                            count.yes(), count.no()));
                    }
                }
            }
        } else {
            report.push_str("No voting information is available for this proposal. ");
        }
    
        if let Some(budget_details) = proposal.budget_request_details() {
            report.push_str(&format!("The budget request was for {} {} for the period from {} to {}. ",
                budget_details.request_amounts().values().sum::<f64>(),
                budget_details.request_amounts().keys().next().unwrap_or(&String::new()),
                budget_details.start_date().map_or("N/A".to_string(), |d| d.format("%Y-%m-%d").to_string()),
                budget_details.end_date().map_or("N/A".to_string(), |d| d.format("%Y-%m-%d").to_string())
            ));
        }
    
        report.push_str("\n\n");
    
        // Proposal Details
        report.push_str("## Proposal Details\n\n");
        report.push_str(&format!("- **ID**: {}\n", proposal.id()));
        report.push_str(&format!("- **Title**: {}\n", proposal.title()));
        report.push_str(&format!("- **URL**: {}\n", proposal.url().as_deref().unwrap_or("N/A")));
        report.push_str(&format!("- **Status**: {:?}\n", proposal.status()));
        report.push_str(&format!("- **Resolution**: {}\n", proposal.resolution().as_ref().map_or("N/A".to_string(), |r| format!("{:?}", r))));
        report.push_str(&format!("- **Announced**: {}\n", proposal.announced_at().map_or("N/A".to_string(), |d| d.format("%Y-%m-%d").to_string())));
        report.push_str(&format!("- **Published**: {}\n", proposal.published_at().map_or("N/A".to_string(), |d| d.format("%Y-%m-%d").to_string())));
        report.push_str(&format!("- **Resolved**: {}\n", proposal.resolved_at().map_or("N/A".to_string(), |d| d.format("%Y-%m-%d").to_string())));
        report.push_str(&format!("- **Is Historical**: {}\n\n", proposal.is_historical()));
    
        // Budget Request Details
        if let Some(budget_details) = proposal.budget_request_details() {
            report.push_str("## Budget Request Details\n\n");
            report.push_str(&format!("- **Requesting Team**: {}\n", 
                budget_details.team()
                    .and_then(|id| self.state.current_state().teams().get(&id))
                    .map_or("N/A".to_string(), |team| team.name().to_string())));
            report.push_str("- **Requested Amount(s)**:\n");
            for (token, amount) in budget_details.request_amounts() {
                report.push_str(&format!("  - {}: {}\n", token, amount));
            }
            report.push_str(&format!("- **Start Date**: {}\n", budget_details.start_date().map_or("N/A".to_string(), |d| d.format("%Y-%m-%d").to_string())));
            report.push_str(&format!("- **End Date**: {}\n", budget_details.end_date().map_or("N/A".to_string(), |d| d.format("%Y-%m-%d").to_string())));
            report.push_str(&format!("- **Payment Status**: {:?}\n\n", budget_details.payment_status()));
        }
    
        // Raffle Information
        if let Some(raffle) = self.state.raffles().values().find(|r| r.config().proposal_id() == proposal_id) {
            report.push_str("## Raffle Information\n\n");
            report.push_str(&format!("- **Raffle ID**: {}\n", raffle.id()));
            report.push_str(&format!("- **Initiation Block**: {}\n", raffle.config().initiation_block()));
            report.push_str(&format!("- **Randomness Block**: [{}]({})\n", 
                raffle.config().randomness_block(), raffle.etherscan_url()));
            report.push_str(&format!("- **Block Randomness**: {}\n", raffle.config().block_randomness()));
            report.push_str(&format!("- **Total Counted Seats**: {}\n", raffle.config().total_counted_seats()));
            report.push_str(&format!("- **Max Earner Seats**: {}\n", raffle.config().max_earner_seats()));
            report.push_str(&format!("- **Is Historical**: {}\n\n", raffle.config().is_historical()));
    
            // Team Snapshots
            report.push_str(&self.generate_team_snapshots_table(raffle));
    
            // Raffle Outcome
            if let Some(result) = raffle.result() {
                report.push_str("### Raffle Outcome\n\n");
                self.generate_raffle_outcome(&mut report, raffle, result);
            }
        } else {
            report.push_str("## Raffle Information\n\nNo raffle was conducted for this proposal.\n\n");
        }
    
        // Voting Information
        if let Some(vote) = self.state.votes().values().find(|v| v.proposal_id() == proposal_id) {
            report.push_str("## Voting Information\n\n");
            report.push_str("### Vote Details\n\n");
            report.push_str(&format!("- **Vote ID**: {}\n", vote.id()));
            report.push_str(&format!("- **Type**: {:?}\n", vote.vote_type()));
            report.push_str(&format!("- **Status**: {:?}\n", vote.status()));
            report.push_str(&format!("- **Opened**: {}\n", vote.opened_at().format("%Y-%m-%d %H:%M:%S")));
            if let Some(closed_at) = vote.closed_at() {
                report.push_str(&format!("- **Closed**: {}\n", closed_at.format("%Y-%m-%d %H:%M:%S")));
            }
            if let Some(result) = vote.result() {
                match result {
                    VoteResult::Formal { passed, .. } => {
                        report.push_str(&format!("- **Result**: {}\n\n", if *passed { "Passed" } else { "Not Passed" }));
                    },
                    VoteResult::Informal { .. } => {
                        report.push_str("- **Result**: Informal (No Pass/Fail)\n\n");
                    }
                }
            }
    
            // Participation
            report.push_str("### Participation\n\n");
            report.push_str(&self.generate_vote_participation_tables(vote));
    
            // Vote Counts
            if !vote.is_historical() {
                report.push_str("### Vote Counts\n");
                match vote.vote_type() {
                    VoteType::Formal { total_eligible_seats, .. } => {
                        if let Some(VoteResult::Formal { counted, uncounted, .. }) = vote.result() {
                            let absent = *total_eligible_seats as i32 - (counted.yes() + counted.no()) as i32;
                            
                            report.push_str("#### Counted Votes\n");
                            report.push_str(&format!("- **Yes**: {}\n", counted.yes()));
                            report.push_str(&format!("- **No**: {}\n", counted.no()));
                            if absent > 0 {
                                report.push_str(&format!("- **Absent**: {}\n", absent));
                            }
    
                            report.push_str("\n#### Uncounted Votes\n");
                            report.push_str(&format!("- **Yes**: {}\n", uncounted.yes()));
                            report.push_str(&format!("- **No**: {}\n", uncounted.no()));
                        }
                    },
                    VoteType::Informal => {
                        if let Some(VoteResult::Informal { count }) = vote.result() {
                            report.push_str(&format!("- **Yes**: {}\n", count.yes()));
                            report.push_str(&format!("- **No**: {}\n", count.no()));
                        }
                    }
                }
            } else {
                report.push_str("Vote counts not available for historical votes.\n");
            }
        } else {
            report.push_str("## Voting Information\n\nNo vote was conducted for this proposal.\n\n");
        }
    
        Ok(report)
    }

    pub fn generate_team_snapshots_table(&self, raffle: &Raffle) -> String {
        let mut table = String::from("### Team Snapshots\n\n");
        table.push_str("| Team Name | Status | Revenue | Ballot Range | Ticket Count |\n");
        table.push_str("|-----------|--------|---------|--------------|--------------|\n");

        for snapshot in raffle.team_snapshots() {
            let team_name = snapshot.name();
            let status = format!("{:?}", snapshot.status());
            let revenue = match snapshot.status() {
                TeamStatus::Earner { trailing_monthly_revenue } => format!("{:?}", trailing_monthly_revenue),
                _ => "N/A".to_string(),
            };
            let tickets: Vec<_> = raffle.tickets().iter()
                .filter(|t| t.team_id() == snapshot.id())
                .collect();
            let ballot_range = if !tickets.is_empty() {
                format!("{} - {}", tickets.first().unwrap().index(), tickets.last().unwrap().index())
            } else {
                "N/A".to_string()
            };
            let ticket_count = tickets.len();

            table.push_str(&format!("| {} | {} | {} | {} | {} |\n", 
                team_name, status, revenue, ballot_range, ticket_count));
        }

        table.push_str("\n");
        table
    }

    pub fn generate_raffle_outcome(&self, report: &mut String, raffle: &Raffle, result: &RaffleResult) {
        let counted_earners: Vec<_> = result.counted().iter()
            .filter(|&team_id| raffle.team_snapshots().iter().any(|s| s.id() == *team_id && matches!(s.status(), TeamStatus::Earner { .. })))
            .collect();
        let counted_supporters: Vec<_> = result.counted().iter()
            .filter(|&team_id| raffle.team_snapshots().iter().any(|s| s.id() == *team_id && matches!(s.status(), TeamStatus::Supporter)))
            .collect();
    
        report.push_str(&format!("#### Counted Seats (Total: {})\n\n", result.counted().len()));
        
        report.push_str(&format!("##### Earner Seats ({})\n", counted_earners.len()));
        for team_id in counted_earners {
            if let Some(snapshot) = raffle.team_snapshots().iter().find(|s| s.id() == *team_id) {
                let best_score = raffle.tickets().iter()
                    .filter(|t| t.team_id() == *team_id)
                    .map(|t| t.score())
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap_or(0.0);
                report.push_str(&format!("- {} (Best Score: {:.4})\n", snapshot.name(), best_score));
            }
        }
    
        report.push_str(&format!("\n##### Supporter Seats ({})\n", counted_supporters.len()));
        for team_id in counted_supporters {
            if let Some(snapshot) = raffle.team_snapshots().iter().find(|s| s.id() == *team_id) {
                let best_score = raffle.tickets().iter()
                    .filter(|t| t.team_id() == *team_id)
                    .map(|t| t.score())
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap_or(0.0);
                report.push_str(&format!("- {} (Best Score: {:.4})\n", snapshot.name(), best_score));
            }
        }
    
        report.push_str("\n#### Uncounted Seats\n");
        for team_id in result.uncounted() {
            if let Some(snapshot) = raffle.team_snapshots().iter().find(|s| s.id() == *team_id) {
                let best_score = raffle.tickets().iter()
                    .filter(|t| t.team_id() == *team_id)
                    .map(|t| t.score())
                    .max_by(|a, b| a.partial_cmp(b).unwrap())
                    .unwrap_or(0.0);
                report.push_str(&format!("- {} (Best Score: {:.4})\n", snapshot.name(), best_score));
            }
        }
    }

    pub fn generate_vote_participation_tables(&self, vote: &Vote) -> String {
        let mut tables = String::new();

        match &vote.participation() {
            VoteParticipation::Formal { counted, uncounted } => {
                tables.push_str("#### Counted Votes\n");
                tables.push_str("| Team | Points Credited |\n");
                tables.push_str("|------|------------------|\n");
                for &team_id in counted {
                    if let Some(team) = self.state.current_state().teams().get(&team_id) {
                        tables.push_str(&format!("| {} | {} |\n", team.name(), self.config.counted_vote_points));
                    }
                }

                tables.push_str("\n#### Uncounted Votes\n");
                tables.push_str("| Team | Points Credited |\n");
                tables.push_str("|------|------------------|\n");
                for &team_id in uncounted {
                    if let Some(team) = self.state.current_state().teams().get(&team_id) {
                        tables.push_str(&format!("| {} | {} |\n", team.name(), self.config.uncounted_vote_points));
                    }
                }
            },
            VoteParticipation::Informal(participants) => {
                tables.push_str("#### Participants\n");
                tables.push_str("| Team | Points Credited |\n");
                tables.push_str("|------|------------------|\n");
                for &team_id in participants {
                    if let Some(team) = self.state.current_state().teams().get(&team_id) {
                        tables.push_str(&format!("| {} | 0 |\n", team.name()));
                    }
                }
            },
        }

        tables
    }

    pub fn calculate_days_between(&self, start: NaiveDate, end: NaiveDate) -> i64 {
        (end - start).num_days()
    }

    pub fn get_current_or_specified_epoch(&self, epoch_name: Option<&str>) -> Result<(&Epoch, Uuid), &'static str> {
        match epoch_name {
            Some(name) => {
                let (id, epoch) = self.state.epochs().iter()
                    .find(|(_, e)| e.name() == name)
                    .ok_or("Specified epoch not found")?;
                Ok((epoch, *id))
            },
            None => {
                let current_epoch_id = self.state.current_epoch().ok_or("No active epoch")?;
                let epoch = self.state.epochs().get(&current_epoch_id).ok_or("Current epoch not found")?;
                Ok((epoch, current_epoch_id))
            }
        }
    }

    pub fn generate_point_report(&self, epoch_name: Option<&str>) -> Result<String, &'static str> {
        let (epoch, epoch_id) = self.get_current_or_specified_epoch(epoch_name)?;
        self.generate_point_report_for_epoch(epoch_id)
    }

    pub fn generate_point_report_for_epoch(&self, epoch_id: Uuid) -> Result<String, &'static str> {
        let epoch = self.state.epochs().get(&epoch_id).ok_or("Epoch not found")?;
        let mut report = String::new();

        for (team_id, team) in self.state.current_state().teams() {
            let mut team_report = format!("{}, ", team.name());
            let mut total_points = 0;
            let mut allocations = Vec::new();

            for proposal_id in epoch.associated_proposals() {
                if let Some(proposal) = self.state.get_proposal(&proposal_id) {
                    if let Some(vote) = self.state.votes().values().find(|v| v.proposal_id() == *proposal_id) {
                        let (participation_type, points) = match (vote.vote_type(), vote.participation()) {
                            (VoteType::Formal { counted_points, uncounted_points, .. }, VoteParticipation::Formal { counted, uncounted }) => {
                                if counted.contains(team_id) {
                                    ("Counted", *counted_points)
                                } else if uncounted.contains(team_id) {
                                    ("Uncounted", *uncounted_points)
                                } else {
                                    continue;
                                }
                            },
                            (VoteType::Informal, VoteParticipation::Informal(participants)) => {
                                if participants.contains(team_id) {
                                    ("Informal", 0)
                                } else {
                                    continue;
                                }
                            },
                            _ => continue,
                        };

                        total_points += points;
                        allocations.push(format!("{}: {} voter, {} points", 
                            proposal.title(), participation_type, points));
                    }
                }
            }

            team_report.push_str(&format!("{} points\n", total_points));
            for allocation in allocations {
                team_report.push_str(&format!("{}\n", allocation));
            }
            team_report.push('\n');

            report.push_str(&team_report);
        }

        Ok(report)
    }

    pub fn get_team_points_history(&self, team_id: Uuid) -> Result<Vec<(Uuid, u32)>, &'static str> {
        self.state.epochs().iter()
            .map(|(&epoch_id, _)| {
                self.get_team_points_for_epoch(team_id, epoch_id)
                    .map(|points| (epoch_id, points))
            })
            .collect()
    }

    pub fn get_team_points_for_epoch(&self, team_id: Uuid, epoch_id: Uuid) -> Result<u32, &'static str> {
        let epoch = self.state.epochs().get(&epoch_id).ok_or("Epoch not found")?;
        let mut total_points = 0;

        for proposal_id in epoch.associated_proposals() {
            if let Some(vote) = self.state.votes().values().find(|v| v.proposal_id() == *proposal_id) {
                if let (VoteType::Formal { counted_points, uncounted_points, .. }, VoteParticipation::Formal { counted, uncounted }) = (vote.vote_type(), vote.participation()) {
                    if counted.contains(&team_id) {
                        total_points += counted_points;
                    } else if uncounted.contains(&team_id) {
                        total_points += uncounted_points;
                    }
                }
            }
        }

        Ok(total_points)
    }

    pub fn close_epoch(&mut self, epoch_name: Option<&str>) -> Result<(), Box<dyn Error>> {
        let epoch_id = match epoch_name {
            Some(name) => self.get_epoch_id_by_name(name)
                .ok_or_else(|| format!("Epoch not found: {}", name))?,
            None => self.state.current_epoch()
                .ok_or("No active epoch")?
        };
    
        // Check for actionable proposals
        let actionable_proposals = self.get_proposals_for_epoch(epoch_id)
            .iter()
            .filter(|p| p.is_actionable())
            .count();
    
        if actionable_proposals > 0 {
            return Err(format!("Cannot close epoch: {} actionable proposals remaining", actionable_proposals).into());
        }
    
        let total_points = self.get_total_points_for_epoch(epoch_id);
        let mut team_rewards = HashMap::new();
    
        // Calculate rewards
        {
            let epoch = self.state.get_epoch(&epoch_id)
                .ok_or("Epoch not found")?;

            if epoch.is_closed() {
                return Err("Epoch is already closed".into());
            }

            if let Some(reward) = epoch.reward() {
                if total_points == 0 {
                    return Err("No points earned in this epoch".into());
                }

                for team_id in self.state.current_state().teams().keys() {
                    let team_points = self.calculate_team_points_for_epoch(*team_id, epoch_id);
                    let percentage = team_points as f64 / total_points as f64 * 100.0;
                    let amount = reward.amount() * (percentage / 100.0);

                    match TeamReward::new(percentage, amount) {
                        Ok(team_reward) => {
                            team_rewards.insert(*team_id, team_reward);
                        },
                        Err(e) => return Err(format!("Failed to create team reward: {}", e).into()),
                    }
                }
            }
        }
    
         // Update epoch
        {
            let epoch = self.state.get_epoch_mut(&epoch_id)
                .ok_or("Epoch not found")?;

            epoch.set_status(EpochStatus::Closed);
            for (team_id, team_reward) in team_rewards {
                epoch.set_team_reward(team_id, team_reward.percentage(), team_reward.amount())?;
            }
        }

        // Clear current_epoch if this was the active epoch
        if self.state.current_epoch() == Some(epoch_id) {
            self.state.set_current_epoch(None);
        }

        self.save_state()?;

        Ok(())
    }

    pub fn get_total_points_for_epoch(&self, epoch_id: Uuid) -> u32 {
        self.state.current_state().teams().keys()
            .map(|team_id| self.calculate_team_points_for_epoch(*team_id, epoch_id))
            .sum()
    }

    pub fn calculate_team_points_for_epoch(&self, team_id: Uuid, epoch_id: Uuid) -> u32 {
        let epoch = match self.state.epochs().get(&epoch_id) {
            Some(e) => e,
            None => return 0,
        };

        epoch.associated_proposals().iter()
            .filter_map(|proposal_id| self.state.votes().values().find(|v| v.proposal_id() == *proposal_id))
            .map(|vote| match (vote.vote_type(), vote.participation()) {
                (VoteType::Formal { counted_points, uncounted_points, .. }, VoteParticipation::Formal { counted, uncounted }) => {
                    if counted.contains(&team_id) {
                        *counted_points
                    } else if uncounted.contains(&team_id) {
                        *uncounted_points
                    } else {
                        0
                    }
                },
                _ => 0,
            })
            .sum()
    }

    pub fn generate_end_of_epoch_report(&self, epoch_name: &str) -> Result<(), Box<dyn Error>> {
        let epoch = self.state.epochs().values()
            .find(|e| e.name() == epoch_name)
            .ok_or_else(|| format!("Epoch not found: {}", epoch_name))?;

        if !epoch.is_closed() {
            return Err("Cannot generate report: Epoch is not closed".into());
        }

        let mut report = String::new();

        // Generate epoch summary
        report.push_str(&self.generate_epoch_summary(epoch)?);

        // Generate proposal tables and individual reports
        report.push_str(&self.generate_proposal_tables(epoch)?);

        // Generate team summary
        report.push_str(&self.generate_team_summary(epoch)?);

        // Save the report
        let file_name = format!("{}-epoch_report.md", Utc::now().format("%Y%m%d"));
        let sanitized_epoch_name = FileSystem::sanitize_filename(epoch_name);
        let report_path = PathBuf::from(&self.config.state_file)
            .parent()
            .unwrap()
            .join("reports")
            .join(sanitized_epoch_name)
            .join(file_name);

        fs::create_dir_all(report_path.parent().unwrap())?;
        fs::write(&report_path, report)?;

        println!("End of Epoch Report generated: {:?}", report_path);

        Ok(())
    }

    pub fn generate_epoch_summary(&self, epoch: &Epoch) -> Result<String, Box<dyn Error>> {
        let proposals = self.get_proposals_for_epoch(epoch.id());
        let approved = proposals.iter().filter(|p| matches!(p.resolution(), Some(Resolution::Approved))).count();
        let rejected = proposals.iter().filter(|p| matches!(p.resolution(), Some(Resolution::Rejected))).count();
        let retracted = proposals.iter().filter(|p| matches!(p.resolution(), Some(Resolution::Retracted))).count();

        let summary = format!(
            "# End of Epoch Report: {}\n\n\
            ## Epoch Summary\n\
            - **Period**: {} to {}\n\
            - **Total Proposals**: {}\n\
            - **Approved Proposals**: {}\n\
            - **Rejected Proposals**: {}\n\
            - **Retracted Proposals**: {}\n\
            - **Total Reward**: {}\n\n",
            epoch.name(),
            epoch.start_date().format("%Y-%m-%d"),
            epoch.end_date().format("%Y-%m-%d"),
            proposals.len(),
            approved,
            rejected,
            retracted,
            epoch.reward().map_or("N/A".to_string(), |r| format!("{} {}", r.amount(), r.token())),
        );

        Ok(summary)
    }

    pub fn generate_proposal_tables(&self, epoch: &Epoch) -> Result<String, Box<dyn Error>> {
        let mut tables = String::new();
        let proposals = self.get_proposals_for_epoch(epoch.id());
    
        let statuses = vec![
            ("Approved", Resolution::Approved),
            ("Rejected", Resolution::Rejected),
            ("Retracted", Resolution::Retracted),
        ];
    
        for (status, resolution) in statuses {
            let filtered_proposals: Vec<&Proposal> = proposals.iter()
                .filter(|p| matches!(p.resolution(), Some(r) if r == resolution))
                .map(|p| *p)  // Dereference once to go from &&Proposal to &Proposal
                .collect();
    
            if !filtered_proposals.is_empty() {
                tables.push_str(&format!("### {} Proposals\n", status));
                tables.push_str("| Name | URL | Team | Amounts | Start Date | End Date | Announced | Resolved | Report |\n");
                tables.push_str("|------|-----|------|---------|------------|----------|-----------|----------|---------|\n");
    
                for proposal in &filtered_proposals {
                    // Generate individual proposal report
                    let report_path = self.generate_and_save_proposal_report(proposal.id(), epoch.name())?;
                    let report_link = report_path.file_name().unwrap().to_str().unwrap();
    
                    let team_name = proposal.budget_request_details()
                        .and_then(|d| d.team())
                        .and_then(|id| self.state.current_state().teams().get(&id))
                        .map_or("N/A".to_string(), |t| t.name().to_string());
    
                    let amounts = proposal.budget_request_details()
                        .map(|d| d.request_amounts().iter()
                            .map(|(token, amount)| format!("{} {}", amount, token))
                            .collect::<Vec<_>>()
                            .join(", "))
                        .unwrap_or_else(|| "N/A".to_string());
    
                    tables.push_str(&format!(
                        "| {} | {} | {} | {} | {} | {} | {} | {} | [Report]({}) |\n",
                        proposal.title(),
                        proposal.url().as_deref().unwrap_or("N/A"),
                        team_name,
                        amounts,
                        proposal.budget_request_details().and_then(|d| d.start_date()).map_or("N/A".to_string(), |d| d.format("%Y-%m-%d").to_string()),
                        proposal.budget_request_details().and_then(|d| d.end_date()).map_or("N/A".to_string(), |d| d.format("%Y-%m-%d").to_string()),
                        proposal.announced_at().map_or("N/A".to_string(), |d| d.format("%Y-%m-%d").to_string()),
                        proposal.resolved_at().map_or("N/A".to_string(), |d| d.format("%Y-%m-%d").to_string()),
                        report_link
                    ));
                }
                tables.push_str("\n");
            }
        }
    
        Ok(tables)
    }
    

    pub fn generate_team_summary(&self, epoch: &Epoch) -> Result<String, Box<dyn Error>> {
        let mut summary = String::from("## Team Summary\n");
        summary.push_str("| Team Name | Status | Counted Votes | Uncounted Votes | Total Points | % of Total Points | Reward Amount |\n");
        summary.push_str("|-----------|--------|---------------|-----------------|--------------|-------------------|---------------|\n");

        let total_points: u32 = self.state.current_state().teams().keys()
            .map(|team_id| self.get_team_points_for_epoch(*team_id, epoch.id()).unwrap_or(0))
            .sum();

        for (team_id, team) in self.state.current_state().teams() {
            let team_points = self.get_team_points_for_epoch(*team_id, epoch.id()).unwrap_or(0);
            let percentage = if total_points > 0 {
                (team_points as f64 / total_points as f64) * 100.0
            } else {
                0.0
            };

            let (counted_votes, uncounted_votes) = self.get_team_vote_counts(*team_id, epoch.id());

            let reward_amount = epoch.team_rewards().get(team_id)
                .map(|reward| format!("{} {}", reward.amount(), epoch.reward().as_ref().map_or("".to_string(), |r| r.token().to_string())))
                .unwrap_or_else(|| "N/A".to_string());

            summary.push_str(&format!(
                "| {} | {:?} | {} | {} | {} | {:.2}% | {} |\n",
                team.name(),
                team.status(),
                counted_votes,
                uncounted_votes,
                team_points,
                percentage,
                reward_amount
            ));
        }

        Ok(summary)
    }

    pub fn get_team_vote_counts(&self, team_id: Uuid, epoch_id: Uuid) -> (u32, u32) {
        let mut counted = 0;
        let mut uncounted = 0;

        for vote in self.state.votes().values() {
            if vote.epoch_id() == epoch_id {
                match vote.participation() {
                    VoteParticipation::Formal { counted: c, uncounted: u } => {
                        if c.contains(&team_id) {
                            counted += 1;
                        } else if u.contains(&team_id) {
                            uncounted += 1;
                        }
                    },
                    VoteParticipation::Informal(_) => {}  // Informal votes are not counted here
                }
            }
        }

        (counted, uncounted)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Utc, Duration};
    use std::sync::Arc;
    use tempfile::TempDir;
    use uuid::Uuid;
    use async_trait::async_trait;
    use crate::app_config::TelegramConfig;
    use crate::services::ethereum::MockEthereumService;

    // Helpers

    async fn create_test_budget_system(state_file: &str, initial_state: Option<BudgetSystemState>) -> BudgetSystem {
        let config = AppConfig {
            state_file: state_file.to_string(),
            ipc_path: "/tmp/test_reth.ipc".to_string(),
            future_block_offset: 10,
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
        BudgetSystem::new(config, ethereum_service, initial_state).await.unwrap()
    }

    async fn create_active_epoch(budget_system: &mut BudgetSystem) -> Uuid {
        let start_date = Utc::now();
        let end_date = start_date + Duration::days(30);
        let epoch_id = budget_system.create_epoch("Test Epoch", start_date, end_date).unwrap();
        budget_system.activate_epoch(epoch_id).unwrap();
        epoch_id
    }

    async fn create_proposal_with_raffle(budget_system: &mut BudgetSystem, proposal_name: &str) -> (Uuid, Uuid) {
        let proposal_id = budget_system.add_proposal(
            proposal_name.to_string(),
            None,
            None,
            Some(Utc::now().date_naive()),
            Some(Utc::now().date_naive()),
            None
        ).unwrap();
    
        let config = budget_system.config().clone();
        let (raffle_id, _) = budget_system.prepare_raffle(proposal_name, None, &config).unwrap();
        budget_system.finalize_raffle(
            raffle_id,
            12345,
            12355,
            "mock_randomness".to_string()
        ).await.unwrap();
    
        (proposal_id, raffle_id)
    }
    
    // Tests

    #[tokio::test]
    async fn test_state_management() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("test_state.json").to_str().unwrap().to_string();

        // Test creating a new BudgetSystem
        let mut budget_system = create_test_budget_system(&state_file, None).await;
        
        // Modify state
        let epoch_id = budget_system.create_epoch("Test Epoch", Utc::now(), Utc::now() + Duration::days(30)).unwrap();
        let team_id = budget_system.create_team("Test Team".to_string(), "Representative".to_string(), Some(vec![1000, 2000, 3000])).unwrap();

        // Save state
        budget_system.save_state().unwrap();

        // Test loading existing state
        let loaded_state = FileSystem::try_load_state(&state_file).unwrap();
        let loaded_system = create_test_budget_system(&state_file, Some(loaded_state)).await;

        // Verify loaded state
        assert_eq!(loaded_system.state().epochs().len(), 1);
        assert!(loaded_system.state().epochs().contains_key(&epoch_id));
        assert_eq!(loaded_system.state().current_state().teams().len(), 1);
        assert!(loaded_system.state().current_state().teams().contains_key(&team_id));

        // Test loading from non-existent file (should create new system)
        let non_existent_file = temp_dir.path().join("non_existent.json").to_str().unwrap().to_string();
        let new_system = create_test_budget_system(&non_existent_file, None).await;
        assert!(new_system.state().epochs().is_empty());
        assert!(new_system.state().current_state().teams().is_empty());
    }

    #[tokio::test]
    async fn test_epoch_management() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("test_state.json").to_str().unwrap().to_string();
        let mut budget_system = create_test_budget_system(&state_file, None).await;

        // Test creating a new epoch
        let start_date = Utc::now();
        let end_date = start_date + Duration::days(30);
        let epoch_id = budget_system.create_epoch("Test Epoch", start_date, end_date).unwrap();
        let epoch = budget_system.get_epoch(&epoch_id).unwrap();
        assert_eq!(epoch.name(), "Test Epoch");
        assert_eq!(epoch.start_date(), start_date);
        assert_eq!(epoch.end_date(), end_date);

        // Test activating an epoch
        budget_system.activate_epoch(epoch_id).unwrap();
        assert_eq!(budget_system.state().current_epoch(), Some(epoch_id));

        // Test setting epoch reward
        budget_system.set_epoch_reward("ETH", 100.0).unwrap();
        let updated_epoch = budget_system.get_epoch(&epoch_id).unwrap();
        assert_eq!(updated_epoch.reward().unwrap().token(), "ETH");
        assert_eq!(updated_epoch.reward().unwrap().amount(), 100.0);

        // Test creating overlapping epoch (should fail)
        let overlapping_start = start_date + Duration::days(15);
        let overlapping_end = end_date + Duration::days(15);
        assert!(budget_system.create_epoch("Overlapping Epoch", overlapping_start, overlapping_end).is_err());

        // Test activating an epoch when another is already active (should fail)
        let another_epoch_id = budget_system.create_epoch("Another Epoch", end_date + Duration::days(1), end_date + Duration::days(31)).unwrap();
        assert!(budget_system.activate_epoch(another_epoch_id).is_err());

        // Ensure points are earned before closing an epoch
        let team_id = budget_system.create_team("Test Team".to_string(), "Rep".to_string(), Some(vec![1000])).unwrap();
        let (proposal_id, raffle_id) = create_proposal_with_raffle(&mut budget_system, "Test Proposal").await;
        let vote_id = budget_system.create_formal_vote(proposal_id, raffle_id, None).unwrap();
        budget_system.cast_votes(vote_id, vec![(team_id, VoteChoice::Yes)]).unwrap();
        budget_system.close_vote(vote_id).unwrap();

        // Close the proposal before closing the epoch
        budget_system.close_with_reason(proposal_id, &Resolution::Approved).unwrap();

        budget_system.close_epoch(Some("Test Epoch")).unwrap();
        let closed_epoch = budget_system.get_epoch(&epoch_id).unwrap();
        assert!(closed_epoch.is_closed());
        assert_eq!(budget_system.state().current_epoch(), None);
    }

    #[tokio::test]
    async fn test_team_management() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("test_state.json").to_str().unwrap().to_string();
        let mut budget_system = create_test_budget_system(&state_file, None).await;

        // Test creating a new team
        let team_id = budget_system.create_team(
            "Test Team".to_string(),
            "Representative".to_string(),
            Some(vec![1000, 2000, 3000])
        ).unwrap();
        let team = budget_system.get_team(&team_id).unwrap();
        assert_eq!(team.name(), "Test Team");
        assert_eq!(team.representative(), "Representative");
        assert!(matches!(team.status(), TeamStatus::Earner { .. }));

        // Test updating team status
        budget_system.update_team_status(team_id, &TeamStatus::Supporter).unwrap();
        let updated_team = budget_system.get_team(&team_id).unwrap();
        assert!(matches!(updated_team.status(), TeamStatus::Supporter));

        // Test getting team by name
        let team_id_by_name = budget_system.get_team_id_by_name("Test Team").unwrap();
        assert_eq!(team_id_by_name, team_id);

        // Test removing a team
        budget_system.remove_team(team_id).unwrap();
        assert!(budget_system.get_team(&team_id).is_none());

        // Test creating a team with invalid data (should fail)
        assert!(budget_system.create_team("".to_string(), "Representative".to_string(), None).is_err());
    }

    #[tokio::test]
    async fn test_proposal_management() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("test_state.json").to_str().unwrap().to_string();
        let mut budget_system = create_test_budget_system(&state_file, None).await;

        // Create an active epoch
        let epoch_id = create_active_epoch(&mut budget_system).await;

        // Test adding a new proposal
        let proposal_id = budget_system.add_proposal(
            "Test Proposal".to_string(),
            Some("http://example.com".to_string()),
            Some(BudgetRequestDetails::new(
                None,
                [("ETH".to_string(), 100.0)].iter().cloned().collect(),
                Some(Utc::now().date_naive()),
                Some((Utc::now() + Duration::days(30)).date_naive()),
                None
            ).unwrap()),
            Some(Utc::now().date_naive()),
            Some(Utc::now().date_naive()),
            None
        ).unwrap();

        let proposal = budget_system.get_proposal(&proposal_id).unwrap();
        assert_eq!(proposal.title(), "Test Proposal");

        // Test updating a proposal
        let updates = UpdateProposalDetails {
            title: Some("Updated Proposal".to_string()),
            url: None,
            budget_request_details: None,
            announced_at: None,
            published_at: None,
            resolved_at: None,
        };
        budget_system.update_proposal("Test Proposal", updates).unwrap();
        let updated_proposal = budget_system.get_proposal(&proposal_id).unwrap();
        assert_eq!(updated_proposal.title(), "Updated Proposal");

        // Test closing a proposal
        budget_system.close_with_reason(proposal_id, &Resolution::Approved).unwrap();
        let closed_proposal = budget_system.get_proposal(&proposal_id).unwrap();
        assert!(closed_proposal.is_closed());
        assert_eq!(closed_proposal.resolution(), Some(Resolution::Approved));

        // Test getting proposals for an epoch
        let epoch_proposals = budget_system.get_proposals_for_epoch(epoch_id);
        assert_eq!(epoch_proposals.len(), 1);
        assert_eq!(epoch_proposals[0].id(), proposal_id);

        // Test adding a proposal without an active epoch (should fail)
        budget_system.close_epoch(None).unwrap();
        assert!(budget_system.add_proposal(
            "Failed Proposal".to_string(),
            None,
            None,
            None,
            None,
            None
        ).is_err());
    }

    #[tokio::test]
    async fn test_raffle_management() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("test_state.json").to_str().unwrap().to_string();
        let mut budget_system = create_test_budget_system(&state_file, None).await;

        // Create an active epoch and a proposal
        let epoch_id = create_active_epoch(&mut budget_system).await;
        let proposal_id = budget_system.add_proposal(
            "Test Proposal".to_string(),
            None,
            None,
            None,
            None,
            None
        ).unwrap();

        // Create some teams
        let team_id1 = budget_system.create_team("Team 1".to_string(), "Rep 1".to_string(), Some(vec![1000])).unwrap();
        let team_id2 = budget_system.create_team("Team 2".to_string(), "Rep 2".to_string(), None).unwrap();

        // Test preparing a raffle
        let config = budget_system.config().clone();
        let (raffle_id, tickets) = budget_system.prepare_raffle(
            "Test Proposal",
            None,
            &config
        ).unwrap();
        assert!(!tickets.is_empty());

        // Test finalizing a raffle
        let raffle = budget_system.finalize_raffle(
            raffle_id,
            12345,
            12355,
            "mock_randomness".to_string()
        ).await.unwrap();
        assert!(raffle.result().is_some());

        // Test importing a predefined raffle
        let imported_raffle_id = budget_system.import_predefined_raffle(
            "Test Proposal",
            vec!["Team 1".to_string()],
            vec!["Team 2".to_string()],
            1,
            1
        ).unwrap();
        let imported_raffle = budget_system.get_raffle(&imported_raffle_id).unwrap();
        assert_eq!(imported_raffle.result().unwrap().counted(), &[team_id1]);
        assert_eq!(imported_raffle.result().unwrap().uncounted(), &[team_id2]);

        // Test importing a historical raffle
        let (historical_raffle_id, historical_raffle) = budget_system.import_historical_raffle(
            "Test Proposal",
            12345,
            12355,
            Some(vec!["Team 1".to_string(), "Team 2".to_string()]),
            None,
            Some(2),
            Some(1)
        ).await.unwrap();
        assert_eq!(historical_raffle.config().initiation_block(), 12345);
        assert_eq!(historical_raffle.config().randomness_block(), 12355);
        assert!(historical_raffle.result().is_some());

        // Test raffle exclusions
        let excluded_raffle_id = budget_system.import_predefined_raffle(
            "Test Proposal",
            vec!["Team 1".to_string()],
            vec![],
            1,
            1
        ).unwrap();
        let excluded_raffle = budget_system.get_raffle(&excluded_raffle_id).unwrap();
        assert_eq!(excluded_raffle.result().unwrap().counted(), &[team_id1]);
        assert!(excluded_raffle.result().unwrap().uncounted().is_empty());

        // Test invalid raffle creation (non-existent proposal)
        assert!(budget_system.prepare_raffle(
            "Non-existent Proposal",
            None,
            &config
        ).is_err());

        // Test invalid raffle finalization (non-existent raffle)
        assert!(budget_system.finalize_raffle(
            Uuid::new_v4(),
            12345,
            12355,
            "mock_randomness".to_string()
        ).await.is_err());
    }

    #[tokio::test]
    async fn test_vote_management() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("test_state.json").to_str().unwrap().to_string();
        let mut budget_system = create_test_budget_system(&state_file, None).await;

        create_active_epoch(&mut budget_system).await;
        let proposal_id = budget_system.add_proposal("Test Proposal".to_string(), None, None, None, None, None).unwrap();

        // Create teams
        let team_id1 = budget_system.create_team("Team 1".to_string(), "Rep 1".to_string(), Some(vec![1000])).unwrap();
        let team_id2 = budget_system.create_team("Team 2".to_string(), "Rep 2".to_string(), Some(vec![2000])).unwrap();

        // Prepare and finalize raffle
        let config = budget_system.config().clone();
        let (raffle_id, _) = budget_system.prepare_raffle("Test Proposal", None, &config).unwrap();
        let mock_randomness = "mock_randomness".to_string();
        budget_system.finalize_raffle(raffle_id, 12345, 12355, mock_randomness).await.unwrap();

        // Create and process a formal vote
        let formal_vote_id = budget_system.create_formal_vote(proposal_id, raffle_id, None).unwrap();
        budget_system.cast_votes(formal_vote_id, vec![(team_id1, VoteChoice::Yes), (team_id2, VoteChoice::No)]).unwrap();

        // Test closing a vote
        let vote_result = budget_system.close_vote(formal_vote_id).unwrap();
        let closed_vote = budget_system.get_vote(&formal_vote_id).unwrap();
        assert!(closed_vote.is_closed());
        assert!(matches!(closed_vote.result(), Some(VoteResult::Formal { .. })));

        // Verify vote result
        if let Some(VoteResult::Formal { counted, uncounted, passed }) = closed_vote.result() {
            assert_eq!(counted.yes() + counted.no(), 2);
            assert_eq!(uncounted.yes() + uncounted.no(), 0);
            assert_eq!(*passed, vote_result);
        } else {
            panic!("Expected Formal vote result");
        }

        // Test error case: closing an already closed vote
        assert!(budget_system.close_vote(formal_vote_id).is_err());
    }

    #[tokio::test]
    async fn test_reporting() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("test_state.json").to_str().unwrap().to_string();
        let mut budget_system = create_test_budget_system(&state_file, None).await;
    
        let epoch_id = create_active_epoch(&mut budget_system).await;
        let team_id = budget_system.create_team("Test Team".to_string(), "Rep".to_string(), Some(vec![1000])).unwrap();
        
        // Create proposal and raffle
        let proposal_id = budget_system.add_proposal("Test Proposal".to_string(), None, None, None, None, None).unwrap();
        let config = budget_system.config().clone();
        let (raffle_id, _) = budget_system.prepare_raffle("Test Proposal", None, &config).unwrap();
        
        // Finalize raffle with the team included
        let mock_randomness = "mock_randomness".to_string();
        budget_system.finalize_raffle(raffle_id, 12345, 12355, mock_randomness).await.unwrap();
    
        // Create and process a vote
        let vote_id = budget_system.create_formal_vote(proposal_id, raffle_id, None).unwrap();
        budget_system.cast_votes(vote_id, vec![(team_id, VoteChoice::Yes)]).unwrap();
        budget_system.close_vote(vote_id).unwrap();
    
        // Generate reports
        let team_report = budget_system.print_team_report();
        assert!(team_report.contains("Test Team"));
    
        let epoch_state = budget_system.print_epoch_state().unwrap();
        assert!(epoch_state.contains("Test Proposal"));
    
        let proposal_report = budget_system.generate_proposal_report(proposal_id).unwrap();
        assert!(proposal_report.contains("Test Proposal"));
    
        let point_report = budget_system.generate_point_report(None).unwrap();
        assert!(point_report.contains("Test Team"));
    
        // Close proposal before closing epoch
        budget_system.close_with_reason(proposal_id, &Resolution::Approved).unwrap();
    
        budget_system.close_epoch(None).unwrap();
        budget_system.generate_end_of_epoch_report(&budget_system.get_epoch(&epoch_id).unwrap().name()).unwrap();
    }

    #[tokio::test]
    async fn test_integration() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("test_state.json").to_str().unwrap().to_string();
        let mut budget_system = create_test_budget_system(&state_file, None).await;

        // Create and activate an epoch
        let epoch_id = create_active_epoch(&mut budget_system).await;
        budget_system.set_epoch_reward("ETH", 1000.0).unwrap();

        // Create teams
        let team_id1 = budget_system.create_team("Team 1".to_string(), "Rep 1".to_string(), Some(vec![1000])).unwrap();
        let team_id2 = budget_system.create_team("Team 2".to_string(), "Rep 2".to_string(), Some(vec![2000])).unwrap();
        let team_id3 = budget_system.create_team("Team 3".to_string(), "Rep 3".to_string(), None).unwrap();

        // Create a proposal
        let proposal_id = budget_system.add_proposal(
            "Test Proposal".to_string(),
            Some("http://example.com".to_string()),
            Some(BudgetRequestDetails::new(
                Some(team_id1),
                [("ETH".to_string(), 100.0)].iter().cloned().collect(),
                Some(Utc::now().date_naive()),
                Some((Utc::now() + Duration::days(30)).date_naive()),
                None
            ).unwrap()),
            Some(Utc::now().date_naive()),
            Some(Utc::now().date_naive()),
            None
        ).unwrap();

        // Conduct a raffle
        let config = budget_system.config().clone();
        let (raffle_id, _) = budget_system.prepare_raffle("Test Proposal", None, &config).unwrap();
        budget_system.finalize_raffle(raffle_id, 12345, 12355, "mock_randomness".to_string()).await.unwrap();
        
        // Generate epoch report
        let epoch_state = budget_system.print_epoch_state().unwrap();
        assert!(epoch_state.contains("Test Proposal"));

        // Create and process a vote
        let vote_id = budget_system.create_formal_vote(proposal_id, raffle_id, None).unwrap();
        budget_system.cast_votes(vote_id, vec![
            (team_id1, VoteChoice::Yes),
            (team_id2, VoteChoice::Yes),
            (team_id3, VoteChoice::No)
        ]).unwrap();
        let vote_result = budget_system.close_vote(vote_id).unwrap();
        
        // Verify the actual vote result
        let vote = budget_system.get_vote(&vote_id).unwrap();
        if let Some(VoteResult::Formal { passed, .. }) = vote.result() {
            assert_eq!(*passed, vote_result);
        } else {
            panic!("Expected Formal vote result");
        }

        // Close the proposal
        budget_system.close_with_reason(proposal_id, &Resolution::Approved).unwrap();
        

        // Close the epoch
        budget_system.close_epoch(None).unwrap();

        // Generate other report
        let team_report = budget_system.print_team_report();
        let proposal_report = budget_system.generate_proposal_report(proposal_id).unwrap();
        let point_report = budget_system.generate_point_report(Some("Test Epoch")).unwrap();
        budget_system.generate_end_of_epoch_report(&budget_system.get_epoch(&epoch_id).unwrap().name()).unwrap();

        // Verify the integrations
        assert!(team_report.contains("Team 1") && team_report.contains("Team 2") && team_report.contains("Team 3"));
        assert!(proposal_report.contains("Approved"));
        assert!(point_report.contains("Team 1") && point_report.contains("Team 2") && point_report.contains("Team 3"));

        // Verify the final state
        let closed_epoch = budget_system.get_epoch(&epoch_id).unwrap();
        assert!(closed_epoch.is_closed());
        let closed_proposal = budget_system.get_proposal(&proposal_id).unwrap();
        assert!(closed_proposal.is_closed());
        assert_eq!(closed_proposal.resolution(), Some(Resolution::Approved));
    }

    #[tokio::test]
    async fn test_error_handling_and_edge_cases() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("test_state.json").to_str().unwrap().to_string();
        let mut budget_system = create_test_budget_system(&state_file, None).await;

        // Test handling of non-existent entities
        assert!(budget_system.get_team(&Uuid::new_v4()).is_none());
        assert!(budget_system.get_proposal(&Uuid::new_v4()).is_none());
        assert!(budget_system.get_epoch(&Uuid::new_v4()).is_none());
        assert!(budget_system.get_raffle(&Uuid::new_v4()).is_none());
        assert!(budget_system.get_vote(&Uuid::new_v4()).is_none());

        // Test behavior with empty state
        assert!(budget_system.print_epoch_state().is_err());
        assert!(budget_system.generate_point_report(None).is_err());

        // Test invalid inputs
        assert!(budget_system.create_epoch("", Utc::now(), Utc::now()).is_err());
        assert!(budget_system.create_team("".to_string(), "Rep".to_string(), None).is_err());
        assert!(budget_system.set_epoch_reward("ETH", -100.0).is_err());

        // Test overlapping epochs
        let epoch1_id = budget_system.create_epoch("Epoch 1", Utc::now(), Utc::now() + Duration::days(30)).unwrap();
        assert!(budget_system.create_epoch("Epoch 2", Utc::now() + Duration::days(15), Utc::now() + Duration::days(45)).is_err());

        // Test activating multiple epochs
        budget_system.activate_epoch(epoch1_id).unwrap();
        let epoch2_id = budget_system.create_epoch("Epoch 2", Utc::now() + Duration::days(31), Utc::now() + Duration::days(61)).unwrap();
        assert!(budget_system.activate_epoch(epoch2_id).is_err());

        // Test closing an epoch with open proposals
        let proposal_id = budget_system.add_proposal("Test Proposal".to_string(), None, None, None, None, None).unwrap();
        assert!(budget_system.close_epoch(None).is_err());

        // Test updating a non-existent proposal
        let updates = UpdateProposalDetails {
            title: Some("Updated Title".to_string()),
            url: None,
            budget_request_details: None,
            announced_at: None,
            published_at: None,
            resolved_at: None,
        };
        assert!(budget_system.update_proposal("Non-existent Proposal", updates).is_err());

        // Test creating a raffle for a non-existent proposal
        let config = budget_system.config().clone();
        assert!(budget_system.prepare_raffle("Non-existent Proposal", None, &config).is_err());

        // Test casting votes for a non-existent vote
        assert!(budget_system.cast_votes(Uuid::new_v4(), vec![(Uuid::new_v4(), VoteChoice::Yes)]).is_err());

        // Test closing a non-existent vote
        assert!(budget_system.close_vote(Uuid::new_v4()).is_err());
    }

    #[tokio::test]
    async fn test_ethereum_service_interaction() {
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("test_state.json").to_str().unwrap().to_string();
        
        let mut budget_system = create_test_budget_system(&state_file, None).await;

        // Test successful interactions
        assert_eq!(budget_system.get_current_block().await.unwrap(), 12345);
        assert_eq!(budget_system.get_randomness(12355).await.unwrap(), "mock_randomness_for_block_12355");
        
        let (init_block, rand_block, randomness) = budget_system.get_raffle_randomness().await.unwrap();
        assert_eq!(init_block, 12345);
        assert_eq!(rand_block, 12355);
        assert_eq!(randomness, "mock_randomness");

        // Test raffle creation with Ethereum service interaction
        create_active_epoch(&mut budget_system).await;
        budget_system.add_proposal("Test Proposal".to_string(), None, None, None, None, None).unwrap();
        
        let config = budget_system.config().clone();
        let (raffle_id, _) = budget_system.prepare_raffle("Test Proposal", None, &config).unwrap();
        
        let raffle = budget_system.finalize_raffle(raffle_id, 12345, 12355, "mock_randomness".to_string()).await.unwrap();
        
        assert_eq!(raffle.config().initiation_block(), 12345);
        assert_eq!(raffle.config().randomness_block(), 12355);
        assert_eq!(raffle.config().block_randomness(), "mock_randomness");
    }
}