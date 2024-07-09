use chrono::{DateTime, NaiveDate, Utc};
use ethers::prelude::*;
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use std::{
    collections::HashMap,
    error::Error,
    fs,
    path::Path,
    str,
    sync::Arc,
};
use tokio::{
    self,
    time::{sleep, Duration},
};
use uuid::Uuid;



// Constants and configuration

// Error types

// Structs and enums

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
    status: TeamStatus,
    points: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct TeamSnapshot {
    id: Uuid,
    name: String,
    representative: String,
    status: TeamStatus,
    points: u32,
    snapshot_time: DateTime<Utc>,
    raffle_status: RaffleParticipationStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum RaffleParticipationStatus {
    Included,
    Excluded,
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

struct RaffleService;

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RaffleConfig {
    proposal_id: Uuid,
    epoch_id: Uuid,
    initiation_block: u64,
    randomness_block: u64,
    block_randomness: String,
    total_counted_seats: usize,
    max_earner_seats: usize,
    excluded_teams: Vec<Uuid>,
    custom_allocation: Option<HashMap<Uuid, u64>>,
    custom_team_order: Option<Vec<Uuid>>,
    is_historical: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RaffleBuilder {
    config: RaffleConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Raffle {
    id: Uuid,
    config: RaffleConfig,
    team_snapshots: HashMap<Uuid, TeamSnapshot>,
    tickets: Vec<RaffleTicket>,
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
    request_amounts: HashMap<String, f64>,
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
    is_historical: bool,
    votes: HashMap<Uuid, VoteChoice> // leave private, temporarily stored
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
enum EpochStatus {
    Planned,
    Active,
    Closed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct EpochReward {
    token: String,
    amount: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
struct TeamReward {
    percentage: f64,
    amount: f64,
}

struct EthereumService {
    client: Arc<Provider<Ipc>>,
    future_block_offset: u64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct Epoch {
    id: Uuid,
    name: String,
    start_date: DateTime<Utc>,
    end_date: DateTime<Utc>,
    status: EpochStatus,
    associated_proposals: Vec<Uuid>,
    reward: Option<EpochReward>,
    team_rewards: HashMap<Uuid, TeamReward>,
}

// Implementations

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
            status,
            points: 0,
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

    fn add_points(&mut self, points: u32) {
        self.points += points;
    }

    fn reset_points(&mut self) {
        self.points = 0;
    }

    fn create_snapshot(&self, raffle_status: RaffleParticipationStatus) -> TeamSnapshot {
        TeamSnapshot {
            id: self.id,
            name: self.name.clone(),
            representative: self.representative.clone(),
            status: self.status.clone(),
            points: self.points,
            snapshot_time: Utc::now(),
            raffle_status,
        }
    }

    fn calculate_ticket_count(&self) -> Result<u64, &'static str> {
        match &self.status {
            TeamStatus::Earner { trailing_monthly_revenue } => {
                let sum: u64 = trailing_monthly_revenue.iter().sum();
                let quarterly_average = sum as f64 / trailing_monthly_revenue.len() as f64;
                let scaled_average = quarterly_average / 1000.0;
                let ticket_count = scaled_average.sqrt().floor() as u64;
                Ok(ticket_count.max(1))
            },
            TeamStatus::Supporter => Ok(1),
            TeamStatus::Inactive => Ok(0),
        }
    }

}

impl RaffleService {
    fn create_raffle(
        config: RaffleConfig,
        teams: &HashMap<Uuid, Team>
    ) -> Result<Raffle, &'static str> {
        Raffle::new(config, teams)
    }
    
    fn conduct_raffle(raffle: &mut Raffle) -> Result<(), &'static str> {
        raffle.generate_scores()?;
        raffle.select_teams();
        Ok(())
    }
}

impl RaffleConfig {
    const DEFAULT_TOTAL_COUNTED_SEATS: usize = 7;
    const DEFAULT_MAX_EARNER_SEATS: usize = 5;
}

impl RaffleBuilder {
    fn new(proposal_id: Uuid, epoch_id: Uuid) -> Self {
        RaffleBuilder {
            config: RaffleConfig {
                proposal_id,
                epoch_id,
                initiation_block: 0,
                randomness_block: 0,
                block_randomness: String::new(),
                total_counted_seats: RaffleConfig::DEFAULT_TOTAL_COUNTED_SEATS,
                max_earner_seats: RaffleConfig::DEFAULT_MAX_EARNER_SEATS,
                excluded_teams: Vec::new(),
                custom_allocation: None,
                custom_team_order: None,
                is_historical: false,
            },
        }
    }

    fn with_seats(mut self, total: usize, max_earner: usize) -> Self {
        self.config.total_counted_seats = total;
        self.config.max_earner_seats = max_earner;
        self
    }

    fn with_randomness(mut self, initiation_block: u64, randomness_block: u64, randomness: String) -> Self {
        self.config.initiation_block = initiation_block;
        self.config.randomness_block = randomness_block;
        self.config.block_randomness = randomness;
        self
    }

    fn with_excluded_teams(mut self, excluded: Vec<Uuid>) -> Self {
        self.config.excluded_teams = excluded;
        self
    }

    fn with_custom_allocation(mut self, allocation: HashMap<Uuid, u64>) -> Self {
        self.config.custom_allocation = Some(allocation);
        self
    }

    fn with_custom_team_order(mut self, order: Vec<Uuid>) -> Self {
        self.config.custom_team_order = Some(order);
        self
    }

    fn historical(mut self) -> Self {
        self.config.is_historical = true;
        self
    }

    fn build(self, teams: &HashMap<Uuid, Team>) -> Result<Raffle, &'static str> {
        if self.config.block_randomness.is_empty() {
            return Err("Block randomness must be provided");
        }

        if self.config.max_earner_seats > self.config.total_counted_seats {
            return Err("Max earner seats cannot exceed total counted seats");
        }

        Raffle::new(self.config, teams)
    }
}

impl Raffle {
    fn new(config: RaffleConfig, teams: &HashMap<Uuid, Team>) -> Result<Self, &'static str> {
        let mut team_snapshots = HashMap::new();
        let mut tickets = Vec::new();
        let mut index = 0;

        // Create team snapshots first
        for (&team_id, team) in teams {
            if team.status != TeamStatus::Inactive {
                let snapshot = TeamSnapshot {
                    id: team.id,
                    name: team.name.clone(),
                    representative: team.representative.clone(),
                    status: team.status.clone(),
                    points: team.points,
                    snapshot_time: Utc::now(),
                    raffle_status: if config.excluded_teams.contains(&team_id) {
                        RaffleParticipationStatus::Excluded
                    } else {
                        RaffleParticipationStatus::Included
                    },
                };
                team_snapshots.insert(team_id, snapshot);
            }
        }

        // Generate tickets based on custom order or all active teams
        let team_order = if let Some(custom_order) = &config.custom_team_order {
            custom_order.clone()
        } else {
            team_snapshots.keys().cloned().collect()
        };

        for team_id in team_order {
            if let Some(snapshot) = team_snapshots.get(&team_id) {
                let ticket_count = match &snapshot.status {
                    TeamStatus::Earner { trailing_monthly_revenue } => {
                        let sum: u64 = trailing_monthly_revenue.iter().sum();
                        let quarterly_average = sum as f64 / trailing_monthly_revenue.len() as f64;
                        let scaled_average = quarterly_average / 1000.0;
                        (scaled_average.sqrt().floor() as u64).max(1)
                    },
                    TeamStatus::Supporter => 1,
                    TeamStatus::Inactive => continue,
                };

                for _ in 0..ticket_count {
                    tickets.push(RaffleTicket {
                        team_id,
                        index,
                        score: 0.0, // Will be updated later for non-excluded teams
                    });
                    index += 1;
                }
            }
        }

        Ok(Raffle {
            id: Uuid::new_v4(),
            config,
            team_snapshots,
            tickets,
            result: None,
        })
    }

    fn allocate_tickets(&mut self) -> Result<(), &'static str> {
        self.tickets.clear();
    
        let ticket_allocations: Vec<(Uuid, u64)> = if let Some(custom_allocation) = &self.config.custom_allocation {
            custom_allocation.iter()
                .filter(|(&team_id, _)| self.team_snapshots.contains_key(&team_id) && 
                    self.team_snapshots[&team_id].raffle_status == RaffleParticipationStatus::Included)
                .map(|(&team_id, &ticket_count)| (team_id, ticket_count))
                .collect()
        } else if let Some(custom_order) = &self.config.custom_team_order {
            custom_order.iter()
                .filter(|&team_id| self.team_snapshots.contains_key(team_id) && 
                    self.team_snapshots[team_id].raffle_status == RaffleParticipationStatus::Included)
                .filter_map(|&team_id| {
                    self.team_snapshots.get(&team_id)
                        .and_then(|snapshot| snapshot.calculate_ticket_count().ok())
                        .map(|count| (team_id, count))
                })
                .collect()
        } else {
            self.team_snapshots.iter()
                .filter(|(_, snapshot)| snapshot.raffle_status == RaffleParticipationStatus::Included)
                .filter_map(|(&team_id, snapshot)| {
                    snapshot.calculate_ticket_count().ok().map(|count| (team_id, count))
                })
                .collect()
        };
    
        for (team_id, ticket_count) in ticket_allocations {
            for _ in 0..ticket_count {
                self.tickets.push(RaffleTicket::new(team_id, self.tickets.len() as u64));
            }
        }
    
        Ok(())
    }

    fn generate_tickets_for_team(&mut self, team_id: Uuid) -> Result<(), &'static str> {
        if let Some(team) = self.team_snapshots.get(&team_id) {
            let ticket_count = team.calculate_ticket_count()?;
            for _ in 0..ticket_count {
                self.tickets.push(RaffleTicket::new(team_id, self.tickets.len() as u64));
            }
        }
        Ok(())
    }

    fn generate_scores(&mut self) -> Result<(), &'static str> {
        for ticket in &mut self.tickets {
            if !self.config.excluded_teams.contains(&ticket.team_id) {
                ticket.score = Self::generate_random_score_from_seed(&self.config.block_randomness, ticket.index);
            }
            // Excluded teams keep their score as 0.0
        }
        Ok(())
    }

    fn select_teams(&mut self) {
        let mut earner_tickets: Vec<_> = self.tickets.iter()
            .filter(|t| !self.config.excluded_teams.contains(&t.team_id))
            .filter(|t| matches!(self.team_snapshots[&t.team_id].status, TeamStatus::Earner { .. }))
            .collect();
        earner_tickets.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        let mut supporter_tickets: Vec<_> = self.tickets.iter()
            .filter(|t| !self.config.excluded_teams.contains(&t.team_id))
            .filter(|t| matches!(self.team_snapshots[&t.team_id].status, TeamStatus::Supporter))
            .collect();
        supporter_tickets.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        let mut counted = Vec::new();
        let mut uncounted = Vec::new();

        // Select earner teams
        for ticket in earner_tickets.iter() {
            if counted.len() < self.config.max_earner_seats && !counted.contains(&ticket.team_id) {
                counted.push(ticket.team_id);
            }
        }

        // Select supporter teams
        let supporter_seats = self.config.total_counted_seats.saturating_sub(counted.len());
        for ticket in supporter_tickets.iter() {
            if counted.len() < self.config.total_counted_seats && !counted.contains(&ticket.team_id) {
                counted.push(ticket.team_id);
            }
        }

        // Add remaining teams to uncounted
        for ticket in self.tickets.iter() {
            if !counted.contains(&ticket.team_id) && !uncounted.contains(&ticket.team_id) {
                uncounted.push(ticket.team_id);
            }
        }

        self.result = Some(RaffleResult { counted, uncounted });
    }

    fn generate_random_score_from_seed(randomness: &str, index: u64) -> f64 {
        let combined_seed = format!("{}_{}", randomness, index);
        let mut hasher = Sha256::new();

        hasher.update(combined_seed.as_bytes());
        let result = hasher.finalize();

        let hash_num = u64::from_be_bytes(result[..8].try_into().unwrap());
        let max_num = u64::MAX as f64;
        hash_num as f64 / max_num
    }
}

