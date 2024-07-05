use chrono::{DateTime, NaiveDate, Utc};
use ethers::prelude::*;
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fs,
    str,
    sync::Arc,
};
use tokio::{
    self,
    time::{sleep, Duration},
};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum TeamStatus {
    Earner { trailing_monthly_revenue: Vec<u64>},
    Supporter,
    Inactive,
}

#[derive(Clone, Serialize, Deserialize)]
struct Team {
    id: Uuid,
    name: String,
    representative: String,
    status: TeamStatus
}

#[derive(Clone, Serialize, Deserialize)]
struct SystemState {
    teams: HashMap<Uuid, Team>,
    timestamp: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
struct BudgetSystem {
    current_state: SystemState,
    history: Vec<SystemState>,
    proposals: HashMap<Uuid, Proposal>,
    raffles: HashMap<Uuid, Raffle>,
    votes: HashMap<Uuid, Vote>,
    epochs: HashMap<Uuid, Epoch>,
    current_epoch: Option<Uuid>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum RaffleTeamStatus {
    Earner { trailing_monthly_revenue: Vec<u64> },
    Supporter,
    Excluded, // For teams with conflict of interest in a particular Vote
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RaffleTeam {
    id: Uuid,
    name: String,
    status: RaffleTeamStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RaffleTicket {
    team_id: Uuid,
    index: u64,
    score: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RaffleResult {
    counted: Vec<Uuid>,
    uncounted: Vec<Uuid>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Raffle {
    id: Uuid,
    proposal_id: Uuid,
    epoch_id: Uuid,
    tickets: Vec<RaffleTicket>,
    teams: HashMap<Uuid, RaffleTeam>,
    total_counted_seats: usize,
    max_earner_seats: usize,
    block_randomness: String,
    result: Option<RaffleResult>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum PaymentStatus {
    Unpaid,
    Paid
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum ProposalStatus {
    Open,
    Closed,
    Reopened,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum Resolution {
    Approved,
    Rejected,
    Invalid,
    Duplicate,
    Retracted
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct BudgetRequestDetails {
    team: Option<Uuid>,
    request_amount: f64,
    request_token: String,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
    payment_status: Option<PaymentStatus>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Proposal {
    id: Uuid,
    title: String,
    url: Option<String>,
    status: ProposalStatus,
    resolution: Option<Resolution>,
    budget_request_details: Option<BudgetRequestDetails>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
enum VoteType {
    Formal {
        raffle_id: Uuid,
        total_eligible_seats: u32,
        threshold: f64,
    },
    Informal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum VoteStatus {
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
enum VoteChoice {
    Yes,
    No,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
struct VoteCount {
    yes: u32,
    no: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum VoteParticipation {
    Formal {
        counted: Vec<Uuid>,
        uncounted: Vec<Uuid>,
    },
    Informal(Vec<Uuid>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
enum VoteResult {
    Formal {
        counted: VoteCount,
        uncounted: VoteCount,
        passed: bool,
    },
    Informal {
        count: VoteCount,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Vote {
    id: Uuid,
    proposal_id: Uuid,
    epoch_id: Uuid,
    vote_type: VoteType,
    status: VoteStatus,
    participation: VoteParticipation,
    result: Option<VoteResult>,
    opened_at: DateTime<Utc>,
    closed_at: Option<DateTime<Utc>>,
    votes: HashMap<Uuid, VoteChoice> // leave private, temporarily stored
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
enum EpochStatus {
    Planned,
    Active,
    Closed,
}

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
struct Epoch {
    id: Uuid,
    start_date: DateTime<Utc>,
    end_date: DateTime<Utc>,
    status: EpochStatus,
    associated_proposals: Vec<Uuid>,
}

impl Team {
    fn new(name: String, representative: String, trailing_monthly_revenue: Option<Vec<u64>>) -> Result<Self, &'static str> {
        let status = match trailing_monthly_revenue {
            Some(revenue) => {
                if revenue.is_empty() {
                    return Err("Revenue data cannot be empty");
                } else if revenue.len() > 3 {
                    return Err("Revenue data cannot exceed 3 entries");  
                } 

                TeamStatus::Earner { trailing_monthly_revenue: revenue }
            },
            None => TeamStatus::Supporter,
        };

        Ok(Team {
            id: Uuid::new_v4(),
            name,
            representative,
            status
        })
    }

    fn get_revenue_data(&self) -> Option<&Vec<u64>> {
        match &self.status {
            TeamStatus::Earner { trailing_monthly_revenue } => Some(trailing_monthly_revenue),
            _ => None,
        }
    }

    fn update_revenue_data(&mut self, new_revenue: Vec<u64>) -> Result<(), &'static str> {
        if new_revenue.is_empty() {
            return Err("New revenue data cannot be empty");
        } else if new_revenue.len() > 3 {
            return Err("New revenue data cannot exceed 3 entries");
        }

        match &mut self.status {
            TeamStatus::Earner { trailing_monthly_revenue } => {
                // Append new revenue data
                trailing_monthly_revenue.extend(new_revenue);

                // Keep only the last 3 entries
                if trailing_monthly_revenue.len() > 3 {
                    let start = trailing_monthly_revenue.len() - 3;
                    *trailing_monthly_revenue = trailing_monthly_revenue[start..].to_vec();
                }
                Ok(())
            },
            TeamStatus::Supporter => Err("Cannot update revenue for a Supporter team"),
            TeamStatus::Inactive => Err("Cannot update revenue for an Inactive team"),
        }
    }

    fn change_status(&mut self, new_status: TeamStatus) -> Result<(), &'static str> {
        match (&self.status, &new_status) {
            (TeamStatus::Supporter, TeamStatus::Earner { trailing_monthly_revenue }) if trailing_monthly_revenue.is_empty() => {
                return Err("Trailing revenue data must be provided when changing to Earner status");
            },
            _ => {}
        }
        self.status = new_status;
        Ok(())
    }

    fn deactivate(&mut self) -> Result<(), &'static str> {
        if matches!(self.status, TeamStatus::Inactive) {
            return Err("Team is already inactive");
        }
        self.status = TeamStatus::Inactive;
        Ok(())
    }

    fn reactivate(&mut self) -> Result<(), &'static str> {
        if !matches!(self.status, TeamStatus::Inactive) {
            return Err("Team is not inactive");
        }
        self.status = TeamStatus::Supporter;
        Ok(())
    }

}


impl BudgetSystem {
    fn new() -> Self {
        BudgetSystem {
            current_state: SystemState {
                teams: HashMap::new(),
                timestamp: Utc::now(),
            },
            history: Vec::new(),
            proposals: HashMap::new(),
            raffles: HashMap::new(),
            votes: HashMap::new(),
            epochs: HashMap::new(),
            current_epoch: None,
        }

    }

    fn add_team(&mut self, name: String, representative: String, trailing_monthly_revenue: Option<Vec<u64>>) -> Result<Uuid, &'static str> {
        let team = Team::new(name, representative, trailing_monthly_revenue)?;
        let id = team.id;
        self.current_state.teams.insert(id, team);
        self.save_state();
        Ok(id)
    }

    fn remove_team(&mut self, team_id: Uuid) -> Result<(), &'static str> {
        if self.current_state.teams.remove(&team_id).is_some() {
            self.save_state();
            Ok(())
        } else {
            Err("Team not found")
        }
    }

    fn deactivate_team(&mut self, team_id: Uuid) -> Result<(), &'static str> {
        match self.current_state.teams.get_mut(&team_id) {
            Some(team) => {
                team.deactivate()?;
                self.save_state();
                Ok(())
            },
            None => Err("Team not found"),
        }
    }

    fn reactivate_team(&mut self, team_id: Uuid) -> Result<(), &'static str> {
        match self.current_state.teams.get_mut(&team_id) {
            Some(team) => {
                team.reactivate()?;
                self.save_state();
                Ok(())
            },
            None => Err("Team not found"),
        }
    }

    fn update_team_status(&mut self, team_id: Uuid, new_status: TeamStatus) -> Result<(), &'static str> {
        match self.current_state.teams.get_mut(&team_id) {
            Some(team) => {
                team.change_status(new_status)?;
                self.save_state();
                Ok(())
            },
            None => Err("Team not found"),
        }
    }

    fn update_team_representative(&mut self, team_id: Uuid, new_representative: String) -> Result<(), &'static str> {
        match self.current_state.teams.get_mut(&team_id) {
            Some(team) => {
                team.representative = new_representative;
                self.save_state();
                Ok(())
            },
            None => Err("Team not found"),
        }
    }

    fn update_team_revenue(&mut self, team_id: Uuid, new_revenue: Vec<u64>) -> Result<(), &'static str> {
        match self.current_state.teams.get_mut(&team_id) {
            Some(team) => {
                team.update_revenue_data(new_revenue)?;
                self.save_state();
                Ok(())
            },
            None => Err("Team not found"),
        }
    }

    fn save_state(&mut self) {
        self.current_state.timestamp = Utc::now();
        self.history.push(self.current_state.clone());
        let json = serde_json::to_string_pretty(&self).unwrap();
        fs::write("budget_system.json", json).unwrap();
    }

    fn load_from_file() -> Result<Self, Box<dyn std::error::Error>> {
        let json = fs::read_to_string("budget_system.json")?;
        let system: BudgetSystem = serde_json::from_str(&json)?;
        Ok(system)
    }

    fn get_state_at(&self, index: usize) -> Option<&SystemState> {
        self.history.get(index)
    }

    fn conduct_raffle(&mut self, proposal_id: Uuid, block_randomness: String, excluded_teams: &[Uuid]) -> Result<Uuid, &'static str> {
        let current_epoch_id = self.current_epoch.ok_or("No active epoch")?;
        
        if !self.proposals.contains_key(&proposal_id) {
            return Err("Proposal not found");
        }

        // Filter for active teams
        let active_teams: HashMap<Uuid, Team> = self.current_state.teams.iter()
            .filter(|(_, team)| !matches!(team.status, TeamStatus::Inactive))
            .map(|(id, team)| (*id, team.clone()))
            .collect();

        if active_teams.is_empty() {
            return Err("No active teams available for the raffle");
        }
        
        let mut raffle = Raffle::new(
            proposal_id,
            current_epoch_id,
            &active_teams,
            excluded_teams,
            block_randomness
        )?;
        raffle.allocate_tickets()?;
        raffle.generate_scores()?;
        raffle.select_teams();

        let raffle_id = raffle.id;
        self.raffles.insert(raffle_id, raffle);
        self.save_state();

        Ok(raffle_id)
    }
    
    fn conduct_raffle_with_custom_seats(
        &mut self,
        proposal_id: Uuid,
        total_counted_seats: usize,
        max_earner_seats: usize,
        block_randomness: String,
        excluded_teams: &[Uuid]
    ) -> Result<Uuid, &'static str> {
        let current_epoch_id = self.current_epoch.ok_or("No active epoch")?;

        if !self.proposals.contains_key(&proposal_id) {
            return Err("Proposal not found");
        }

        if max_earner_seats > total_counted_seats {
            return Err("Earner seats cannot be greater than the total number of seats");
        }

        // Filter for active teams
        let active_teams: HashMap<Uuid, Team> = self.current_state.teams.iter()
            .filter(|(_, team)| !matches!(team.status, TeamStatus::Inactive))
            .map(|(id, team)| (*id, team.clone()))
            .collect();

        if active_teams.is_empty() {
            return Err("No active teams available for the raffle");
        }

        let mut raffle = Raffle::with_custom_seats(
            proposal_id,
            current_epoch_id,
            &active_teams,
            excluded_teams,
            total_counted_seats,
            max_earner_seats,
            block_randomness
        )?;

        raffle.allocate_tickets()?;
        raffle.generate_scores()?;
        raffle.select_teams();

        let raffle_id = raffle.id;
        self.raffles.insert(raffle_id, raffle);
        self.save_state();

        Ok(raffle_id)
        
    }

    fn conduct_raffle_with_custom_allocation(
        &mut self,
        proposal_id: Uuid,
        custom_allocation: Vec<(Uuid, u64)>,
        excluded_teams: &[Uuid],
        block_randomness: String
    ) -> Result<Uuid, &'static str> {
        let current_epoch_id = self.current_epoch.ok_or("No active epoch")?;

        if !self.proposals.contains_key(&proposal_id) {
            return Err("Proposal not found");
        }

        // Filter for active teams
        let active_teams: HashMap<Uuid, Team> = self.current_state.teams.iter()
        .filter(|(_, team)| !matches!(team.status, TeamStatus::Inactive))
        .map(|(id, team)| (*id, team.clone()))
        .collect();

        if active_teams.is_empty() {
            return Err("No active teams available for the raffle");
        }

        let mut raffle = Raffle::with_custom_allocation(
            proposal_id,
            current_epoch_id,
            &active_teams,
            custom_allocation,
            excluded_teams,
            block_randomness
        )?;
        raffle.generate_scores()?;
        raffle.select_teams();

        let raffle_id = raffle.id;
        self.raffles.insert(raffle_id, raffle);
        self.save_state();

        Ok(raffle_id)
    }

    fn create_raffle_with_custom_order(
        &mut self,
        proposal_id: Uuid,
        team_order: &[Uuid],
        excluded_teams: &[Uuid],
        block_randomness: String
    ) -> Result<Uuid, &'static str> {
        let current_epoch_id = self.current_epoch.ok_or("No active epoch")?;

        if !self.proposals.contains_key(&proposal_id) {
            return Err("Proposal not found");
        }

        // Filter for active teams
        let active_teams: HashMap<Uuid, Team> = self.current_state.teams.iter()
            .filter(|(_, team)| !matches!(team.status, TeamStatus::Inactive))
            .map(|(id, team)| (*id, team.clone()))
            .collect();

        if active_teams.is_empty() {
            return Err("No active teams available for the raffle");
        }

        let mut raffle = Raffle::with_custom_team_order(
            proposal_id,
            current_epoch_id,
            &active_teams,
            team_order,
            excluded_teams,
            block_randomness
        )?;
        raffle.generate_scores()?;
        raffle.select_teams();

        let raffle_id = raffle.id;
        self.raffles.insert(raffle_id, raffle);
        self.save_state();

        Ok(raffle_id)
    }    

    fn add_proposal(&mut self, title: String, url: Option<String>, budget_request_details: Option<BudgetRequestDetails>) -> Result<Uuid, &'static str> {
        let current_epoch_id = self.current_epoch.ok_or("No active epoch")?;

        // Validate dates if present
        if let Some(details) = &budget_request_details {
            if let (Some(start), Some(end)) = (details.start_date, details.end_date) {
                if start > end {
                    return Err("Start date cannot be after end date");
                }
            }
            // Ensure payment_status is None for new proposals
            if details.payment_status.is_some() {
                return Err("New proposals should not have a payment status");
            }
        }
    
        let proposal = Proposal::new(title, url, budget_request_details);
        let proposal_id = proposal.id;
        self.proposals.insert(proposal_id, proposal);

        if let Some(epoch) = self.epochs.get_mut(&current_epoch_id) {
            epoch.associated_proposals.push(proposal_id);
        } else {
            return Err("Current epoch not found");
        }
        self.save_state();
        Ok(proposal_id)
    }

    fn get_proposal(&self, id: Uuid) -> Option<&Proposal> {
        self.proposals.get(&id)
    }

    fn update_proposal_status(&mut self, id: Uuid, new_status: ProposalStatus) -> Result<(), &'static str> {
        if let Some(proposal) = self.proposals.get_mut(&id) {
           proposal.update_status(new_status);
            self.save_state();
            Ok(())
        } else {
            Err("Proposal not found")
        }
    }

    fn approve(&mut self, id: Uuid) -> Result<(), &'static str> {
        if let Some(proposal) = self.proposals.get_mut(&id) {
            if proposal.resolution.is_some() {
                return Err("Cannot approve: Proposal already has a resolution");
            }
            if let Some(details) = &proposal.budget_request_details {
                if matches!(details.payment_status, Some(PaymentStatus::Paid)) {
                    return Err("Cannot approve: Proposal is already paid");
                }
            }
            proposal.set_resolution(Resolution::Approved);
            if proposal.is_budget_request() {
                if let Some(details) = &mut proposal.budget_request_details {
                    details.payment_status = Some(PaymentStatus::Unpaid);
                }
            }
            self.save_state();
            Ok(())
        } else {
            Err("Proposal not found")
        }
    }

    fn reject(&mut self, id: Uuid) -> Result<(), &'static str> {
        if let Some(proposal) = self.proposals.get_mut(&id) {
            if let Some(details) = &proposal.budget_request_details {
                if matches!(details.payment_status, Some(PaymentStatus::Paid)) {
                    return Err("Cannot reject: Proposal is already paid");
                }
            }
            proposal.set_resolution(Resolution::Rejected);
            proposal.update_status(ProposalStatus::Closed);
            self.save_state();
            Ok(())
        } else {
            Err("Proposal not found")
        }
    }