impl TeamSnapshot {
    fn calculate_ticket_count(&self) -> Result<u64, &'static str> {
        match self.raffle_status {
            RaffleParticipationStatus::Excluded => Ok(0),
            RaffleParticipationStatus::Included => match &self.status {
                TeamStatus::Earner { trailing_monthly_revenue } => {
                if trailing_monthly_revenue.len() > 3 { 
                    return Err("Trailing monthly revenue cannot exceed 3 entries");
                }
        
                let sum: u64 = trailing_monthly_revenue.iter().sum();
                let quarterly_average = sum as f64 / 3.0;
                let scaled_average = quarterly_average / 1000.0; // Scale down by 1000 for legacy compatibility
                let ticket_count = scaled_average.sqrt().floor() as u64;
        
                Ok(ticket_count.max(1))
                },
                TeamStatus::Supporter => Ok(1),
                TeamStatus::Inactive => Ok(0),
            }
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
    
    fn reject(&mut self) -> Result<(), &'static str> {
        if self.status != ProposalStatus::Open && self.status != ProposalStatus::Reopened {
            return Err("Proposal is not in a state that can be rejected");
        }

        self.status = ProposalStatus::Closed;
        self.resolution = Some(Resolution::Rejected);
        Ok(())
    }
    
}

impl Vote {
    const DEFAULT_QUALIFIED_MAJORITY_THRESHOLD:f64 = 0.7;
    const COUNTED_VOTE_POINTS: u32 = 5;
    const UNCOUNTED_VOTE_POINTS: u32 = 2;

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
            is_historical: false,
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
            is_historical: false,
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

    fn count_informal_votes(&self) -> VoteCount {
        let mut count = VoteCount { yes: 0, no: 0 };

        for &choice in self.votes.values() {
            match choice {
                VoteChoice::Yes => count.yes += 1,
                VoteChoice::No => count.no += 1,
            }
        }
        count
    }

    fn count_formal_votes(&self) -> (VoteCount, VoteCount) {
        let mut counted = VoteCount { yes: 0, no: 0 };
        let mut uncounted = VoteCount { yes: 0, no: 0 };

        if let VoteParticipation::Formal { counted: counted_teams, uncounted: uncounted_teams } = &self.participation {
            for (&team_id, &choice) in &self.votes {
                if counted_teams.contains(&team_id) {
                    match choice {
                        VoteChoice::Yes => counted.yes += 1,
                        VoteChoice::No => counted.no += 1,
                    }
                } else if uncounted_teams.contains(&team_id) {
                    match choice {
                        VoteChoice::Yes => uncounted.yes += 1,
                        VoteChoice::No => uncounted.no += 1,
                    }

                }
            }
        }

        (counted, uncounted)
    }

    fn close(&mut self) -> Result<Option<HashMap<Uuid, u32>>, &'static str> {
        if self.status == VoteStatus::Closed {
            return Err("Vote is already closed");
        }

        self.status = VoteStatus::Closed;
        self.closed_at = Some(Utc::now());

        match &self.vote_type {
            VoteType::Formal { total_eligible_seats, threshold, .. } => {
                let (counted_result, uncounted_result) = self.count_formal_votes();
                let passed = (counted_result.yes as f64 / *total_eligible_seats as f64) >= *threshold;

                self.result = Some(VoteResult::Formal { 
                    counted: counted_result, 
                    uncounted: uncounted_result,
                    passed,
                });

                let team_points = self.calculate_formal_vote_points();
                self.votes.clear();
                Ok(Some(team_points))
            },
            VoteType::Informal => {
                let count = self.count_informal_votes();
                self.result = Some(VoteResult::Informal { count });
                self.votes.clear();
                Ok(None)
            },
        }
    }

    fn get_result(&self) -> Option<&VoteResult> {
        self.result.as_ref()
    }

    fn add_points_for_vote(&self, team_id: &Uuid) -> u32 {
        match &self.vote_type {
            VoteType::Formal { .. } => {
                if let VoteParticipation:: Formal { counted, uncounted } = &self.participation {
                    if counted.contains(team_id) {
                        Self::COUNTED_VOTE_POINTS
                    } else if uncounted.contains(team_id) {
                        Self::UNCOUNTED_VOTE_POINTS
                    } else { 0 }
                } else { 0 }
            },
            VoteType::Informal => 0
        }
    }

    fn calculate_formal_vote_points(&self) -> HashMap<Uuid, u32> {
        let mut team_points = HashMap::new();

        if let VoteParticipation::Formal { counted, uncounted } = &self.participation {
            for &team_id in counted {
                if self.votes.contains_key(&team_id) {
                    team_points.insert(team_id, Self::COUNTED_VOTE_POINTS);
                }
            }
            for &team_id in uncounted {
                if self.votes.contains_key(&team_id) {
                    team_points.insert(team_id, Self::UNCOUNTED_VOTE_POINTS);
                }
            }
        }
        team_points
    }

    fn retract_vote(&mut self, team_id: &Uuid) -> Result<(), &'static str> {
        if self.status == VoteStatus::Closed {
            return Err("Cannot retract vote: Vote is closed");
        }

        self.votes.remove(team_id);

        match &mut self.participation {
            VoteParticipation::Formal { counted, uncounted } => {
                counted.retain(|&id| id != *team_id);
                uncounted.retain(|&id| id != *team_id);
            },
            VoteParticipation::Informal(participants) => {
                participants.retain(|&id| id != *team_id);
            },
        }

        Ok(())
    }

    fn calculate_points(&self) -> HashMap<Uuid, u32> {
        let mut points = HashMap::new();
        if let VoteParticipation::Formal { counted, uncounted } = &self.participation {
            for &team_id in counted {
                points.insert(team_id, Self::COUNTED_VOTE_POINTS);
            }
            for &team_id in uncounted {
                points.insert(team_id, Self::UNCOUNTED_VOTE_POINTS);
            }
        }
        points
    }
    
}

impl VoteParticipation {
    fn counted_count(&self) -> u32 {
        match self {
            VoteParticipation::Formal { counted, .. } => counted.len() as u32,
            _ => 0,
        }
    }

    fn uncounted_count(&self) -> u32 {
        match self {
            VoteParticipation::Formal { uncounted, .. } => uncounted.len() as u32,
            _ => 0,
        }
    }
}

impl Epoch {
    fn new(name: String, start_date: DateTime<Utc>, end_date: DateTime<Utc>) -> Result<Self, &'static str> {
        if start_date >= end_date {
            return Err("Start date must be before end date")
        }

        Ok(Self {
            id: Uuid::new_v4(),
            name,
            start_date,
            end_date,
            status: EpochStatus::Planned,
            associated_proposals: Vec::new(),
            reward: None,
            team_rewards: HashMap::new(),
        })
    }

    fn name(&self) -> &str {
        &self.name
    }

    fn set_name(&mut self, new_name: String) -> Result<(), &'static str> {
        if new_name.trim().is_empty() {
            return Err("Epoch name cannot be empty");
        }
        self.name = new_name;
        Ok(())
    }

    fn set_reward(&mut self, token: String, amount: f64) -> Result<(), &'static str> {
        self.reward = Some(EpochReward { token, amount });
        Ok(())
    }

    fn calculate_rewards(&self, teams: &HashMap<Uuid, Team>) -> Result<HashMap<Uuid, TeamReward>, &'static str> {
        let reward = self.reward.as_ref().ok_or("No reward set for this epoch")?;
        
        let total_points: u32 = teams.values().map(|team| team.points).sum();
        if total_points == 0 {
            return Err("No points earned in this epoch");
        }

        let mut rewards = HashMap::new();
        for (team_id, team) in teams {
            if team.points > 0 {
                let percentage = team.points as f64 / total_points as f64;
                let amount = percentage * reward.amount;
                rewards.insert(*team_id, TeamReward { percentage, amount });
            }
        }

        Ok(rewards)
    }

    fn close_current_epoch(&mut self) -> Result<(), &'static str> {
        if self.status != EpochStatus::Active {
            return Err("Only active epochs can be closed");
        }
        self.status = EpochStatus::Closed;
        Ok(())
    }

    fn close_with_rewards(&mut self, teams: &HashMap<Uuid, Team>) -> Result<(), &'static str> {
        if self.status != EpochStatus::Active {
            return Err("Only active epochs can be closed");
        }

        self.team_rewards = self.calculate_rewards(teams)?;
        self.status = EpochStatus::Closed;
        Ok(())
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