    fn retract_resolution(&mut self, id: Uuid) -> Result<(), &'static str> {
        if let Some(proposal) = self.proposals.get_mut(&id) {
            if proposal.resolution.is_none() {
                return Err("No resolution to retract");
            }
            if let Some(details) = &proposal.budget_request_details {
                if matches!(details.payment_status, Some(PaymentStatus::Paid)) {
                    return Err("Cannot retract resolution: Proposal is already paid");
                }
            }
            proposal.remove_resolution();
            if let Some(details) = &mut proposal.budget_request_details {
                details.payment_status = None;
            }
            self.save_state();
            Ok(())
        } else {
            Err("Proposal not found")
        }
    }

    fn reopen(&mut self, id: Uuid) -> Result<(), &'static str> {
        if let Some(proposal) = self.proposals.get_mut(&id) {
            match proposal.status {
                ProposalStatus::Closed => {
                    proposal.update_status(ProposalStatus::Reopened);
                    proposal.remove_resolution();
                    self.save_state();
                    Ok(())
                }
                ProposalStatus::Open => Err("Cannot reopen: Proposal is already open"),
                ProposalStatus::Reopened => Err("Cannot reopen: Proposal is already reopened"),
            }
        } else {
            Err("Proposal not found")
        }
    }
    
    fn close(&mut self, id: Uuid) -> Result<(), &'static str> {
        if let Some(proposal) = self.proposals.get_mut(&id) {
            match proposal.status {
                ProposalStatus::Open | ProposalStatus::Reopened => {
                    proposal.update_status(ProposalStatus::Closed);
                    self.save_state();
                    Ok(())
                },
                ProposalStatus::Closed => Err("Cannot close: Proposal is already closed"),
            }
        } else {
            Err("Proposal not found")
        }
    }

    fn close_with_reason(&mut self, id: Uuid, resolution: Resolution) -> Result<(), &'static str> {
        if let Some(proposal) = self.proposals.get_mut(&id) {
            if proposal.status == ProposalStatus::Closed {
                return Err("Proposal is already closed");
            }
            if let Some(details) = &proposal.budget_request_details {
                if matches!(details.payment_status, Some(PaymentStatus::Paid)) {
                    return Err("Cannot close: Proposal is already paid");
                }
            }
            proposal.set_resolution(resolution);
            proposal.update_status(ProposalStatus::Closed);
            self.save_state();
            Ok(())
        } else {
            Err("Proposal not found")
        }
    }

    fn mark_proposal_as_paid(&mut self, id: Uuid) -> Result<(), &'static str> {
        if let Some(proposal) = self.proposals.get_mut(&id) {
            let result = proposal.mark_as_paid();
            if result.is_ok() {
                self.save_state();
            }
            result
        } else {
            Err("Proposal not found")
        }
    }

    fn create_formal_vote(&mut self, proposal_id: Uuid, raffle_id: Uuid, threshold: Option<f64>) -> Result<Uuid, &'static str> {
        let current_epoch_id = self.current_epoch.ok_or("No active epoch")?;
        
        let proposal = self.proposals.get(&proposal_id)
            .ok_or("Proposal not found")?;

        if !proposal.is_actionable() {
            return Err("Proposal is not in a votable state");
        }

        let raffle = &self.raffles.get(&raffle_id)
            .ok_or("Raffle not found")?;

        if raffle.result.is_none() {
            return Err("Raffle results have not been generated");
        }

        let vote = Vote::new_formal(
            proposal_id,
            current_epoch_id,
            raffle_id, 
            raffle.total_counted_seats as u32,
            threshold
        );
        let vote_id = vote.id;
        self.votes.insert(vote_id, vote);
        self.save_state();
        Ok(vote_id)
    }

    fn create_informal_vote(&mut self, proposal_id: Uuid) -> Result<Uuid, &'static str> {
        let current_epoch_id = self.current_epoch.ok_or("No active epoch")?;
        
        let proposal = self.proposals.get(&proposal_id)
            .ok_or("Proposal not found")?;

        if !proposal.is_actionable() {
            return Err("Proposal is not in a votable state");
        }

        let vote = Vote::new_informal(proposal_id, current_epoch_id);
        let vote_id = vote.id;
        self.votes.insert(vote_id, vote);
        self.save_state();
        Ok(vote_id)
    }

    fn cast_votes(&mut self, vote_id: Uuid, votes: Vec<(Uuid, VoteChoice)>) -> Result<(), &'static str> {
        let vote = self.votes.get_mut(&vote_id).ok_or("Vote not found")?;

        match &vote.vote_type {
            VoteType::Formal { raffle_id, .. } => {
                let raffle = self.raffles.get(raffle_id).ok_or("Associated raffle not found")?;
                let raffle_result = raffle.result.as_ref().ok_or("Raffle teams have not been selected")?;

                let mut counted_votes = Vec::new();
                let mut uncounted_votes = Vec::new();

                for (team_id, choice) in votes {
                    if raffle_result.counted.contains(&team_id) {
                        counted_votes.push((team_id, choice));
                    } else if raffle_result.uncounted.contains(&team_id) {
                        uncounted_votes.push((team_id, choice));
                    }
                    // Votes from teams not in the raffle result are silently ignored
                }

                vote.cast_counted_votes(&counted_votes)?;
                vote.cast_uncounted_votes(&uncounted_votes)?;
            },
            VoteType::Informal => {
                let eligible_votes: Vec<_> = votes.into_iter()
                    .filter(|(team_id, _)| {
                        self.current_state.teams.get(team_id)
                            .map(|team| !matches!(team.status, TeamStatus::Inactive))
                            .unwrap_or(false)
                    })
                    .collect();

                vote.cast_informal_votes(&eligible_votes)?;
            },
        }

        self.save_state();
        Ok(())
    }

    fn close_vote(&mut self, vote_id: Uuid) -> Result<bool, &'static str> {
        let vote = self.votes.get_mut(&vote_id).ok_or("Vote not found")?;
        
        if vote.status == VoteStatus::Closed {
            return Err("Vote is already closed");
        }

        let (proposal_id, vote_type) = (vote.proposal_id, vote.vote_type.clone());
        let result = vote.close()?;

        // If it's a formal vote and it passed, approve the proposal
        if result && matches!(vote_type, VoteType::Formal { .. }) {
            let proposal = self.proposals.get_mut(&proposal_id).ok_or("Associated proposal not found")?;
            proposal.approve()?;
        }

        self.save_state();
        Ok(result)
    }

    fn create_epoch(&mut self, start_date:DateTime<Utc>, end_date: DateTime<Utc>) -> Result<Uuid, &'static str> {
        let new_epoch = Epoch::new(start_date, end_date)?;

        // Check for overlapping epochs
        for epoch in self.epochs.values() {
            if (start_date < epoch.end_date && end_date > epoch.start_date) ||
            (epoch.start_date < end_date && epoch.end_date > start_date) {
                return Err("New epoch overlaps with an existing epoch");
            }
        }

        let epoch_id = new_epoch.id();
        self.epochs.insert(epoch_id, new_epoch);
        Ok(epoch_id)
    }

    fn activate_epoch(&mut self, epoch_id: Uuid) -> Result<(), &'static str> {
        if self.current_epoch.is_some() {
            return Err("Another epoch is currently active");
        }

        let epoch = self.epochs.get_mut(&epoch_id).ok_or("Epoch not found")?;

        if epoch.status != EpochStatus::Planned {
            return Err("Only planned epochs can be activated");
        }

        epoch.status = EpochStatus::Active;
        self.current_epoch = Some(epoch_id);
        Ok(())
    }

    fn close_current_epoch(&mut self) -> Result<(), &'static str> {
        let epoch_id = self.current_epoch.ok_or("No active epoch");
        let epoch = self.epochs.get_mut(&epoch_id?).unwrap();

        epoch.status = EpochStatus::Closed;
        self.current_epoch = None;
        Ok(())
    }

    fn get_current_epoch(&self) -> Option<&Epoch> {
        self.current_epoch.and_then(|id| self.epochs.get(&id))
    }

    fn list_epochs(&self, status: Option<EpochStatus>) -> Vec<&Epoch> {
        self.epochs.values()
            .filter(|&epoch| status.map_or(true, |s| epoch.status == s))
            .collect()
    }

    pub fn get_proposals_for_epoch(&self, epoch_id: Uuid) -> Vec<&Proposal> {
        if let Some(epoch) = self.epochs.get(&epoch_id) {
            epoch.associated_proposals.iter()
                .filter_map(|&id| self.proposals.get(&id))
                .collect()
        } else {
            vec![]
        }
    }

    fn get_votes_for_epoch(&self, epoch_id: Uuid) -> Vec<&Vote> {
        self.votes.values()
            .filter(|vote| vote.epoch_id == epoch_id)
            .collect()
    }

    fn get_raffles_for_epoch(&self, epoch_id: Uuid) -> Vec<&Raffle> {
        self.raffles.values()
            .filter(|raffle| raffle.epoch_id == epoch_id)
            .collect()
    }

    fn get_epoch_for_vote(&self, vote_id: Uuid) -> Option<&Epoch> {
        self.votes.get(&vote_id).and_then(|vote| self.epochs.get(&vote.epoch_id))
    }

    fn get_epoch_for_raffle(&self, raffle_id: Uuid) -> Option<&Epoch> {
        self.raffles.get(&raffle_id).and_then(|raffle| self.epochs.get(&raffle.epoch_id))
    }

    fn transition_to_next_epoch(&mut self) -> Result<(), &'static str> {
        self.close_current_epoch()?;

        let next_epoch = self.epochs.values()
            .filter(|&epoch| epoch.status == EpochStatus::Planned)
            .min_by_key(|&epoch| epoch.start_date)
            .ok_or("No planned epochs available")?;

        self.activate_epoch(next_epoch.id())
    }

    fn update_epoch_dates(&mut self, epoch_id: Uuid, new_start: DateTime<Utc>, new_end: DateTime<Utc>) -> Result<(), &'static str> {
        // Check for overlaps with other epochs
        for other_epoch in self.epochs.values() {
            if other_epoch.id != epoch_id &&
               ((new_start < other_epoch.end_date && new_end > other_epoch.start_date) ||
                (other_epoch.start_date < new_end && other_epoch.end_date > new_start)) {
                return Err("New dates overlap with an existing epoch");
            }
        }
        
        let epoch = self.epochs.get_mut(&epoch_id).ok_or("Epoch not found")?;

        if epoch.status != EpochStatus::Planned {
            return Err("Can only modify dates of planned epochs");
        }

        if new_start >= new_end {
            return Err("Start date must be before end date");
        }

        epoch.start_date = new_start;
        epoch.end_date = new_end;
        Ok(())
    }

    fn cancel_planned_epoch(&mut self, epoch_id: Uuid) -> Result<(), &'static str> {
        let epoch = self.epochs.get(&epoch_id).ok_or("Epoch not found")?;

        if epoch.status != EpochStatus::Planned {
            return Err("Can only cancel planned epochs");
        }

        self.epochs.remove(&epoch_id);
        Ok(())
    }

}