impl BudgetRequestDetails {
    fn add_token_amount(&mut self, token: String, amount: f64) -> Result<(), &'static str> {
        if amount <= 0.0 {
            return Err("Amount must be positive");
        }
        self.request_amounts.insert(token, amount);
        Ok(())
    }

    fn remove_token(&mut self, token: &str) -> Option<f64> {
        self.request_amounts.remove(token)
    }

    fn update_token_amount(&mut self, token: &str, amount: f64) -> Result<(), &'static str> {
        if amount <= 0.0 {
            return Err("Amount must be positive");
        }
        if let Some(existing_amount) = self.request_amounts.get_mut(token) {
            *existing_amount = amount;
            Ok(())
        } else {
            Err("Token not found in request")
        }
    }

    fn total_value_in(&self, target_token: &str, exchange_rates: &HashMap<String, f64>) -> Result<f64, &'static str> {
        let mut total = 0.0;
        for (token, &amount) in &self.request_amounts {
            if token == target_token {
                total += amount;
            } else if let Some(&rate) = exchange_rates.get(token) {
                total += amount * rate;
            } else {
                return Err("Exchange rate not available for token");
            }
        }
        Ok(total)
    }
}

impl EthereumService {
    async fn new(ipc_path: &str, future_block_offset: u64) -> Result<Self, Box<dyn std::error::Error>> {
        let provider = Provider::connect_ipc(ipc_path).await?;
        Ok(Self {
            client: Arc::new(provider),
            future_block_offset,
        })
    }

    async fn get_current_block(&self) -> Result<u64, Box<dyn std::error::Error>> {
        Ok(self.client.get_block_number().await?.as_u64())
    }

    async fn get_randomness(&self, block_number: u64) -> Result<String, Box<dyn std::error::Error>> {
        let block = self.client.get_block(block_number).await?
            .ok_or("Block not found")?;
        block.mix_hash
            .ok_or_else(|| "Randomness not found".into())
            .map(|hash| format!("0x{:x}", hash))
    }

    async fn get_raffle_randomness(&self) -> Result<(u64, u64, String), Box<dyn std::error::Error>> {
        let initiation_block = self.get_current_block().await?;
        let randomness_block = initiation_block + self.future_block_offset;

        // Wait for the randomness block
        while self.get_current_block().await? < randomness_block {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        let randomness = self.get_randomness(randomness_block).await?;

        Ok((initiation_block, randomness_block, randomness))
    }
}

// Main BudgetSystem struct and its methods

#[derive(Clone, Serialize, Deserialize)]
struct SystemState {
    teams: HashMap<Uuid, Team>,
    timestamp: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
struct BudgetSystemState {
    current_state: SystemState,
    history: Vec<SystemState>,
    proposals: HashMap<Uuid, Proposal>,
    raffles: HashMap<Uuid, Raffle>,
    votes: HashMap<Uuid, Vote>,
    epochs: HashMap<Uuid, Epoch>,
    current_epoch: Option<Uuid>,
}

struct BudgetSystem {
    state: BudgetSystemState,
    ethereum_service: Arc<EthereumService>,
}

impl BudgetSystem {
    async fn new(ipc_path: &str, future_block_offset: u64) -> Result<Self, Box<dyn std::error::Error>> {
        let ethereum_service = Arc::new(EthereumService::new(ipc_path, future_block_offset).await?);
        
        Ok(Self {
            state: BudgetSystemState {
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
            },
            ethereum_service,
        })
    }

    fn add_team(&mut self, name: String, representative: String, trailing_monthly_revenue: Option<Vec<u64>>) -> Result<Uuid, &'static str> {
        let team = Team::new(name, representative, trailing_monthly_revenue)?;
        let id = team.id;
        self.state.current_state.teams.insert(id, team);
        self.save_state();
        Ok(id)
    }

    fn remove_team(&mut self, team_id: Uuid) -> Result<(), &'static str> {
        if self.state.current_state.teams.remove(&team_id).is_some() {
            self.save_state();
            Ok(())
        } else {
            Err("Team not found")
        }
    }

    fn deactivate_team(&mut self, team_id: Uuid) -> Result<(), &'static str> {
        match self.state.current_state.teams.get_mut(&team_id) {
            Some(team) => {
                team.deactivate()?;
                self.save_state();
                Ok(())
            },
            None => Err("Team not found"),
        }
    }

    fn reactivate_team(&mut self, team_id: Uuid) -> Result<(), &'static str> {
        match self.state.current_state.teams.get_mut(&team_id) {
            Some(team) => {
                team.reactivate()?;
                self.save_state();
                Ok(())
            },
            None => Err("Team not found"),
        }
    }

    fn update_team_status(&mut self, team_id: Uuid, new_status: TeamStatus) -> Result<(), &'static str> {
        match self.state.current_state.teams.get_mut(&team_id) {
            Some(team) => {
                team.change_status(new_status)?;
                self.save_state();
                Ok(())
            },
            None => Err("Team not found"),
        }
    }

    fn update_team_representative(&mut self, team_id: Uuid, new_representative: String) -> Result<(), &'static str> {
        match self.state.current_state.teams.get_mut(&team_id) {
            Some(team) => {
                team.representative = new_representative;
                self.save_state();
                Ok(())
            },
            None => Err("Team not found"),
        }
    }

    fn update_team_revenue(&mut self, team_id: Uuid, new_revenue: Vec<u64>) -> Result<(), &'static str> {
        match self.state.current_state.teams.get_mut(&team_id) {
            Some(team) => {
                team.update_revenue_data(new_revenue)?;
                self.save_state();
                Ok(())
            },
            None => Err("Team not found"),
        }
    }

    fn save_state(&self) -> Result<(), Box<dyn std::error::Error>> {
        let json = serde_json::to_string_pretty(&self.state)?;
        fs::write("budget_system_state.json", json)?;
        Ok(())
    }

    fn load_state(path: &str) -> Result<BudgetSystemState, Box<dyn std::error::Error>> {
        let json = fs::read_to_string(path)?;
        let state: BudgetSystemState = serde_json::from_str(&json)?;
        Ok(state)
    }

    async fn load_from_file(path: &str, ipc_path: &str, future_block_offset: u64) -> Result<Self, Box<dyn std::error::Error>> {
        let state = Self::load_state(path)?;
        let ethereum_service = Arc::new(EthereumService::new(ipc_path, future_block_offset).await?);
        
        Ok(Self {
            state,
            ethereum_service,
        })
    }

    fn get_state_at(&self, index: usize) -> Option<&SystemState> {
        self.state.history.get(index)
    }

    async fn create_raffle(&mut self, mut builder: RaffleBuilder) -> Result<Uuid, Box<dyn std::error::Error>> {
        let (initiation_block, randomness_block, randomness) = self.ethereum_service.get_raffle_randomness().await?;
        
        let raffle = builder
            .with_randomness(initiation_block, randomness_block, randomness)
            .build(&self.state.current_state.teams)?;
        
        let raffle_id = raffle.id;
        self.state.raffles.insert(raffle_id, raffle);
        self.save_state()?;
        Ok(raffle_id)
    }

    async fn import_historical_raffle(
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

        let epoch_id = self.state.current_epoch
            .ok_or("No active epoch")?;

        let randomness = self.ethereum_service.get_randomness(randomness_block).await?;

        let custom_team_order = team_order.map(|names| {
            names.into_iter()
                .filter_map(|name| self.get_team_id_by_name(&name))
                .collect::<Vec<Uuid>>()
        });

        let excluded_team_ids = excluded_teams.map(|names| {
            names.into_iter()
                .filter_map(|name| self.get_team_id_by_name(&name))
                .collect::<Vec<Uuid>>()
        }).unwrap_or_else(Vec::new);

        let raffle_config = RaffleConfig {
            proposal_id,
            epoch_id,
            initiation_block,
            randomness_block,
            block_randomness: randomness.clone(),
            total_counted_seats: total_counted_seats.unwrap_or(RaffleConfig::DEFAULT_TOTAL_COUNTED_SEATS),
            max_earner_seats: max_earner_seats.unwrap_or(RaffleConfig::DEFAULT_MAX_EARNER_SEATS),
            excluded_teams: excluded_team_ids,
            custom_allocation: None,
            custom_team_order,
            is_historical: true,
        };

        let mut raffle = Raffle::new(raffle_config, &self.state.current_state.teams)?;
        raffle.generate_scores()?;
        raffle.select_teams();

        let raffle_id = raffle.id;
        self.state.raffles.insert(raffle_id, raffle.clone());
        self.save_state()?;

        Ok((raffle_id, raffle))
    }

    fn conduct_raffle(&mut self, raffle_id: Uuid) -> Result<(), &'static str> {
        let raffle = self.state.raffles.get_mut(&raffle_id).ok_or("Raffle not found")?;
        RaffleService::conduct_raffle(raffle)?;
        self.save_state();
        Ok(())
    }

    fn add_proposal(&mut self, title: String, url: Option<String>, budget_request_details: Option<BudgetRequestDetails>) -> Result<Uuid, &'static str> {
        let current_epoch_id = self.state.current_epoch.ok_or("No active epoch")?;

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
            // Validate request_amounts
            if details.request_amounts.is_empty() {
                return Err("Budget request must include at least one token amount");
            }
            for &amount in details.request_amounts.values() {
                if amount <= 0.0 {
                    return Err("All requested amounts must be positive");
                }
            }
        }
    
        let proposal = Proposal::new(title, url, budget_request_details);
        let proposal_id = proposal.id;
        self.state.proposals.insert(proposal_id, proposal);

        if let Some(epoch) = self.state.epochs.get_mut(&current_epoch_id) {
            epoch.associated_proposals.push(proposal_id);
        } else {
            return Err("Current epoch not found");
        }
        self.save_state();
        Ok(proposal_id)
    }

    fn get_proposal(&self, id: Uuid) -> Option<&Proposal> {
        self.state.proposals.get(&id)
    }

    fn update_proposal_status(&mut self, id: Uuid, new_status: ProposalStatus) -> Result<(), &'static str> {
        if let Some(proposal) = self.state.proposals.get_mut(&id) {
           proposal.update_status(new_status);
            self.save_state();
            Ok(())
        } else {
            Err("Proposal not found")
        }
    }

    fn approve(&mut self, id: Uuid) -> Result<(), &'static str> {
        if let Some(proposal) = self.state.proposals.get_mut(&id) {
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
        if let Some(proposal) = self.state.proposals.get_mut(&id) {
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
        if let Some(proposal) = self.state.proposals.get_mut(&id) {
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
        if let Some(proposal) = self.state.proposals.get_mut(&id) {
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
        if let Some(proposal) = self.state.proposals.get_mut(&id) {
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
        if let Some(proposal) = self.state.proposals.get_mut(&id) {
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
        if let Some(proposal) = self.state.proposals.get_mut(&id) {
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
        let current_epoch_id = self.state.current_epoch.ok_or("No active epoch")?;
        
        let proposal = self.state.proposals.get(&proposal_id)
            .ok_or("Proposal not found")?;

        if !proposal.is_actionable() {
            return Err("Proposal is not in a votable state");
        }

        let raffle = &self.state.raffles.get(&raffle_id)
            .ok_or("Raffle not found")?;

        if raffle.result.is_none() {
            return Err("Raffle results have not been generated");
        }

        let vote = Vote::new_formal(
            proposal_id,
            current_epoch_id,
            raffle_id, 
            raffle.config.total_counted_seats as u32,
            threshold
        );
        let vote_id = vote.id;
        self.state.votes.insert(vote_id, vote);
        self.save_state();
        Ok(vote_id)
    }

    fn create_informal_vote(&mut self, proposal_id: Uuid) -> Result<Uuid, &'static str> {
        let current_epoch_id = self.state.current_epoch.ok_or("No active epoch")?;
        
        let proposal = self.state.proposals.get(&proposal_id)
            .ok_or("Proposal not found")?;

        if !proposal.is_actionable() {
            return Err("Proposal is not in a votable state");
        }

        let vote = Vote::new_informal(proposal_id, current_epoch_id);
        let vote_id = vote.id;
        self.state.votes.insert(vote_id, vote);
        self.save_state();
        Ok(vote_id)
    }

    fn cast_votes(&mut self, vote_id: Uuid, votes: Vec<(Uuid, VoteChoice)>) -> Result<(), &'static str> {
        let vote = self.state.votes.get_mut(&vote_id).ok_or("Vote not found")?;

        match &vote.vote_type {
            VoteType::Formal { raffle_id, .. } => {
                let raffle = self.state.raffles.get(raffle_id).ok_or("Associated raffle not found")?;
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
                        self.state.current_state.teams.get(team_id)
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

    fn retract_vote(&mut self, vote_id: Uuid, team_id: Uuid) -> Result<(), &'static str> {
        let vote = self.state.votes.get_mut(&vote_id).ok_or("Vote not found")?;
        vote.retract_vote(&team_id)?;
        self.save_state();
        Ok(())
    }

    fn close_vote(&mut self, vote_id: Uuid) -> Result<bool, &'static str> {
        let vote = self.state.votes.get_mut(&vote_id).ok_or("Vote not found")?;
        
        if vote.status == VoteStatus::Closed {
            return Err("Vote is already closed");
        }

        let team_points_option = vote.close()?;

        // If it's a formal vote, add points to team
        if let Some(team_points) = team_points_option {
            for (team_id, points) in team_points {
                if let Some(team) = self.state.current_state.teams.get_mut(&team_id) {
                    team.add_points(points);
                }
            }
        }

        let result = match &vote.result {
            Some(VoteResult::Formal { passed, .. }) => *passed,
            Some(VoteResult::Informal { .. }) => false,
            None => return Err("Vote result not available"),
        };

        // If it's a formal vote and it passed, approve the proposal
        if result {
            let proposal = self.state.proposals.get_mut(&vote.proposal_id)
                .ok_or("Associated proposal not found")?;
            proposal.approve()?;
        }

        self.save_state();
        Ok(result)
    }

    fn create_epoch(&mut self, name: &str, start_date:DateTime<Utc>, end_date: DateTime<Utc>) -> Result<Uuid, &'static str> {
        let new_epoch = Epoch::new(name.to_string(), start_date, end_date)?;

        // Check for overlapping epochs
        for epoch in self.state.epochs.values() {
            if (start_date < epoch.end_date && end_date > epoch.start_date) ||
            (epoch.start_date < end_date && epoch.end_date > start_date) {
                return Err("New epoch overlaps with an existing epoch");
            }
        }

        let epoch_id = new_epoch.id();
        self.state.epochs.insert(epoch_id, new_epoch);
        self.save_state();
        Ok(epoch_id)
    }

    fn update_epoch_name(&mut self, epoch_id: Uuid, new_name: String) -> Result<(), &'static str> {
        let epoch = self.state.epochs.get_mut(&epoch_id).ok_or("Epoch not found")?;
        epoch.set_name(new_name)?;
        self.save_state();
        Ok(())
    }

    fn activate_epoch(&mut self, epoch_id: Uuid) -> Result<(), &'static str> {
        if self.state.current_epoch.is_some() {
            return Err("Another epoch is currently active");
        }

        let epoch = self.state.epochs.get_mut(&epoch_id).ok_or("Epoch not found")?;

        if epoch.status != EpochStatus::Planned {
            return Err("Only planned epochs can be activated");
        }

        epoch.status = EpochStatus::Active;
        self.state.current_epoch = Some(epoch_id);

        // Reset points for all teams
        for team in self.state.current_state.teams.values_mut() {
            team.reset_points();
        }
        self.save_state();
        Ok(())
    }

    fn set_epoch_reward(&mut self, token: &str, amount: f64) -> Result<(), &'static str> {
        let epoch_id = self.state.current_epoch.ok_or("No active epoch")?;
        let epoch = self.state.epochs.get_mut(&epoch_id).ok_or("Epoch not found")?;
        
        epoch.set_reward(token.to_string(), amount);
        self.save_state();
        Ok(())
    }

    fn calculate_epoch_rewards(&self) -> Result<HashMap<Uuid, TeamReward>, &'static str> {
        let epoch_id = self.state.current_epoch.ok_or("No active epoch")?;
        let epoch = self.state.epochs.get(&epoch_id).ok_or("Epoch not found")?;
        
        epoch.calculate_rewards(&self.state.current_state.teams)
    }

    fn close_epoch_with_rewards(&mut self) -> Result<(), &'static str> {
        let epoch_id = self.state.current_epoch.ok_or("No active epoch")?;
        let epoch = self.state.epochs.get_mut(&epoch_id).ok_or("Epoch not found")?;
        
        epoch.close_with_rewards(&self.state.current_state.teams)?;
        
        // Reset points for all teams
        for team in self.state.current_state.teams.values_mut() {
            team.reset_points();
        }

        self.state.current_epoch = None;
        self.save_state();
        Ok(())
    }

    fn close_current_epoch(&mut self) -> Result<(), &'static str> {
        let epoch_id = self.state.current_epoch.ok_or("No active epoch");
        let epoch = self.state.epochs.get_mut(&epoch_id?).unwrap();

        epoch.close_current_epoch();
        self.state.current_epoch = None;
        self.save_state();
        Ok(())
    }

    fn get_current_epoch(&self) -> Option<&Epoch> {
        self.state.current_epoch.and_then(|id| self.state.epochs.get(&id))
    }

    fn list_epochs(&self, status: Option<EpochStatus>) -> Vec<&Epoch> {
        self.state.epochs.values()
            .filter(|&epoch| status.map_or(true, |s| epoch.status == s))
            .collect()
    }

    fn get_proposals_for_epoch(&self, epoch_id: Uuid) -> Vec<&Proposal> {
        if let Some(epoch) = self.state.epochs.get(&epoch_id) {
            epoch.associated_proposals.iter()
                .filter_map(|&id| self.state.proposals.get(&id))
                .collect()
        } else {
            vec![]
        }
    }

    fn get_votes_for_epoch(&self, epoch_id: Uuid) -> Vec<&Vote> {
        self.state.votes.values()
            .filter(|vote| vote.epoch_id == epoch_id)
            .collect()
    }

    fn get_raffles_for_epoch(&self, epoch_id: Uuid) -> Vec<&Raffle> {
        self.state.raffles.values()
            .filter(|raffle| raffle.config.epoch_id == epoch_id)
            .collect()
    }

    fn get_epoch_for_vote(&self, vote_id: Uuid) -> Option<&Epoch> {
        self.state.votes.get(&vote_id).and_then(|vote| self.state.epochs.get(&vote.epoch_id))
    }

    fn get_epoch_for_raffle(&self, raffle_id: Uuid) -> Option<&Epoch> {
        self.state.raffles.get(&raffle_id).and_then(|raffle| self.state.epochs.get(&raffle.config.epoch_id))
    }

    fn transition_to_next_epoch(&mut self) -> Result<(), &'static str> {
        self.close_current_epoch()?;

        let next_epoch = self.state.epochs.values()
            .filter(|&epoch| epoch.status == EpochStatus::Planned)
            .min_by_key(|&epoch| epoch.start_date)
            .ok_or("No planned epochs available")?;

        self.activate_epoch(next_epoch.id())
    }

    fn update_epoch_dates(&mut self, epoch_id: Uuid, new_start: DateTime<Utc>, new_end: DateTime<Utc>) -> Result<(), &'static str> {
        // Check for overlaps with other epochs
        for other_epoch in self.state.epochs.values() {
            if other_epoch.id != epoch_id &&
               ((new_start < other_epoch.end_date && new_end > other_epoch.start_date) ||
                (other_epoch.start_date < new_end && other_epoch.end_date > new_start)) {
                return Err("New dates overlap with an existing epoch");
            }
        }
        
        let epoch = self.state.epochs.get_mut(&epoch_id).ok_or("Epoch not found")?;

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
        let epoch = self.state.epochs.get(&epoch_id).ok_or("Epoch not found")?;

        if epoch.status != EpochStatus::Planned {
            return Err("Can only cancel planned epochs");
        }

        self.state.epochs.remove(&epoch_id);
        Ok(())
    }

    fn get_epoch_id_by_name(&self, name: &str) -> Option<Uuid> {
        self.state.epochs.iter()
            .find(|(_, epoch)| epoch.name() == name)
            .map(|(id, _)| *id)
    }

    fn get_team_id_by_name(&self, name: &str) -> Option<Uuid> {
        self.state.current_state.teams.iter()
            .find(|(_, team)| team.name == name)
            .map(|(id, _)| *id)
    }

    fn get_proposal_id_by_name(&self, name: &str) -> Option<Uuid> {
        self.state.proposals.iter()
            .find(|(_, proposal)| proposal.title == name)
            .map(|(id, _)| *id)
    }

    fn import_predefined_raffle(
        &mut self,
        proposal_name: &str,
        counted_teams: Vec<String>,
        uncounted_teams: Vec<String>,
        total_counted_seats: usize,
        max_earner_seats: usize
    ) -> Result<Uuid, Box<dyn Error>> {
        let proposal_id = self.get_proposal_id_by_name(proposal_name)
            .ok_or_else(|| format!("Proposal not found: {}", proposal_name))?;
        
        let epoch_id = self.state.current_epoch
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

        let raffle_config = RaffleConfig {
            proposal_id,
            epoch_id,
            initiation_block: 0,
            randomness_block: 0,
            block_randomness: "N/A".to_string(),
            total_counted_seats,
            max_earner_seats,
            excluded_teams: Vec::new(),
            custom_allocation: Some(HashMap::new()),
            custom_team_order: Some(Vec::new()),
            is_historical: true,
        };

        let mut raffle = Raffle::new(raffle_config, &self.state.current_state.teams)?;
        
        raffle.result = Some(RaffleResult {
            counted: counted_team_ids,
            uncounted: uncounted_team_ids,
        });

        let raffle_id = raffle.id;
        self.state.raffles.insert(raffle_id, raffle);
        self.save_state()?;

        Ok(raffle_id)
    }

    fn import_historical_vote(
        &mut self,
        proposal_name: &str,
        passed: bool,
        participating_teams: Vec<String>,
        non_participating_teams: Vec<String>
    ) -> Result<Uuid, Box<dyn Error>> {
        let proposal_id = self.get_proposal_id_by_name(proposal_name)
            .ok_or_else(|| format!("Proposal not found: {}", proposal_name))?;

        // Find the associated raffle
        let raffle_id = self.state.raffles.iter()
            .find(|(_, raffle)| raffle.config.proposal_id == proposal_id)
            .map(|(id, _)| *id)
            .ok_or_else(|| format!("No raffle found for proposal: {}", proposal_name))?;

        let raffle = self.state.raffles.get(&raffle_id)
            .ok_or_else(|| format!("Raffle not found: {}", raffle_id))?;

        let epoch_id = raffle.config.epoch_id;

        // Create a new historical vote
        let mut vote = Vote::new_formal(
            proposal_id,
            epoch_id,
            raffle_id,
            raffle.config.total_counted_seats as u32,
            None // Use default threshold
        );
        vote.is_historical = true;

        // Determine participating and non-participating teams
        let (participating_ids, non_participating_ids) = self.determine_participation(
            raffle,
            &participating_teams,
            &non_participating_teams
        )?;

        // Record participation without specifying vote choices
        if let VoteParticipation::Formal { counted, uncounted } = &mut vote.participation {
            for &team_id in &participating_ids {
                if raffle.result.as_ref().unwrap().counted.contains(&team_id) {
                    counted.push(team_id);
                } else {
                    uncounted.push(team_id);
                }
            }
        }

        // Set the result based on the 'passed' parameter
        vote.result = Some(if passed {
            VoteResult::Formal {
                counted: VoteCount { yes: vote.participation.counted_count(), no: 0 },
                uncounted: VoteCount { yes: vote.participation.uncounted_count(), no: 0 },
                passed: true,
            }
        } else {
            VoteResult::Formal {
                counted: VoteCount { yes: 0, no: vote.participation.counted_count() },
                uncounted: VoteCount { yes: 0, no: vote.participation.uncounted_count() },
                passed: false,
            }
        });

        // Close the vote and allocate points
        let team_points = vote.calculate_points();
        for (team_id, points) in team_points.iter() {
            if let Some(team) = self.state.current_state.teams.get_mut(team_id) {
                team.add_points(*points);
            }
        }

        // Approve and close the proposal
        let proposal = self.state.proposals.get_mut(&proposal_id)
            .ok_or_else(|| format!("Proposal not found: {}", proposal_id))?;
        
        if passed {
            proposal.approve()?;
        } else {
            proposal.reject()?;
        }
        proposal.update_status(ProposalStatus::Closed);

        // Save the vote
        let vote_id = vote.id;
        self.state.votes.insert(vote_id, vote);

        self.save_state()?;

        Ok(vote_id)
    }

    fn determine_participation(
        &self,
        raffle: &Raffle,
        participating_teams: &[String],
        non_participating_teams: &[String]
    ) -> Result<(Vec<Uuid>, Vec<Uuid>), Box<dyn Error>> {
        let raffle_result = raffle.result.as_ref()
            .ok_or("Raffle result not found")?;

        let all_team_ids: Vec<Uuid> = raffle_result.counted.iter()
            .chain(raffle_result.uncounted.iter())
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
}