impl Raffle {
    const DEFAULT_TOTAL_COUNTED_SEATS: usize = 7;
    const DEFAULT_MAX_EARNER_SEATS: usize = 5;

    // Initiates a Raffle with default seat allocations
    fn new(proposal_id: Uuid, epoch_id: Uuid, teams: &HashMap<Uuid, Team>, excluded_teams: &[Uuid], block_randomness: String) -> Result<Self, &'static str> {
        let mut raffle = Self::with_custom_seats(
            proposal_id,
            epoch_id,
            teams,
            excluded_teams,
            Self::DEFAULT_TOTAL_COUNTED_SEATS,
            Self::DEFAULT_MAX_EARNER_SEATS,
            block_randomness
        )?;
        raffle.allocate_tickets()?;
        Ok(raffle)
    }
    
    // Clones the Teams into Raffle Teams and initiates a Raffle.
    // Supports non-default seat allocations.
    fn with_custom_seats(
        proposal_id: Uuid,
        epoch_id: Uuid,
        teams: &HashMap<Uuid, Team>,
        excluded_teams: &[Uuid],
        total_counted_seats: usize,
        max_earner_seats: usize,
        block_randomness: String
    ) -> Result<Self, &'static str> {

        if max_earner_seats > total_counted_seats {
            return Err("Max earner seats cannot exceed total counted seats");
        }

        let raffle_teams = teams.iter()
            .filter(|(_, team)| !matches!(team.status, TeamStatus::Inactive))
            .map(|(id, team)| {
            let status = if excluded_teams.contains(id) {
                RaffleTeamStatus::Excluded
            } else {
                match &team.status {
                    TeamStatus::Earner { trailing_monthly_revenue } => 
                        RaffleTeamStatus::Earner { trailing_monthly_revenue: trailing_monthly_revenue.clone() },
                    TeamStatus::Supporter => RaffleTeamStatus::Supporter,
                    TeamStatus::Inactive => unreachable!(),
                }
            };
            (*id, RaffleTeam { id: *id, name: team.name.clone(), status })
        }).collect();

        Ok(Self {
            id: Uuid::new_v4(),
            proposal_id,
            epoch_id,
            tickets: Vec::new(),
            teams: raffle_teams,
            total_counted_seats,
            max_earner_seats,
            block_randomness,
            result: None,
        })
    }

    fn allocate_tickets(&mut self) -> Result<(), &'static str> {
        self.tickets.clear();
        let team_ids: Vec<Uuid> = self.teams.keys().cloned().collect();
        for team_id in team_ids {
            self.generate_tickets_for_team(team_id)?;
        }
        Ok(())
    }

    fn generate_scores(&mut self) -> Result<(), &'static str> {
        for ticket in &mut self.tickets {
            ticket.score = generate_random_score_from_seed(&self.block_randomness, ticket.index);
        }
        Ok(())
    }

    fn select_teams(&mut self) {
        let mut earner_teams: Vec<_> = self.tickets.iter()
            .filter(|ticket| matches!(self.teams[&ticket.team_id].status, RaffleTeamStatus::Earner { .. }))
            .map(|ticket| (ticket.team_id, ticket.score))
            .collect();
        earner_teams.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        earner_teams.dedup_by(|a, b| a.0 == b.0);

        let mut supporter_teams: Vec<_> = self.tickets.iter()
        .filter(|ticket| matches!(self.teams[&ticket.team_id].status, RaffleTeamStatus::Supporter))
        .map(|ticket| (ticket.team_id, ticket.score))
        .collect();
        supporter_teams.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        supporter_teams.dedup_by(|a, b| a.0 == b.0);

        let mut counted = Vec::new();
        let mut uncounted = Vec::new();

        // Select earner teams for counted seats
        let earner_seats = earner_teams.len().min(self.max_earner_seats);
        counted.extend(earner_teams.iter().take(earner_seats).map(|(id, _)| *id));

        // Fill remaining counted seats with supporter teams
        let supporter_seats = self.total_counted_seats - counted.len();
        counted.extend(supporter_teams.iter().take(supporter_seats).map(|(id, _)| *id));

        // Assign remaining teams to uncounted voters
        uncounted.extend(earner_teams.iter().skip(earner_seats).map(|(id, _)| *id));
        uncounted.extend(supporter_teams.iter().skip(supporter_seats).map(|(id, _)| *id));

        // Add excluded teams to uncounted voters
        uncounted.extend(
            self.teams.iter()
                .filter(|(_, team)| matches!(team.status, RaffleTeamStatus::Excluded))
                .map(|(id, _)| *id)
        );

        self.result = Some(RaffleResult { counted, uncounted });

    }

    fn with_custom_allocation(
        proposal_id: Uuid,
        epoch_id: Uuid,
        teams: &HashMap<Uuid, Team>,
        custom_allocation: Vec<(Uuid, u64)>,
        excluded_teams: &[Uuid],
        block_randomness: String
    ) -> Result<Self, &'static str> {
        let mut raffle = Self::with_custom_seats(
            proposal_id,
            epoch_id,
            teams,
            excluded_teams,
            Self::DEFAULT_TOTAL_COUNTED_SEATS,
            Self::DEFAULT_MAX_EARNER_SEATS,
            block_randomness
        )?;

        raffle.tickets.clear();
        for (team_id, ticket_count) in custom_allocation {
            if !raffle.teams.contains_key(&team_id) {
                return Err("Custom allocation includes a team not present in the provided list of teams")
            }
            // Check if the team is excluded
            if excluded_teams.contains(&team_id) {
                continue; // Skip allocating tickets for excluded teams
            }
            for _ in 0..ticket_count {
                raffle.tickets.push(RaffleTicket::new(team_id, raffle.tickets.len() as u64));
            }
        }

        Ok(raffle)

    }

    fn with_custom_team_order(
        proposal_id: Uuid,
        epoch_id: Uuid,
        teams: &HashMap<Uuid, Team>,
        team_order: &[Uuid],
        excluded_teams: &[Uuid],
        block_randomness: String
    ) -> Result<Self, &'static str> {
        let mut raffle = Self::with_custom_seats(
            proposal_id,
            epoch_id,
            teams,
            excluded_teams,
            Self::DEFAULT_TOTAL_COUNTED_SEATS, 
            Self::DEFAULT_MAX_EARNER_SEATS,
            block_randomness
        )?;

        raffle.tickets.clear();
        for &team_id in team_order {
            raffle.generate_tickets_for_team(team_id);
        }

        Ok(raffle)

    }

    fn generate_tickets_for_team(&mut self, team_id: Uuid) -> Result<(), &'static str> {
        if let Some(team) = self.teams.get(&team_id) {
            let ticket_count = team.calculate_ticket_count()?;
            for _ in 0..ticket_count {
                self.tickets.push(RaffleTicket::new(team_id, self.tickets.len() as u64));
            }
        }
        Ok(())
    }

}

impl RaffleTeam {
    fn calculate_ticket_count(&self) -> Result<u64, &'static str> {
        match &self.status {
            RaffleTeamStatus::Earner { trailing_monthly_revenue } => {
                if trailing_monthly_revenue.len() > 3 { 
                    return Err("Trailing monthly revenue cannot exceed 3 entries");
                }
        
                let sum: u64 = trailing_monthly_revenue.iter().sum();
                let quarterly_average = sum as f64 / 3.0;
                let scaled_average = quarterly_average / 1000.0; // Scale down by 1000 for legacy compatibility
                let ticket_count = scaled_average.sqrt().floor() as u64;
        
                Ok(ticket_count.max(1))
            },
            RaffleTeamStatus::Supporter => Ok(1),
            RaffleTeamStatus::Excluded => Ok(0),
        }
    }
}

impl RaffleTicket {
    fn new(team_id: Uuid, index: u64) -> Self {
        RaffleTicket {
            team_id,
            index,
            score: 0.0,
        }
    }
}

// Takes a seed and an index and deterministically generates 
// a random float in the range of 0 < x < 1
fn generate_random_score_from_seed(randomness: &str, index: u64) -> f64 {
    let combined_seed = format!("{}_{}", randomness, index);
    let mut hasher = Sha256::new();

    hasher.update(combined_seed.as_bytes());
    let result = hasher.finalize();

    // Convert first 8 bytes of the hash to a u64
    let hash_num = u64::from_be_bytes(result[..8].try_into().unwrap());
    let max_num = u64::MAX as f64;
    hash_num as f64 / max_num
}

fn draw_with(block_randomness: &str, ballot_index: u64) -> f64 {
    let combined_seed = format!("{}_{}", block_randomness, ballot_index);
    let mut hasher = Sha256::new();

    hasher.update(combined_seed.as_bytes());
    let result = hasher.finalize();

    // Convert first 8 bytes of the hash to a u64
    let hash_num = u64::from_be_bytes(result[..8].try_into().unwrap());
    let max_num = u64::MAX as f64;
    hash_num as f64 / max_num
}

impl Proposal {
    fn new(title: String, url: Option<String>, budget_request_details: Option<BudgetRequestDetails>) -> Self {
        Proposal {
            id: Uuid::new_v4(),
            title,
            url,
            status: ProposalStatus::Open,
            resolution: None,
            budget_request_details,
        }
    }

    fn update_status(&mut self, new_status: ProposalStatus) {
        self.status = new_status;
    }

    fn set_resolution(&mut self, resolution: Resolution) {
        self.resolution = Some(resolution);
    }

    fn remove_resolution(&mut self) {
        self.resolution = None;
    }

    fn mark_as_paid(&mut self) -> Result<(), &'static str> {
        match (&self.status, &self.resolution, &mut self.budget_request_details) {
            (_, Some(Resolution::Approved), Some(details)) => {
                details.payment_status = Some(PaymentStatus::Paid);
                Ok(())
            }
            (_, Some(Resolution::Approved), None) => Err("Cannot mark as paid: Not a budget request"),
            _ => Err("Cannot mark as paid: Proposal is not approved")
        }
    }

    fn is_budget_request(&self) -> bool {
        self.budget_request_details.is_some()
    }

    fn is_actionable(&self) -> bool {
        matches!(self.status, ProposalStatus::Open | ProposalStatus::Reopened)
    }

    fn approve(&mut self) -> Result<(), &'static str> {
        if self.status != ProposalStatus::Open && self.status != ProposalStatus::Reopened {
            return Err("Proposal is not in a state that can be approved");
        }

        self.status = ProposalStatus::Closed;
        self.resolution = Some(Resolution::Approved);
        Ok(())
    }
    
}

impl Vote {
    const DEFAULT_QUALIFIED_MAJORITY_THRESHOLD:f64 = 0.7;

    fn new_formal(proposal_id: Uuid, epoch_id: Uuid, raffle_id: Uuid, total_eligible_seats: u32, threshold: Option<f64>) -> Self {
        Vote {
            id: Uuid::new_v4(),
            proposal_id,
            epoch_id,
            vote_type: VoteType::Formal {
                raffle_id,
                total_eligible_seats,
                threshold: threshold.unwrap_or(Self::DEFAULT_QUALIFIED_MAJORITY_THRESHOLD),
            },
            status: VoteStatus::Open,
            participation: VoteParticipation::Formal {
                counted: Vec::new(),
                uncounted: Vec::new(),
            },
            result: None,
            opened_at: Utc::now(),
            closed_at: None,
            votes: HashMap::new(),
        }
    }

    fn new_informal(proposal_id: Uuid, epoch_id: Uuid) -> Self {
        Vote {
            id: Uuid::new_v4(),
            proposal_id,
            epoch_id,
            vote_type: VoteType::Informal,
            status: VoteStatus::Open,
            participation: VoteParticipation::Informal(Vec::new()),
            result: None,
            opened_at: Utc::now(),
            closed_at: None,
            votes: HashMap::new(),
        }
    }

    fn cast_counted_votes(&mut self, votes: &[(Uuid, VoteChoice)]) -> Result<(), &'static str> {
        if self.status != VoteStatus::Open {
            return Err("Vote is not open");
        }