// Script commands

#[derive(Deserialize, Clone)]
#[serde(tag = "type", content = "params")]
enum ScriptCommand {
    CreateEpoch { name: String, start_date: DateTime<Utc>, end_date: DateTime<Utc> },
    ActivateEpoch { name: String },
    SetEpochReward { token: String, amount: f64 },
    AddTeam { name: String, representative: String, trailing_monthly_revenue: Option<Vec<u64>> },
    AddProposal { 
        title: String, 
        url: Option<String>, 
        budget_request_details: Option<BudgetRequestDetailsScript> 
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
}

#[derive(Deserialize, Clone)]
struct BudgetRequestDetailsScript {
    team: Option<String>,
    request_amounts: HashMap<String, f64>,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
    payment_status: Option<PaymentStatus>,
}

async fn execute_command(budget_system: &mut BudgetSystem, command: ScriptCommand) -> Result<(), Box<dyn Error>> {
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
            let team_id = budget_system.add_team(name.clone(), representative, trailing_monthly_revenue)?;
            println!("Added team: {} ({})", name, team_id);
        },
        ScriptCommand::AddProposal { title, url, budget_request_details } => {
            let budget_request_details = budget_request_details.map(|details| {
                BudgetRequestDetails {
                    team: details.team.as_ref().and_then(|name| budget_system.get_team_id_by_name(name)),
                    request_amounts: details.request_amounts,
                    start_date: details.start_date,
                    end_date: details.end_date,
                    payment_status: details.payment_status,
                }
            });
            let proposal_id = budget_system.add_proposal(title.clone(), url, budget_request_details)?;
            println!("Added proposal: {} ({})", title, proposal_id);
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
            println!("Imported predefined raffle for proposal '{}' (Raffle ID: {})", proposal_name, raffle_id);
            println!("  Counted teams: {:?}", counted_teams);
            println!("  Uncounted teams: {:?}", uncounted_teams);
            println!("  Total counted seats: {}", total_counted_seats);
            println!("  Max earner seats: {}", max_earner_seats);
        },
        ScriptCommand::ImportHistoricalVote { 
            proposal_name, 
            passed, 
            participating_teams,
            non_participating_teams 
        } => {
            let vote_id = budget_system.import_historical_vote(
                &proposal_name,
                passed,
                participating_teams.clone(),
                non_participating_teams.clone()
            )?;
            println!("Imported historical vote for proposal '{}' (Vote ID: {})", proposal_name, vote_id);
            println!("  Passed: {}", passed);
            if !participating_teams.is_empty() {
                println!("  Participating teams: {:?}", participating_teams);
            } else if !non_participating_teams.is_empty() {
                println!("  Non-participating teams: {:?}", non_participating_teams);
            } else {
                println!("  All teams participated");
            }
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
                total_counted_seats,
                max_earner_seats
            ).await?;