        if let VoteType::Formal { .. } = self.vote_type {
            for &(team_id, choice) in votes {
                self.votes.insert(team_id, choice);
                if let VoteParticipation::Formal { counted, .. } = &mut self.participation {
                    if !counted.contains(&team_id) {
                        counted.push(team_id);
                    }
                }
            }
            Ok(())
        } else {
            Err("This is not a formal vote")
        }
    }

    fn cast_uncounted_votes(&mut self, votes: &[(Uuid, VoteChoice)]) -> Result<(), &'static str> {
        if self.status != VoteStatus::Open {
            return Err("Vote is not open");
        }

        if let VoteType::Formal { .. } = self.vote_type {
            for &(team_id, choice) in votes {
                self.votes.insert(team_id, choice);
                if let VoteParticipation::Formal { uncounted, .. } = &mut self.participation {
                    if !uncounted.contains(&team_id) {
                        uncounted.push(team_id);
                    }
                }
            }
            Ok(())
        } else {
            Err("This is not a formal vote")
        }
    }

    fn cast_informal_votes(&mut self, votes: &[(Uuid, VoteChoice)]) -> Result<(), &'static str> {
        if self.status != VoteStatus::Open {
            return Err("Vote is not open");
        }

        if let VoteType::Informal = self.vote_type {
            for &(team_id, choice) in votes {
                self.votes.insert(team_id, choice);
                if let VoteParticipation::Informal(participants) = &mut self.participation {
                    if !participants.contains(&team_id) {
                        participants.push(team_id);
                    }
                }
            }
            Ok(())
        } else {
            Err("This is not an informal vote")
        }
    }

    fn count_votes(&self) -> VoteCount {
        let mut count = VoteCount::default();
        for &choice in self.votes.values() {
            match choice {
                VoteChoice::Yes => count.yes += 1,
                VoteChoice::No => count.no += 1,
            }
        }
        count
    }

    fn count_formal_votes(&self) -> (VoteCount, VoteCount) {
        let mut counted = VoteCount::default();
        let mut uncounted = VoteCount::default();

        if let VoteParticipation::Formal { counted: counted_participants, uncounted: uncounted_participants } = &self.participation {
            for (&team_id, &choice) in &self.votes {
                let target = if counted_participants.contains(&team_id) { &mut counted } else { &mut uncounted };
                match choice {
                    VoteChoice::Yes => target.yes += 1,
                    VoteChoice::No => target.no += 1,
                }
            }
        }

        (counted, uncounted)
    }

    fn close(&mut self) -> Result<bool, &'static str> {
        if self.status == VoteStatus::Closed {
            return Err("Vote is already closed");
        }

        self.status = VoteStatus::Closed;
        self.closed_at = Some(Utc::now());

        let result = match &self.vote_type {
            VoteType::Formal { total_eligible_seats, threshold, .. } => {
                let counted_yes = self.votes.iter()
                    .filter(|(&team_id, &choice)| {
                        if let VoteParticipation::Formal { counted, .. } = &self.participation {
                            counted.contains(&team_id) && choice == VoteChoice::Yes
                        } else {
                            false
                        }
                    })
                    .count() as f64;

                let passed = counted_yes / *total_eligible_seats as f64 >= *threshold;
                
                let counted = VoteCount {
                    yes: counted_yes as u32,
                    no: *total_eligible_seats as u32 - counted_yes as u32,
                };
                
                let uncounted = VoteCount {
                    yes: self.votes.iter()
                        .filter(|(&team_id, &choice)| {
                            if let VoteParticipation::Formal { uncounted, .. } = &self.participation {
                                uncounted.contains(&team_id) && choice == VoteChoice::Yes
                            } else {
                                false
                            }
                        })
                        .count() as u32,
                    no: self.votes.iter()
                        .filter(|(&team_id, &choice)| {
                            if let VoteParticipation::Formal { uncounted, .. } = &self.participation {
                                uncounted.contains(&team_id) && choice == VoteChoice::No
                            } else {
                                false
                            }
                        })
                        .count() as u32,
                };

                self.result = Some(VoteResult::Formal {
                    counted,
                    uncounted,
                    passed,
                });

                passed
            },
            VoteType::Informal => {
                let count = VoteCount {
                    yes: self.votes.values().filter(|&&choice| choice == VoteChoice::Yes).count() as u32,
                    no: self.votes.values().filter(|&&choice| choice == VoteChoice::No).count() as u32,
                };

                self.result = Some(VoteResult::Informal { count });

                false // Informal votes don't automatically pass
            },
        };

        self.votes.clear();
        Ok(result)
    }

    fn get_result(&self) -> Option<&VoteResult> {
        self.result.as_ref()
    }
    
}

impl Epoch {
    fn new(start_date: DateTime<Utc>, end_date: DateTime<Utc>) -> Result<Self, &'static str> {
        if start_date >= end_date {
            return Err("Start date must be before end date")
        }

        Ok(Self {
            id: Uuid::new_v4(),
            start_date,
            end_date,
            status: EpochStatus::Planned,
            associated_proposals: Vec::new(),
        })
    }

    fn id(&self) -> Uuid {
        self.id
    }

    fn start_date(&self) -> DateTime<Utc> {
        self.start_date
    }

    fn end_date(&self) -> DateTime<Utc> {
        self.end_date
    }

    fn status(&self) -> &EpochStatus {
        &self.status
    }

    fn associated_proposals(&self) -> &Vec<Uuid> {
        &self.associated_proposals
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let mut budget_system = BudgetSystem::new();

    // Create and activate an epoch
    let start_date = Utc::now();
    let end_date = start_date + chrono::Duration::days(30);
    let epoch_id = budget_system.create_epoch(start_date, end_date)?;
    budget_system.activate_epoch(epoch_id)?;

    println!("Epoch created and activated successfully.");

    // Add teams
    let team_a = budget_system.add_team("Team A".to_string(), "Alice".to_string(), Some(vec![100000, 120000, 110000]))?;
    let team_b = budget_system.add_team("Team B".to_string(), "Bob".to_string(), Some(vec![90000, 95000, 100000]))?;
    let team_c = budget_system.add_team("Team C".to_string(), "Charlie".to_string(), None)?;
    let team_d = budget_system.add_team("Team D".to_string(), "David".to_string(), Some(vec![150000, 160000, 170000]))?;
    let team_e = budget_system.add_team("Team E".to_string(), "Eve".to_string(), None)?;

    println!("Teams added successfully.");

    // Create a proposal
    let proposal_id = budget_system.add_proposal(
        "Q3 Budget Allocation".to_string(),
        Some("https://example.com/q3-budget".to_string()),
        None
    )?;
    println!("Proposal created with ID: {}", proposal_id);

    // Conduct a raffle
    let raffle_id = budget_system.conduct_raffle(proposal_id, "random_seed_123".to_string(), &[])?;
    println!("Raffle conducted with ID: {}", raffle_id);

    // Create a formal vote
    let vote_id = budget_system.create_formal_vote(proposal_id, raffle_id, Some(0.7))?;
    println!("Formal vote created with ID: {}", vote_id);

    // Cast votes
    let votes = vec![
        (team_a, VoteChoice::Yes),
        (team_b, VoteChoice::Yes),
        (team_c, VoteChoice::No),
        (team_d, VoteChoice::Yes),
        (team_e, VoteChoice::No),
    ];
    budget_system.cast_votes(vote_id, votes)?;
    println!("Votes cast successfully.");

    // Close the vote
    let vote_result = budget_system.close_vote(vote_id)?;
    println!("Vote closed. Result: {}", if vote_result { "Passed" } else { "Failed" });

    // Check proposal status
    let proposal = budget_system.get_proposal(proposal_id).ok_or("Proposal not found")?;
    println!("Proposal status: {:?}", proposal.status);
    println!("Proposal resolution: {:?}", proposal.resolution);

    // Create an informal vote for another proposal
    let informal_proposal_id = budget_system.add_proposal(
        "Team Building Event".to_string(),
        None,
        None
    )?;
    let informal_vote_id = budget_system.create_informal_vote(informal_proposal_id)?;
    println!("Informal vote created with ID: {}", informal_vote_id);

    // Cast informal votes
    let informal_votes = vec![
        (team_a, VoteChoice::Yes),
        (team_b, VoteChoice::Yes),
        (team_c, VoteChoice::Yes),
        (team_d, VoteChoice::No),
        (team_e, VoteChoice::Yes),
    ];
    budget_system.cast_votes(informal_vote_id, informal_votes)?;
    println!("Informal votes cast successfully.");

    // Close the informal vote
    let informal_vote_result = budget_system.close_vote(informal_vote_id)?;
    println!("Informal vote closed. Result: {}", if informal_vote_result { "Passed" } else { "Not automatically passed" });

    // Check informal proposal status
    let informal_proposal = budget_system.get_proposal(informal_proposal_id).ok_or("Informal proposal not found")?;
    println!("Informal proposal status: {:?}", informal_proposal.status);
    println!("Informal proposal resolution: {:?}", informal_proposal.resolution);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, Duration};

    fn setup_system_with_epoch() -> (BudgetSystem, Uuid) {
        let mut system = BudgetSystem::new();
        let start_date = Utc::now();
        let end_date = start_date + Duration::days(30);
        let epoch_id = system.create_epoch(start_date, end_date).unwrap();
        system.activate_epoch(epoch_id).unwrap();
        system.add_team("Team A".to_string(), "Alice".to_string(), Some(vec![100000])).unwrap();
        system.add_team("Team B".to_string(), "Bob".to_string(), Some(vec![90000])).unwrap();
        system.add_team("Team C".to_string(), "Charlie".to_string(), None).unwrap();
        (system, epoch_id)
    }

    #[test]
    fn test_add_team() {
        let mut system = BudgetSystem::new();
        let result = system.add_team("Team A".to_string(), "Alice".to_string(), None);
        assert!(result.is_ok());
        assert_eq!(system.current_state.teams.len(), 1);
    }

    #[test]
    fn test_deactivate_team() {
        let (mut system, _) = setup_system_with_epoch();
        let team_id = *system.current_state.teams.keys().next().unwrap();
        system.deactivate_team(team_id).unwrap();
        assert!(matches!(system.current_state.teams[&team_id].status, TeamStatus::Inactive));
    }

    #[test]
    fn test_reactivate_team() {
        let (mut system, _) = setup_system_with_epoch();
        let team_id = *system.current_state.teams.keys().next().unwrap();
        system.deactivate_team(team_id).unwrap();
        system.reactivate_team(team_id).unwrap();
        assert!(matches!(system.current_state.teams[&team_id].status, TeamStatus::Supporter));
    }

    #[test]
    fn test_conduct_raffle() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        let raffle_id = system.conduct_raffle(proposal_id, "test_randomness".to_string(), &[]).unwrap();
        let raffle = system.raffles.get(&raffle_id).unwrap();
        assert_eq!(raffle.teams.len(), 3);
    }

    #[test]
    fn test_raffle_ignores_inactive_team() {
        let (mut system, _) = setup_system_with_epoch();
        let team_ids: Vec<Uuid> = system.current_state.teams.keys().cloned().collect();
        
        // Add a new team and set it to inactive
        let inactive_team_id = system.add_team("Inactive Team".to_string(), "Inactive".to_string(), None).unwrap();
        system.deactivate_team(inactive_team_id).unwrap();
        
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        let raffle_id = system.conduct_raffle(proposal_id, "test_randomness".to_string(), &[]).unwrap();
        let raffle = system.raffles.get(&raffle_id).unwrap();
        
        // Check that the inactive team is not included in the raffle
        assert!(!raffle.teams.contains_key(&inactive_team_id));
        
        // Check that all other teams are included
        for team_id in team_ids {
            assert!(raffle.teams.contains_key(&team_id));
        }
    }

    #[test]
    fn test_conduct_raffle_with_custom_seats() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        let raffle_id = system.conduct_raffle_with_custom_seats(proposal_id, 9, 6, "test_randomness".to_string(), &[]).unwrap();
        let raffle = system.raffles.get(&raffle_id).unwrap();
        assert_eq!(raffle.teams.len(), 3);
        assert_eq!(raffle.total_counted_seats, 9);
        assert_eq!(raffle.max_earner_seats, 6);
    }

    #[test]
    fn test_raffle_with_custom_seats_ignores_inactive_team() {
        let (mut system, _) = setup_system_with_epoch();
        let team_ids: Vec<Uuid> = system.current_state.teams.keys().cloned().collect();
        
        // Add a new team and set it to inactive
        let inactive_team_id = system.add_team("Inactive Team".to_string(), "Inactive".to_string(), None).unwrap();
        system.deactivate_team(inactive_team_id).unwrap();
        
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        let raffle_id = system.conduct_raffle_with_custom_seats(proposal_id, 9, 6, "test_randomness".to_string(), &[]).unwrap();
        let raffle = system.raffles.get(&raffle_id).unwrap();
        
        // Check that the inactive team is not included in the raffle
        assert!(!raffle.teams.contains_key(&inactive_team_id));
        
        // Check that all other teams are included
        for team_id in team_ids {
            assert!(raffle.teams.contains_key(&team_id));
        }
        
        // Check that the custom seat numbers are respected
        assert_eq!(raffle.total_counted_seats, 9);
        assert_eq!(raffle.max_earner_seats, 6);
    }

    #[test]
    fn test_raffle_ticket_allocation() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        let raffle_id = system.conduct_raffle(proposal_id, "test_randomness".to_string(), &[]).unwrap();
        let raffle = system.raffles.get(&raffle_id).unwrap();
        
        let ticket_counts: HashMap<_, _> = raffle.tickets.iter()
            .fold(HashMap::new(), |mut acc, ticket| {
                *acc.entry(ticket.team_id).or_insert(0) += 1;
                acc
            });

        // Check that Earner teams have more than 1 ticket
        for (team_id, team) in &system.current_state.teams {
            if let TeamStatus::Earner { .. } = team.status {
                assert!(*ticket_counts.get(team_id).unwrap_or(&0) > 1);
            }
        }

        // Check that Supporter teams have exactly 1 ticket
        for (team_id, team) in &system.current_state.teams {
            if let TeamStatus::Supporter = team.status {
                assert_eq!(*ticket_counts.get(team_id).unwrap_or(&0), 1);
            }
        }
    }

    #[test]
    fn test_raffle_score_generation() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        let raffle_id = system.conduct_raffle(proposal_id, "test_randomness".to_string(), &[]).unwrap();
        let raffle = system.raffles.get(&raffle_id).unwrap();
        
        for ticket in &raffle.tickets {
            assert!(ticket.score > 0.0 && ticket.score < 1.0);
        }
    }

    #[test]
    fn test_add_team_with_revenue() {
        let mut system = BudgetSystem::new();
        let team_id = system.add_team("Team A".to_string(), "Alice".to_string(), Some(vec![100000])).unwrap();
        assert_eq!(system.current_state.teams.len(), 1);
        assert!(matches!(system.current_state.teams[&team_id].status, TeamStatus::Earner { .. }));
    }

    #[test]
    fn test_add_team_with_invalid_revenue() {
        let mut system = BudgetSystem::new();
        let result = system.add_team("Team A".to_string(), "Alice".to_string(), Some(vec![]));
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_team() {
        let mut system = BudgetSystem::new();
        let team_id = system.add_team("Team A".to_string(), "Alice".to_string(), None).unwrap();
        let result = system.remove_team(team_id);
        assert!(result.is_ok());
        assert_eq!(system.current_state.teams.len(), 0);
    }

    #[test]
    fn test_update_team_status() {
        let mut system = BudgetSystem::new();
        let team_id = system.add_team("Team A".to_string(), "Alice".to_string(), None).unwrap();
        let result = system.update_team_status(team_id, TeamStatus::Earner { trailing_monthly_revenue: vec![100000] });
        assert!(result.is_ok());
        assert!(matches!(system.current_state.teams[&team_id].status, TeamStatus::Earner { .. }));
    }

    #[test]
    fn test_update_team_revenue() {
        let mut system = BudgetSystem::new();
        let team_id = system.add_team("Team A".to_string(), "Alice".to_string(), Some(vec![100000])).unwrap();
        let result = system.update_team_revenue(team_id, vec![120000]);
        assert!(result.is_ok());
        if let TeamStatus::Earner { trailing_monthly_revenue } = &system.current_state.teams[&team_id].status {
            assert_eq!(trailing_monthly_revenue, &vec![100000, 120000]);
        } else {
            panic!("Team A should be an Earner");
        }
    }

    #[test]
    fn test_raffle_creation() {
        let (system, epoch) = setup_system_with_epoch();
        let teams = system.current_state.teams;
        let raffle = Raffle::new(Uuid::new_v4(), epoch,&teams, &[], "test_randomness".to_string()).unwrap();
        assert_eq!(raffle.teams.len(), 5);
        assert_eq!(raffle.total_counted_seats, Raffle::DEFAULT_TOTAL_COUNTED_SEATS);
        assert_eq!(raffle.max_earner_seats, Raffle::DEFAULT_MAX_EARNER_SEATS);
    }

    #[test]
    fn test_raffle_with_custom_seats() {
        let (system, epoch) = setup_system_with_epoch();
        let teams = system.current_state.teams;
        let raffle = Raffle::with_custom_seats(Uuid::new_v4(), epoch, &teams, &[], 9, 6, "test_randomness".to_string()).unwrap();
        assert_eq!(raffle.total_counted_seats, 9);
        assert_eq!(raffle.max_earner_seats, 6);
    }

    #[test]
    fn test_raffle_with_excluded_teams() {
        let (system, epoch) = setup_system_with_epoch();
        let teams = system.current_state.teams;
        let excluded_teams: Vec<Uuid> = teams.values()
            .filter(|t| t.name == "Team C" || t.name == "Team E")
            .map(|t| t.id)
            .collect();
        let raffle = Raffle::new(Uuid::new_v4(), epoch, &teams, &excluded_teams, "test_randomness".to_string()).unwrap();
        assert_eq!(raffle.teams.len(), 5);
        assert!(raffle.teams.values().any(|t| t.name == "Team C" && matches!(t.status, RaffleTeamStatus::Excluded)));
        assert!(raffle.teams.values().any(|t| t.name == "Team E" && matches!(t.status, RaffleTeamStatus::Excluded)));
    }

    #[test]
    fn test_ticket_allocation() {
        let (system, epoch) = setup_system_with_epoch();
        let teams = system.current_state.teams;
        let mut raffle = Raffle::new(Uuid::new_v4(), epoch, &teams, &[], "test_randomness".to_string()).unwrap();
        raffle.allocate_tickets().unwrap();
        
        // Check if earner teams have more than 1 ticket
        assert!(raffle.tickets.iter().filter(|t| raffle.teams[&t.team_id].name == "Team A").count() > 1);
        assert!(raffle.tickets.iter().filter(|t| raffle.teams[&t.team_id].name == "Team B").count() > 1);
        assert!(raffle.tickets.iter().filter(|t| raffle.teams[&t.team_id].name == "Team D").count() > 1);
        
        // Check if supporter teams have exactly 1 ticket
        assert_eq!(raffle.tickets.iter().filter(|t| raffle.teams[&t.team_id].name == "Team C").count(), 1);
        assert_eq!(raffle.tickets.iter().filter(|t| raffle.teams[&t.team_id].name == "Team E").count(), 1);
    }

    #[test]
    fn test_score_generation() {
        let (system, epoch) = setup_system_with_epoch();
        let teams = system.current_state.teams;
        let mut raffle = Raffle::new(Uuid::new_v4(), epoch, &teams, &[], "test_randomness".to_string()).unwrap();
        raffle.allocate_tickets().unwrap();
        raffle.generate_scores().unwrap();
        
        for ticket in &raffle.tickets {
            assert!(ticket.score > 0.0 && ticket.score < 1.0);
        }
    }

    #[test]
    fn test_team_selection() {
        let (system, epoch) = setup_system_with_epoch();
        let teams = system.current_state.teams;
        let mut raffle = Raffle::new(Uuid::new_v4(), epoch, &teams, &[], "test_randomness".to_string()).unwrap();
        raffle.allocate_tickets().unwrap();
        raffle.generate_scores().unwrap();
        raffle.select_teams();
        
        let result = raffle.result.as_ref().unwrap();
        assert_eq!(result.counted.len() + result.uncounted.len(), teams.len());
        assert_eq!(result.counted.len(), Raffle::DEFAULT_TOTAL_COUNTED_SEATS);
        assert!(result.counted.len() <= Raffle::DEFAULT_MAX_EARNER_SEATS + 2); // Max earners + min 2 supporters
    }

    #[test]
    fn test_raffle_with_fewer_teams_than_seats() {
        let (system, epoch) = setup_system_with_epoch();
        let mut teams = HashMap::new();
        let team_a = Team::new("Team A".to_string(), "Alice".to_string(), Some(vec![100000])).unwrap();
        let team_b = Team::new("Team B".to_string(), "Bob".to_string(), None).unwrap();
        teams.insert(team_a.id, team_a);
        teams.insert(team_b.id, team_b);
        
        let mut raffle = Raffle::new(Uuid::new_v4(), epoch, &teams, &[], "test_randomness".to_string()).unwrap();
        raffle.allocate_tickets().unwrap();
        raffle.generate_scores().unwrap();
        raffle.select_teams();
        
        let result = raffle.result.as_ref().unwrap();
        assert_eq!(result.counted.len() + result.uncounted.len(), teams.len());
        assert_eq!(result.counted.len(), teams.len());
        assert_eq!(result.uncounted.len(), 0);
    }

    #[test]
    fn test_raffle_with_all_excluded_teams() {
        let (system, epoch) = setup_system_with_epoch();
        let teams = system.current_state.teams;
        let excluded_teams: Vec<Uuid> = teams.keys().cloned().collect();
        let mut raffle = Raffle::new(Uuid::new_v4(), epoch, &teams, &excluded_teams, "test_randomness".to_string()).unwrap();
        raffle.allocate_tickets().unwrap();
        raffle.generate_scores().unwrap();
        raffle.select_teams();
        
        let result = raffle.result.as_ref().unwrap();
        assert_eq!(result.counted.len(), 0);
        assert_eq!(result.uncounted.len(), teams.len());
    }

    #[test]
    fn test_raffle_with_custom_allocation() {
        let (system, epoch) = setup_system_with_epoch();
        let teams = system.current_state.teams;
        let custom_allocation: Vec<(Uuid, u64)> = teams.iter().map(|(id, _)| (*id, 2)).collect();
        let raffle = Raffle::with_custom_allocation(Uuid::new_v4(), epoch, &teams, custom_allocation, &[], "test_randomness".to_string()).unwrap();
        
        assert_eq!(raffle.tickets.len(), teams.len() * 2);
        for team_id in teams.keys() {
            assert_eq!(raffle.tickets.iter().filter(|t| t.team_id == *team_id).count(), 2);
        }
    }

    #[test]
    fn test_raffle_with_custom_team_order() {
        let (system, epoch) = setup_system_with_epoch();
        let teams = system.current_state.teams;
        let team_order: Vec<Uuid> = teams.keys().cloned().collect();
        let raffle = Raffle::with_custom_team_order(Uuid::new_v4(), epoch, &teams, &team_order, &[], "test_randomness".to_string()).unwrap();
        
        let mut expected_index = 0;
        for team_id in team_order {
            let team_tickets: Vec<_> = raffle.tickets.iter().filter(|t| t.team_id == team_id).collect();
            assert!(!team_tickets.is_empty());
            assert_eq!(team_tickets[0].index, expected_index);
            expected_index += team_tickets.len() as u64;
        }
    }

    #[test]
    fn test_raffle_with_inactive_teams() {
        let (system, epoch) = setup_system_with_epoch();
        let mut teams = system.current_state.teams;
        let inactive_team_id = *teams.keys().next().unwrap();
        teams.get_mut(&inactive_team_id).unwrap().status = TeamStatus::Inactive;

        let raffle = Raffle::new(Uuid::new_v4(), epoch, &teams, &[], "test_randomness".to_string()).unwrap();
        
        assert_eq!(raffle.teams.len(), teams.len() - 1);
        assert!(!raffle.teams.contains_key(&inactive_team_id));
    }

    #[test]
    fn test_raffle_team_ticket_count_calculation() {
        let (_, epoch) = setup_system_with_epoch();
        let mut teams = HashMap::new();
        let team_a = Team::new("Team A".to_string(), "Alice".to_string(), Some(vec![1_000_000])).unwrap();
        teams.insert(team_a.id, team_a);
    
        let raffle = Raffle::new(Uuid::new_v4(), epoch, &teams, &[], "test_randomness".to_string()).unwrap();
    
        // sqrt(1_000_000 / 1000) = sqrt(1000) ≈ 31.6, which should round down to 31
        assert_eq!(raffle.tickets.len(), 31);
    }

    #[test]
    fn test_raffle_with_excessive_revenue_data() {
        let (_, epoch) = setup_system_with_epoch();
        let mut teams = HashMap::new();
        let team_a = Team::new("Team A".to_string(), "Alice".to_string(), Some(vec![100000, 120000, 110000, 130000])).unwrap();
        teams.insert(team_a.id, team_a);

        let result = Raffle::new(Uuid::new_v4(), epoch, &teams, &[], "test_randomness".to_string());
        assert!(result.is_err());
    }

    #[test]
    fn test_raffle_result_generation() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        let raffle_id = system.conduct_raffle(proposal_id, "test_randomness".to_string(), &[]).unwrap();
        
        let raffle = system.raffles.get(&raffle_id).unwrap();
        assert!(raffle.result.is_some());
        
        if let Some(result) = &raffle.result {
            assert_eq!(result.counted.len() + result.uncounted.len(), system.current_state.teams.len());
        } else {
            panic!("Expected raffle result");
        }
    }

    #[test]
    fn test_raffle_with_custom_allocation_and_selection() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        
        let custom_allocation: Vec<(Uuid, u64)> = system.current_state.teams.keys()
            .map(|&id| (id, 2))
            .collect();
        
        let raffle_id = system.conduct_raffle_with_custom_allocation(
            proposal_id, 
            custom_allocation, 
            &[], 
            "test_randomness".to_string()
        ).unwrap();
        
        let raffle = system.raffles.get(&raffle_id).unwrap();
        assert_eq!(raffle.tickets.len(), system.current_state.teams.len() * 2);
        assert!(raffle.result.is_some());
    }

    #[test]
    fn test_raffle_with_custom_team_order_and_selection() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        
        let team_order: Vec<Uuid> = system.current_state.teams.keys().cloned().collect();
        
        let raffle_id = system.create_raffle_with_custom_order(
            proposal_id,
            &team_order,
            &[],
            "test_randomness".to_string()
        ).unwrap();
        
        let raffle = system.raffles.get(&raffle_id).unwrap();
        assert_eq!(raffle.tickets.len(), system.current_state.teams.len());
        assert!(raffle.result.is_some());
    }
    
    #[test]
    fn test_add_proposal() {
        let (mut system, _) = setup_system_with_epoch();
        let team_id = *system.current_state.teams.keys().next().unwrap();
        let proposal_id = system.add_proposal(
            "Test Proposal".to_string(),
            Some("https://example.com".to_string()),
            Some(BudgetRequestDetails {
                team: Some(team_id),
                request_amount: 50000.0,
                request_token: "USD".to_string(),
                start_date: Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
                end_date: Some(NaiveDate::from_ymd_opt(2024, 12, 31).unwrap()),
                payment_status: None,
            })
        ).unwrap();
        
        let proposal = system.get_proposal(proposal_id).unwrap();
        assert_eq!(proposal.title, "Test Proposal");
        assert_eq!(proposal.status, ProposalStatus::Open);
        assert!(proposal.is_budget_request());
        assert!(proposal.resolution.is_none());
    }

    #[test]
    fn test_approve_proposal() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        
        system.approve(proposal_id).unwrap();
        let proposal = system.get_proposal(proposal_id).unwrap();
        assert_eq!(proposal.resolution, Some(Resolution::Approved));
        assert_eq!(proposal.status, ProposalStatus::Open);
    }

    #[test]
    fn test_reject_proposal() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        
        system.reject(proposal_id).unwrap();
        let proposal = system.get_proposal(proposal_id).unwrap();
        assert_eq!(proposal.resolution, Some(Resolution::Rejected));
        assert_eq!(proposal.status, ProposalStatus::Closed);
    }

    #[test]
    fn test_close_proposal() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        
        system.close(proposal_id).unwrap();
        let proposal = system.get_proposal(proposal_id).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Closed);
    }

    #[test]
    fn test_reopen_proposal() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        
        system.close(proposal_id).unwrap();
        system.reopen(proposal_id).unwrap();
        let proposal = system.get_proposal(proposal_id).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Reopened);
        assert!(proposal.resolution.is_none());
    }

    #[test]
    fn test_retract_resolution() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        
        system.approve(proposal_id).unwrap();
        system.retract_resolution(proposal_id).unwrap();
        let proposal = system.get_proposal(proposal_id).unwrap();
        assert!(proposal.resolution.is_none());
    }

    #[test]
    fn test_mark_proposal_as_paid() {
        let (mut system, _) = setup_system_with_epoch();
        let team_id = *system.current_state.teams.keys().next().unwrap();
        let proposal_id = system.add_proposal(
            "Test Proposal".to_string(),
            None,
            Some(BudgetRequestDetails {
                team: Some(team_id),
                request_amount: 50000.0,
                request_token: "USD".to_string(),
                start_date: None,
                end_date: None,
                payment_status: None,
            })
        ).unwrap();
        
        system.approve(proposal_id).unwrap();
        system.mark_proposal_as_paid(proposal_id).unwrap();
        let proposal = system.get_proposal(proposal_id).unwrap();
        assert_eq!(proposal.budget_request_details.as_ref().unwrap().payment_status, Some(PaymentStatus::Paid));
    }

    #[test]
    fn test_cannot_approve_already_resolved_proposal() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        
        system.approve(proposal_id).unwrap();
        assert!(system.approve(proposal_id).is_err());
    }

    #[test]
    fn test_cannot_reject_paid_proposal() {
        let (mut system, _) = setup_system_with_epoch();
        let team_id = *system.current_state.teams.keys().next().unwrap();
        let proposal_id = system.add_proposal(
            "Test Proposal".to_string(),
            None,
            Some(BudgetRequestDetails {
                team: Some(team_id),
                request_amount: 50000.0,
                request_token: "USD".to_string(),
                start_date: None,
                end_date: None,
                payment_status: None,
            })
        ).unwrap();
        
        system.approve(proposal_id).unwrap();
        system.mark_proposal_as_paid(proposal_id).unwrap();
        assert!(system.reject(proposal_id).is_err());
    }

    #[test]
    fn test_cannot_retract_resolution_on_paid_proposal() {
        let (mut system, _) = setup_system_with_epoch();
        let team_id = *system.current_state.teams.keys().next().unwrap();
        let proposal_id = system.add_proposal(
            "Test Proposal".to_string(),
            None,
            Some(BudgetRequestDetails {
                team: Some(team_id),
                request_amount: 50000.0,
                request_token: "USD".to_string(),
                start_date: None,
                end_date: None,
                payment_status: None,
            })
        ).unwrap();
        
        system.approve(proposal_id).unwrap();
        system.mark_proposal_as_paid(proposal_id).unwrap();
        assert!(system.retract_resolution(proposal_id).is_err());
    }

    #[test]
    fn test_cannot_reopen_open_proposal() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        
        assert!(system.reopen(proposal_id).is_err());
    }

    #[test]
    fn test_full_proposal_lifecycle() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        
        // Open -> Approved -> Closed -> Reopened -> Rejected -> Closed
        system.approve(proposal_id).unwrap();
        system.close(proposal_id).unwrap();
        system.reopen(proposal_id).unwrap();
        system.retract_resolution(proposal_id).unwrap();
        system.reject(proposal_id).unwrap();
        
        let proposal = system.get_proposal(proposal_id).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Closed);
        assert_eq!(proposal.resolution, Some(Resolution::Rejected));
    }

    #[test]
    fn test_close_with_reason() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        
        system.close_with_reason(proposal_id, Resolution::Invalid).unwrap();
        let proposal = system.get_proposal(proposal_id).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Closed);
        assert_eq!(proposal.resolution, Some(Resolution::Invalid));
    }

    #[test]
    fn test_cannot_close_with_reason_already_closed_proposal() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        
        system.close(proposal_id).unwrap();
        assert!(system.close_with_reason(proposal_id, Resolution::Duplicate).is_err());
    }

    #[test]
    fn test_cannot_close_with_reason_paid_proposal() {
        let (mut system, _) = setup_system_with_epoch();
        let team_id = *system.current_state.teams.keys().next().unwrap();
        let proposal_id = system.add_proposal(
            "Test Proposal".to_string(),
            None,
            Some(BudgetRequestDetails {
                team: Some(team_id),
                request_amount: 50000.0,
                request_token: "USD".to_string(),
                start_date: None,
                end_date: None,
                payment_status: None,
            })
        ).unwrap();
        
        system.approve(proposal_id).unwrap();
        system.mark_proposal_as_paid(proposal_id).unwrap();
        assert!(system.close_with_reason(proposal_id, Resolution::Retracted).is_err());
    }

    #[test]
    fn test_reopen_removes_resolution() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        
        system.close_with_reason(proposal_id, Resolution::Invalid).unwrap();
        system.reopen(proposal_id).unwrap();
        let proposal = system.get_proposal(proposal_id).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Reopened);
        assert!(proposal.resolution.is_none());
    }

    #[test]
    fn test_cannot_approve_non_existent_proposal() {
        let (mut system, _) = setup_system_with_epoch();
        let non_existent_id = Uuid::new_v4();
        assert!(system.approve(non_existent_id).is_err());
    }

    #[test]
    fn test_cannot_reject_non_existent_proposal() {
        let (mut system, _) = setup_system_with_epoch();
        let non_existent_id = Uuid::new_v4();
        assert!(system.reject(non_existent_id).is_err());
    }

    #[test]
    fn test_cannot_close_non_existent_proposal() {
        let (mut system, _) = setup_system_with_epoch();
        let non_existent_id = Uuid::new_v4();
        assert!(system.close(non_existent_id).is_err());
    }

    #[test]
    fn test_cannot_reopen_non_existent_proposal() {
        let (mut system, _) = setup_system_with_epoch();
        let non_existent_id = Uuid::new_v4();
        assert!(system.reopen(non_existent_id).is_err());
    }

    #[test]
    fn test_cannot_retract_resolution_non_existent_proposal() {
        let (mut system, _) = setup_system_with_epoch();
        let non_existent_id = Uuid::new_v4();
        assert!(system.retract_resolution(non_existent_id).is_err());
    }

    #[test]
    fn test_cannot_mark_as_paid_non_existent_proposal() {
        let (mut system, _) = setup_system_with_epoch();
        let non_existent_id = Uuid::new_v4();
        assert!(system.mark_proposal_as_paid(non_existent_id).is_err());
    }

    #[test]
    fn test_cannot_close_with_reason_non_existent_proposal() {
        let (mut system, _) = setup_system_with_epoch();
        let non_existent_id = Uuid::new_v4();
        assert!(system.close_with_reason(non_existent_id, Resolution::Invalid).is_err());
    }

    #[test]
    fn test_create_formal_vote() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        let raffle_id = system.conduct_raffle(proposal_id, "test_randomness".to_string(), &[]).unwrap();
        let vote_id = system.create_formal_vote(proposal_id, raffle_id, Some(0.7)).unwrap();
        
        let vote = system.votes.get(&vote_id).unwrap();
        assert!(matches!(vote.vote_type, VoteType::Formal { .. }));
        assert_eq!(vote.status, VoteStatus::Open);
    }

    #[test]
    fn test_create_informal_vote() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        let vote_id = system.create_informal_vote(proposal_id).unwrap();
        
        let vote = system.votes.get(&vote_id).unwrap();
        assert!(matches!(vote.vote_type, VoteType::Informal));
        assert_eq!(vote.status, VoteStatus::Open);
    }

    #[test]
    fn test_cast_formal_votes() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        let raffle_id = system.conduct_raffle(proposal_id, "test_randomness".to_string(), &[]).unwrap();
        let vote_id = system.create_formal_vote(proposal_id, raffle_id, Some(0.7)).unwrap();
        
        let team_ids: Vec<Uuid> = system.current_state.teams.keys().cloned().collect();
        let votes: Vec<(Uuid, VoteChoice)> = team_ids.iter().map(|&id| (id, VoteChoice::Yes)).collect();
        
        system.cast_votes(vote_id, votes).unwrap();
        
        let vote = system.votes.get(&vote_id).unwrap();
        if let VoteParticipation::Formal { counted, uncounted } = &vote.participation {
            assert!(!counted.is_empty() || !uncounted.is_empty());
        } else {
            panic!("Expected formal vote participation");
        }
    }

    #[test]
    fn test_cast_informal_votes() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        let vote_id = system.create_informal_vote(proposal_id).unwrap();
        
        let team_ids: Vec<Uuid> = system.current_state.teams.keys().cloned().collect();
        let votes: Vec<(Uuid, VoteChoice)> = team_ids.iter().map(|&id| (id, VoteChoice::Yes)).collect();
        
        system.cast_votes(vote_id, votes).unwrap();
        
        let vote = system.votes.get(&vote_id).unwrap();
        if let VoteParticipation::Informal(participants) = &vote.participation {
            assert!(!participants.is_empty());
        } else {
            panic!("Expected informal vote participation");
        }
    }

    #[test]
    fn test_close_formal_vote() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        let raffle_id = system.conduct_raffle(proposal_id, "test_randomness".to_string(), &[]).unwrap();
        let vote_id = system.create_formal_vote(proposal_id, raffle_id, Some(0.7)).unwrap();
        
        let team_ids: Vec<Uuid> = system.current_state.teams.keys().cloned().collect();
        let votes: Vec<(Uuid, VoteChoice)> = team_ids.iter().map(|&id| (id, VoteChoice::Yes)).collect();
        system.cast_votes(vote_id, votes).unwrap();
        
        let result = system.close_vote(vote_id).unwrap();
        assert!(result);  // Assuming all votes are "Yes", it should pass
        
        let vote = system.votes.get(&vote_id).unwrap();
        assert_eq!(vote.status, VoteStatus::Closed);
        assert!(vote.result.is_some());
    }

    #[test]
    fn test_close_informal_vote() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        let vote_id = system.create_informal_vote(proposal_id).unwrap();
        
        let team_ids: Vec<Uuid> = system.current_state.teams.keys().cloned().collect();
        let votes: Vec<(Uuid, VoteChoice)> = team_ids.iter().map(|&id| (id, VoteChoice::Yes)).collect();
        system.cast_votes(vote_id, votes).unwrap();
        
        let result = system.close_vote(vote_id).unwrap();
        assert!(!result);  // Informal votes don't automatically pass
        
        let vote = system.votes.get(&vote_id).unwrap();
        assert_eq!(vote.status, VoteStatus::Closed);
        assert!(vote.result.is_some());
    }

    #[test]
    fn test_vote_result_calculation() {
        let (mut system, _) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        let raffle_id = system.conduct_raffle(proposal_id, "test_randomness".to_string(), &[]).unwrap();
        let vote_id = system.create_formal_vote(proposal_id, raffle_id, Some(0.7)).unwrap();
        
        let team_ids: Vec<Uuid> = system.current_state.teams.keys().cloned().collect();
        let votes: Vec<(Uuid, VoteChoice)> = team_ids.iter().enumerate().map(|(i, &id)| {
            if i % 2 == 0 { (id, VoteChoice::Yes) } else { (id, VoteChoice::No) }
        }).collect();
        system.cast_votes(vote_id, votes).unwrap();
        
        system.close_vote(vote_id).unwrap();
        
        let vote = system.votes.get(&vote_id).unwrap();
        if let Some(VoteResult::Formal { counted, uncounted, .. }) = &vote.result {
            assert_eq!(counted.yes + counted.no, Raffle::DEFAULT_TOTAL_COUNTED_SEATS as u32);
            assert_eq!(uncounted.yes + uncounted.no, (team_ids.len() - Raffle::DEFAULT_TOTAL_COUNTED_SEATS) as u32);
        } else {
            panic!("Expected formal vote result");
        }
    }

    #[test]
    fn test_create_and_activate_epoch() {
        let mut system = BudgetSystem::new();
        let start_date = Utc::now();
        let end_date = start_date + Duration::days(30);
        let epoch_id = system.create_epoch(start_date, end_date).unwrap();
        assert!(system.activate_epoch(epoch_id).is_ok());
        assert_eq!(system.get_current_epoch().unwrap().id(), epoch_id);
    }

    #[test]
    fn test_add_proposal_to_epoch() {
        let (mut system, epoch_id) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        let epoch = system.epochs.get(&epoch_id).unwrap();
        assert!(epoch.associated_proposals().contains(&proposal_id));
    }

    #[test]
    fn test_get_proposals_for_epoch() {
        let (mut system, epoch_id) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        let proposals = system.get_proposals_for_epoch(epoch_id);
        assert_eq!(proposals.len(), 1);
        assert_eq!(proposals[0].id, proposal_id);
    }

    #[test]
    fn test_get_votes_for_epoch() {
        let (mut system, epoch_id) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        let raffle_id = system.conduct_raffle(proposal_id, "test_randomness".to_string(), &[]).unwrap();
        let vote_id = system.create_formal_vote(proposal_id, raffle_id, Some(0.7)).unwrap();
        let votes = system.get_votes_for_epoch(epoch_id);
        assert_eq!(votes.len(), 1);
        assert_eq!(votes[0].id, vote_id);
    }

    #[test]
    fn test_get_raffles_for_epoch() {
        let (mut system, epoch_id) = setup_system_with_epoch();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        let raffle_id = system.conduct_raffle(proposal_id, "test_randomness".to_string(), &[]).unwrap();
        let raffles = system.get_raffles_for_epoch(epoch_id);
        assert_eq!(raffles.len(), 1);
        assert_eq!(raffles[0].id, raffle_id);
    }

    #[test]
    fn test_epoch_transition() {
        let mut system = BudgetSystem::new();
        let start_date = Utc::now();
        let end_date = start_date + Duration::days(30);
        let epoch_1_id = system.create_epoch(start_date, end_date).unwrap();
        system.activate_epoch(epoch_1_id).unwrap();

        let start_date_2 = end_date + Duration::seconds(1);
        let end_date_2 = start_date_2 + Duration::days(30);
        let epoch_2_id = system.create_epoch(start_date_2, end_date_2).unwrap();

        system.transition_to_next_epoch().unwrap();
        assert_eq!(system.get_current_epoch().unwrap().id(), epoch_2_id);
        assert_eq!(system.epochs.get(&epoch_1_id).unwrap().status(), &EpochStatus::Closed);
    }


}