            println!("Imported historical raffle for proposal '{}' (Raffle ID: {})", proposal_name, raffle_id);
            println!("Randomness: {}", raffle.config.block_randomness);

            // Print excluded teams
            if let Some(excluded) = excluded_teams {
                println!("Excluded teams: {:?}", excluded);
            }

            // Print ballot ID ranges for each team
            for (team_id, snapshot) in &raffle.team_snapshots {
                let tickets: Vec<_> = raffle.tickets.iter()
                    .filter(|t| t.team_id == *team_id)
                    .collect();
                
                if !tickets.is_empty() {
                    let start = tickets.first().unwrap().index;
                    let end = tickets.last().unwrap().index;
                    println!("Team '{}' ballot range: {} - {}", snapshot.name, start, end);
                }
            }

            // Print raffle results
            if let Some(result) = &raffle.result {
                println!("Counted seats:");
                println!("Earner seats:");
                let mut earner_count = 0;
                for &team_id in &result.counted {
                    let snapshot = &raffle.team_snapshots[&team_id];
                    if let TeamStatus::Earner { .. } = snapshot.status {
                        earner_count += 1;
                        let best_score = raffle.tickets.iter()
                            .filter(|t| t.team_id == team_id)
                            .map(|t| t.score)
                            .max_by(|a, b| a.partial_cmp(b).unwrap())
                            .unwrap_or(0.0);
                        println!("  {} (score: {})", snapshot.name, best_score);
                    }
                }
                println!("Supporter seats:");
                for &team_id in &result.counted {
                    let snapshot = &raffle.team_snapshots[&team_id];
                    if let TeamStatus::Supporter = snapshot.status {
                        let best_score = raffle.tickets.iter()
                            .filter(|t| t.team_id == team_id)
                            .map(|t| t.score)
                            .max_by(|a, b| a.partial_cmp(b).unwrap())
                            .unwrap_or(0.0);
                        println!("  {} (score: {})", snapshot.name, best_score);
                    }
                }
                println!("Total counted seats: {} (Earners: {}, Supporters: {})", 
                         result.counted.len(), earner_count, result.counted.len() - earner_count);

                println!("Uncounted seats:");
                println!("Earner seats:");
                for &team_id in &result.uncounted {
                    let snapshot = &raffle.team_snapshots[&team_id];
                    if let TeamStatus::Earner { .. } = snapshot.status {
                        let best_score = raffle.tickets.iter()
                            .filter(|t| t.team_id == team_id)
                            .map(|t| t.score)
                            .max_by(|a, b| a.partial_cmp(b).unwrap())
                            .unwrap_or(0.0);
                        println!("  {} (score: {})", snapshot.name, best_score);
                    }
                }
                println!("Supporter seats:");
                for &team_id in &result.uncounted {
                    let snapshot = &raffle.team_snapshots[&team_id];
                    if let TeamStatus::Supporter = snapshot.status {
                        let best_score = raffle.tickets.iter()
                            .filter(|t| t.team_id == team_id)
                            .map(|t| t.score)
                            .max_by(|a, b| a.partial_cmp(b).unwrap())
                            .unwrap_or(0.0);
                        println!("  {} (score: {})", snapshot.name, best_score);
                    }
                }
            } else {
                println!("Raffle result not available");
            }
        },

    }
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Configuration
    const IPC_PATH: &str = "/tmp/reth.ipc";
    const FUTURE_BLOCK_OFFSET: u64 = 10;
    const STATE_FILE: &str = "budget_system_state.json";
    const SCRIPT_FILE: &str = "input_script.json";

    // Initialize or load the BudgetSystem
    let mut budget_system = match BudgetSystem::load_from_file(STATE_FILE, IPC_PATH, FUTURE_BLOCK_OFFSET).await {
        Ok(system) => {
            println!("Loaded existing state from {}", STATE_FILE);
            system
        },
        Err(_) => {
            println!("No existing state found. Creating a new BudgetSystem.");
            BudgetSystem::new(IPC_PATH, FUTURE_BLOCK_OFFSET).await?
        },
    };

    // Read and execute the script
    if Path::new(SCRIPT_FILE).exists() {
        let script_content = fs::read_to_string(SCRIPT_FILE)?;
        let script: Vec<ScriptCommand> = serde_json::from_str(&script_content)?;
        
        for command in script {
            execute_command(&mut budget_system, command).await?;
        }

        println!("Script execution completed.");
    } else {
        println!("No script file found at {}. Skipping script execution.", SCRIPT_FILE);
    }

    // Save the current state
    budget_system.save_state()?;
    println!("Saved current state to {}", STATE_FILE);

    Ok(())
}