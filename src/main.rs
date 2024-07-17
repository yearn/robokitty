use chrono::{DateTime, NaiveDate, Utc};
use ethers::prelude::*;
use prettytable::{Table, Row, Cell};
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
    team_snapshots: Vec<TeamSnapshot>,
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
    announced_at: Option<NaiveDate>,
    published_at: Option<NaiveDate>,
    resolved_at: Option<NaiveDate>,
    is_historical: bool,
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
        let mut team_snapshots = Vec::new();
        let mut tickets = Vec::new();

        // Create team snapshots
        let mut active_teams: Vec<_> = teams.values()
            .filter(|team| team.status != TeamStatus::Inactive)
            .collect();

        // Sort teams based on custom order or by name
        if let Some(custom_order) = &config.custom_team_order {
            active_teams.sort_by_key(|team| custom_order.iter().position(|&id| id == team.id).unwrap_or(usize::MAX));
        } else {
            active_teams.sort_by(|a, b| a.name.cmp(&b.name));
        }

        // Create snapshots and tickets
        for team in active_teams {
            let snapshot = TeamSnapshot {
                id: team.id,
                name: team.name.clone(),
                representative: team.representative.clone(),
                status: team.status.clone(),
                points: team.points,
                snapshot_time: Utc::now(),
                raffle_status: if config.excluded_teams.contains(&team.id) {
                    RaffleParticipationStatus::Excluded
                } else {
                    RaffleParticipationStatus::Included
                },
            };
            team_snapshots.push(snapshot);

            let ticket_count = match &team.status {
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
                    team_id: team.id,
                    index: tickets.len() as u64,
                    score: 0.0, // Will be updated later for non-excluded teams
                });
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
                .filter(|(&team_id, _)| self.team_snapshots.iter().any(|s| s.id == team_id && s.raffle_status == RaffleParticipationStatus::Included))
                .map(|(&team_id, &ticket_count)| (team_id, ticket_count))
                .collect()
        } else if let Some(custom_order) = &self.config.custom_team_order {
            custom_order.iter()
                .filter(|&team_id| self.team_snapshots.iter().any(|s| s.id == *team_id && s.raffle_status == RaffleParticipationStatus::Included))
                .filter_map(|&team_id| {
                    self.team_snapshots.iter()
                        .find(|s| s.id == team_id)
                        .and_then(|snapshot| snapshot.calculate_ticket_count().ok())
                        .map(|count| (team_id, count))
                })
                .collect()
        } else {
            self.team_snapshots.iter()
                .filter(|snapshot| snapshot.raffle_status == RaffleParticipationStatus::Included)
                .filter_map(|snapshot| {
                    snapshot.calculate_ticket_count().ok().map(|count| (snapshot.id, count))
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
        if let Some(team) = self.team_snapshots.iter().find(|s| s.id == team_id) {
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
            .filter(|t| self.team_snapshots.iter().any(|s| s.id == t.team_id && matches!(s.status, TeamStatus::Earner { .. })))
            .collect();
        earner_tickets.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap());

        let mut supporter_tickets: Vec<_> = self.tickets.iter()
            .filter(|t| !self.config.excluded_teams.contains(&t.team_id))
            .filter(|t| self.team_snapshots.iter().any(|s| s.id == t.team_id && matches!(s.status, TeamStatus::Supporter)))
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
    fn new(title: String, url: Option<String>, budget_request_details: Option<BudgetRequestDetails>, announced_at: Option<NaiveDate>, published_at: Option<NaiveDate>, is_historical: Option<bool>) -> Self {
        let is_historical = is_historical.unwrap_or(false);

        Proposal {
            id: Uuid::new_v4(),
            title,
            url,
            status: ProposalStatus::Open,
            resolution: None,
            budget_request_details,
            announced_at,
            published_at,
            resolved_at: None,
            is_historical,
        }
    }

    fn set_announced_at(&mut self, date: NaiveDate) {
        self.announced_at = Some(date);
    }

    fn set_published_at(&mut self, date: NaiveDate) {
        self.published_at = Some(date);
    }

    fn set_resolved_at(&mut self, date: NaiveDate) {
        self.resolved_at = Some(date);
    }

    fn set_historical(&mut self, is_historical: bool) {
        self.is_historical = is_historical;
    }

    fn set_dates(&mut self, announced_at: Option<NaiveDate>, published_at: Option<NaiveDate>, resolved_at: Option<NaiveDate>) {
        if let Some(date) = announced_at {
            self.set_announced_at(date);
        }
        if let Some(date) = published_at {
            self.set_published_at(date);
        }
        if let Some(date) = resolved_at {
            self.set_resolved_at(date);
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

    fn get_result(&self) -> Option<bool> {
        self.result.as_ref().map(|r| match r {
            VoteResult::Formal { passed, .. } => *passed,
            VoteResult::Informal { .. } => false, // Informal votes don't have a pass/fail status
        })
    }

    fn get_vote_counts(&self) -> Option<(VoteCount, VoteCount)> {
        match &self.result {
            Some(VoteResult::Formal { counted, uncounted, .. }) => Some((*counted, *uncounted)),
            _ => None,
        }
    }

    fn is_vote_count_available(&self) -> bool {
        !self.is_historical
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

    fn update_team_status(&mut self, team_id: Uuid, new_status: &TeamStatus) -> Result<(), &'static str> {
        match self.state.current_state.teams.get_mut(&team_id) {
            Some(team) => {
                team.change_status(new_status.clone())?;
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

    fn conduct_raffle(&mut self, raffle_id: Uuid) -> Result<(), &'static str> {
        let raffle = self.state.raffles.get_mut(&raffle_id).ok_or("Raffle not found")?;
        RaffleService::conduct_raffle(raffle)?;
        self.save_state();
        Ok(())
    }

    fn add_proposal(&mut self, title: String, url: Option<String>, budget_request_details: Option<BudgetRequestDetails>, announced_at: Option<NaiveDate>, published_at: Option<NaiveDate>, is_historical: Option<bool>) -> Result<Uuid, &'static str> {
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
    
        let proposal = Proposal::new(title, url, budget_request_details, announced_at, published_at, is_historical);
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

    fn set_proposal_dates(&mut self, proposal_name: &str, announced_at: Option<NaiveDate>, published_at: Option<NaiveDate>, resolved_at: Option<NaiveDate>) -> Result<(), &'static str> {
        let proposal = self.state.proposals.values_mut()
            .find(|p| p.title == proposal_name)
            .ok_or("Proposal not found")?;

        proposal.set_dates(announced_at, published_at, resolved_at);
        self.save_state();
        Ok(())
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

    fn close_with_reason(&mut self, id: Uuid, resolution: &Resolution) -> Result<(), &'static str> {
        if let Some(proposal) = self.state.proposals.get_mut(&id) {
            if proposal.status == ProposalStatus::Closed {
                return Err("Proposal is already closed");
            }
            if let Some(details) = &proposal.budget_request_details {
                if matches!(details.payment_status, Some(PaymentStatus::Paid)) {
                    return Err("Cannot close: Proposal is already paid");
                }
            }
            proposal.set_resolution(resolution.clone());
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
            custom_allocation: None,
            custom_team_order: Some(counted_team_ids.iter().chain(uncounted_team_ids.iter()).cloned().collect()),
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

        let raffle_id = self.state.raffles.iter()
            .find(|(_, raffle)| raffle.config.proposal_id == proposal_id)
            .map(|(id, _)| *id)
            .ok_or_else(|| format!("No raffle found for proposal: {}", proposal_name))?;

        let raffle = self.state.raffles.get(&raffle_id)
            .ok_or_else(|| format!("Raffle not found: {}", raffle_id))?;

        let epoch_id = raffle.config.epoch_id;

        let (participating_ids, _) = self.determine_participation(
            raffle,
            &participating_teams,
            &non_participating_teams
        )?;

        let participation = VoteParticipation::Formal {
            counted: participating_ids.iter()
                .filter(|&id| raffle.result.as_ref().unwrap().counted.contains(id))
                .cloned()
                .collect(),
            uncounted: participating_ids.iter()
                .filter(|&id| raffle.result.as_ref().unwrap().uncounted.contains(id))
                .cloned()
                .collect(),
        };

        let vote = Vote {
            id: Uuid::new_v4(),
            proposal_id,
            epoch_id,
            vote_type: VoteType::Formal {
                raffle_id,
                total_eligible_seats: raffle.config.total_counted_seats as u32,
                threshold: Vote::DEFAULT_QUALIFIED_MAJORITY_THRESHOLD
            },
            status: VoteStatus::Closed,
            participation,
            result: Some(VoteResult::Formal {
                counted: VoteCount { yes: 0, no: 0 }, // We don't know the actual counts
                uncounted: VoteCount { yes: 0, no: 0 }, // We don't know the actual counts
                passed,
            }),
            opened_at: Utc::now(), // Use import time as a placeholder
            closed_at: Some(Utc::now()), // Use import time as a placeholder
            is_historical: true,
            votes: HashMap::new(), // Empty, as we don't know individual votes
        };

        let vote_id = vote.id;

        // Calculate and award points
        if let VoteParticipation::Formal { counted, uncounted } = &vote.participation {
            for &team_id in counted {
                if let Some(team) = self.state.current_state.teams.get_mut(&team_id) {
                    team.add_points(Vote::COUNTED_VOTE_POINTS);
                }
            }
            for &team_id in uncounted {
                if let Some(team) = self.state.current_state.teams.get_mut(&team_id) {
                    team.add_points(Vote::UNCOUNTED_VOTE_POINTS);
                }
            }
        }

        self.state.votes.insert(vote_id, vote);

        // Update proposal status based on vote result
        let proposal = self.state.proposals.get_mut(&proposal_id)
            .ok_or_else(|| format!("Proposal not found: {}", proposal_id))?;
        
        if passed {
            proposal.approve()?;
        } else {
            proposal.reject()?;
        }
        proposal.update_status(ProposalStatus::Closed);

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

    fn print_team_report(&self) -> String {
        let mut teams: Vec<&Team> = self.state.current_state.teams.values().collect();
        teams.sort_by(|a, b| a.name.cmp(&b.name));

        let mut report = String::from("Team Report:\n\n");

        for team in teams {
            report.push_str(&format!("Name: {}\n", team.name));
            report.push_str(&format!("ID: {}\n", team.id));
            report.push_str(&format!("Representative: {}\n", team.representative));
            report.push_str(&format!("Status: {:?}\n", team.status));
            report.push_str(&format!("Points: {}\n", team.points));

            if let TeamStatus::Earner { trailing_monthly_revenue } = &team.status {
                report.push_str(&format!("Trailing Monthly Revenue: {:?}\n", trailing_monthly_revenue));
            }

            report.push_str("\n");
        }

        report
    }

    fn add_count_if_positive(report: &mut String, label: &str, count: usize) {
        if count > 0 {
            report.push_str(&format!("{}: {}\n", label, count));
        }
    }

    fn print_epoch_state(&self) -> Result<String, &'static str> {
        let epoch = self.get_current_epoch().ok_or("No active epoch")?;
        let proposals = self.get_proposals_for_epoch(epoch.id());

        let total_proposals = proposals.len();
        let mut open_proposals = Vec::new();
        let mut approved_count = 0;
        let mut rejected_count = 0;
        let mut invalid_count = 0;
        let mut duplicate_count = 0;
        let mut retracted_count = 0;

        for proposal in &proposals {
            match &proposal.resolution {
                Some(Resolution::Approved) => approved_count += 1,
                Some(Resolution::Rejected) => rejected_count += 1,
                Some(Resolution::Invalid) => invalid_count += 1,
                Some(Resolution::Duplicate) => duplicate_count += 1,
                Some(Resolution::Retracted) => retracted_count += 1,
                None => {
                    if proposal.is_actionable() {
                        open_proposals.push(proposal);
                    }
                }
            }
        }

        let open_count = open_proposals.len();

        let mut report = String::from("Current Epoch State:\n\n");
        report.push_str(&format!("Name: {}\n", epoch.name()));
        report.push_str(&format!("ID: {}\n", epoch.id()));
        report.push_str(&format!("Start Date: {}\n", epoch.start_date()));
        report.push_str(&format!("End Date: {}\n", epoch.end_date()));
        report.push_str(&format!("Status: {:?}\n\n", epoch.status()));
        report.push_str("Proposal Counts:\n");
        report.push_str(&format!("Total Proposals: {}\n", total_proposals));
        
        Self::add_count_if_positive(&mut report, "Open Proposals", open_count);
        Self::add_count_if_positive(&mut report, "Approved Proposals", approved_count);
        Self::add_count_if_positive(&mut report, "Rejected Proposals", rejected_count);
        Self::add_count_if_positive(&mut report, "Invalid Proposals", invalid_count);
        Self::add_count_if_positive(&mut report, "Duplicate Proposals", duplicate_count);
        Self::add_count_if_positive(&mut report, "Retracted Proposals", retracted_count);

        if let Some(reward) = &epoch.reward {
            report.push_str(&format!("\nEpoch Reward: {} {}\n", reward.amount, reward.token));
        } else {
            report.push_str("\nEpoch Reward: Not set\n");
        }

        if !open_proposals.is_empty() {
            report.push_str("\nOpen Proposals:\n");
            let mut table = Table::new();
            table.add_row(Row::new(vec![
                Cell::new("Title"),
                Cell::new("Days Open"),
                Cell::new("Status"),
                Cell::new("URL"),
            ]));

            for proposal in open_proposals {
                let days_open = self.days_open(proposal);
                let status = match proposal.status {
                    ProposalStatus::Open => "Open",
                    ProposalStatus::Closed => "Closed",
                    ProposalStatus::Reopened => "Reopened",
                };
                table.add_row(Row::new(vec![
                    Cell::new(&proposal.title),
                    Cell::new(&days_open.to_string()),
                    Cell::new(status),
                    Cell::new(proposal.url.as_deref().unwrap_or("N/A")),
                ]));
            }

            report.push_str(&table.to_string());
        }

        Ok(report)
    }

    fn print_team_vote_participation(&self, team_name: &str, epoch_name: Option<&str>) -> Result<String, Box<dyn Error>> {
        let team_id = self.get_team_id_by_name(team_name)
            .ok_or_else(|| format!("Team not found: {}", team_name))?;

        let epoch = if let Some(name) = epoch_name {
            self.state.epochs.values()
                .find(|e| e.name() == name)
                .ok_or_else(|| format!("Epoch not found: {}", name))?
        } else {
            self.get_current_epoch()
                .ok_or("No active epoch and no epoch specified")?
        };

        let mut report = format!("Vote Participation Report for Team: {}\n", team_name);
        report.push_str(&format!("Epoch: {} ({})\n\n", epoch.name(), epoch.id()));
        let mut vote_reports = Vec::new();

        for vote_id in epoch.associated_proposals.iter()
            .filter_map(|proposal_id| self.state.votes.values()
                .find(|v| v.proposal_id == *proposal_id)
                .map(|v| v.id)) 
        {
            let vote = &self.state.votes[&vote_id];
            let participation_status = match &vote.participation {
                VoteParticipation::Formal { counted, uncounted } => {
                    if counted.contains(&team_id) {
                        Some("Counted")
                    } else if uncounted.contains(&team_id) {
                        Some("Uncounted")
                    } else {
                        None
                    }
                },
                VoteParticipation::Informal(participants) => {
                    if participants.contains(&team_id) {
                        Some("N/A (Informal)")
                    } else {
                        None
                    }
                },
            };

            if let Some(status) = participation_status {
                let proposal = self.state.proposals.get(&vote.proposal_id)
                    .ok_or_else(|| format!("Proposal not found for vote: {}", vote_id))?;

                let vote_type = match vote.vote_type {
                    VoteType::Formal { .. } => "Formal",
                    VoteType::Informal => "Informal",
                };

                let result = match vote.get_result() {
                    Some(true) => "Passed",
                    Some(false) => "Failed",
                    None => "Pending",
                };

                let points = vote.add_points_for_vote(&team_id);

                vote_reports.push((
                    vote.opened_at,
                    format!(
                        "Vote ID: {}\n\
                        Proposal: {}\n\
                        Type: {}\n\
                        Participation: {}\n\
                        Result: {}\n\
                        Points Earned: {}\n\n",
                        vote_id, proposal.title, vote_type, status, result, points
                    )
                ));
            }
        }

        // Sort vote reports by date, most recent first
        vote_reports.sort_by(|a, b| b.0.cmp(&a.0));

        // Use a reference to iterate over vote_reports
        for (_, vote_report) in &vote_reports {
            report.push_str(vote_report);
        }

        if vote_reports.is_empty() {
            report.push_str("This team has not participated in any votes during this epoch.\n");
        }

        Ok(report)
    }

    fn days_open(&self, proposal: &Proposal) -> i64 {
        let announced_date = proposal.announced_at
            .unwrap_or_else(|| Utc::now().date_naive());
        Utc::now().date_naive().signed_duration_since(announced_date).num_days()
    }

    fn prepare_raffle(&mut self, proposal_name: &str, excluded_teams: Option<Vec<String>>) -> Result<(Uuid, Vec<RaffleTicket>), Box<dyn Error>> {
        let proposal_id = self.get_proposal_id_by_name(proposal_name)
            .ok_or_else(|| format!("Proposal not found: {}", proposal_name))?;
        let epoch_id = self.state.current_epoch
            .ok_or("No active epoch")?;

        let excluded_team_ids = excluded_teams.map(|names| {
            names.into_iter()
                .filter_map(|name| self.get_team_id_by_name(&name))
                .collect::<Vec<Uuid>>()
        }).unwrap_or_else(Vec::new);

        let raffle_config = RaffleConfig {
            proposal_id,
            epoch_id,
            initiation_block: 0,
            randomness_block: 0,
            block_randomness: String::new(),
            total_counted_seats: RaffleConfig::DEFAULT_TOTAL_COUNTED_SEATS,
            max_earner_seats: RaffleConfig::DEFAULT_MAX_EARNER_SEATS,
            excluded_teams: excluded_team_ids,
            custom_allocation: None,
            custom_team_order: None,
            is_historical: false,
        };

        let raffle = Raffle::new(raffle_config, &self.state.current_state.teams)?;
        let raffle_id = raffle.id;
        let tickets = raffle.tickets.clone();
        
        self.state.raffles.insert(raffle_id, raffle);
        self.save_state()?;

        Ok((raffle_id, tickets))
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
    async fn finalize_raffle(&mut self, raffle_id: Uuid, block_number: u64, randomness: String) -> Result<Raffle, Box<dyn Error>> {
        let raffle = self.state.raffles.get_mut(&raffle_id)
            .ok_or_else(|| format!("Raffle not found: {}", raffle_id))?;

        raffle.config.initiation_block = block_number;
        raffle.config.randomness_block = block_number;
        raffle.config.block_randomness = randomness;

        raffle.generate_scores()?;
        raffle.select_teams();

        let raffle_clone = raffle.clone();
        self.save_state()?;

        Ok(raffle_clone)
    }

    fn group_tickets_by_team(&self, tickets: &[RaffleTicket]) -> Vec<(String, u64, u64)> {
        let mut grouped_tickets: Vec<(String, u64, u64)> = Vec::new();
        let mut current_team: Option<(String, u64, u64)> = None;

        for ticket in tickets {
            let team_name = self.state.current_state.teams.get(&ticket.team_id)
                .map(|team| team.name.clone())
                .unwrap_or_else(|| format!("Unknown Team ({})", ticket.team_id));

            match &mut current_team {
                Some((name, _, end)) if *name == team_name => {
                    *end = ticket.index;
                }
                _ => {
                    if let Some(team) = current_team.take() {
                        grouped_tickets.push(team);
                    }
                    current_team = Some((team_name, ticket.index, ticket.index));
                }
            }
        }

        if let Some(team) = current_team {
            grouped_tickets.push(team);
        }

        grouped_tickets
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
        budget_request_details: Option<BudgetRequestDetailsScript>,
        announced_at: Option<NaiveDate>,
        published_at: Option<NaiveDate>,
        is_historical: Option<bool>,
    },
    SetProposalDates {
        proposal_name: String,
        announced_at: Option<NaiveDate>,
        published_at: Option<NaiveDate>,
        resolved_at: Option<NaiveDate>,
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
        ScriptCommand::AddProposal { title, url, budget_request_details, announced_at, published_at, is_historical} => {
            let budget_request_details = budget_request_details.map(|details| {
                BudgetRequestDetails {
                    team: details.team.as_ref().and_then(|name| budget_system.get_team_id_by_name(name)),
                    request_amounts: details.request_amounts,
                    start_date: details.start_date,
                    end_date: details.end_date,
                    payment_status: details.payment_status,
                }
            });
            let proposal_id = budget_system.add_proposal(title.clone(), url, budget_request_details, announced_at, published_at, is_historical)?;
            println!("Added proposal: {} ({})", title, proposal_id);
        },
        ScriptCommand::SetProposalDates { proposal_name, announced_at, published_at, resolved_at } => {
            budget_system.set_proposal_dates(&proposal_name, announced_at, published_at, resolved_at)?;
            println!("Updated dates for proposal: {}", proposal_name);
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
            
            let raffle = budget_system.state.raffles.get(&raffle_id).unwrap();

            println!("Imported predefined raffle for proposal '{}' (Raffle ID: {})", proposal_name, raffle_id);
            println!("  Counted teams: {:?}", counted_teams);
            println!("  Uncounted teams: {:?}", uncounted_teams);
            println!("  Total counted seats: {}", total_counted_seats);
            println!("  Max earner seats: {}", max_earner_seats);

            // Print team snapshots
            println!("\nTeam Snapshots:");
            for snapshot in &raffle.team_snapshots {
                println!("  {} ({}): {:?}", snapshot.name, snapshot.id, snapshot.status);
            }

            // Print raffle result
            if let Some(result) = &raffle.result {
                println!("\nRaffle Result:");
                println!("  Counted teams: {:?}", result.counted);
                println!("  Uncounted teams: {:?}", result.uncounted);
            } else {
                println!("\nRaffle result not available");
            }
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

            let vote = budget_system.state.votes.get(&vote_id).unwrap();
            let proposal = budget_system.state.proposals.get(&vote.proposal_id).unwrap();

            println!("Imported historical vote for proposal '{}' (Vote ID: {})", proposal_name, vote_id);
            println!("Vote passed: {}", passed);

            println!("\nNon-participating teams:");
            for team_name in &non_participating_teams {
                println!("  {}", team_name);
            }

            if let VoteType::Formal { raffle_id, .. } = vote.vote_type {
                if let Some(raffle) = budget_system.state.raffles.get(&raffle_id) {
                    if let VoteParticipation::Formal { counted, uncounted } = &vote.participation {
                        println!("\nCounted seats:");
                        for &team_id in counted {
                            if let Some(team) = raffle.team_snapshots.iter().find(|s| s.id == team_id) {
                                println!("  {} (+{} points)", team.name, Vote::COUNTED_VOTE_POINTS);
                            }
                        }

                        println!("\nUncounted seats:");
                        for &team_id in uncounted {
                            if let Some(team) = raffle.team_snapshots.iter().find(|s| s.id == team_id) {
                                println!("  {} (+{} points)", team.name, Vote::UNCOUNTED_VOTE_POINTS);
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
            for snapshot in &raffle.team_snapshots {
                let tickets: Vec<_> = raffle.tickets.iter()
                    .filter(|t| t.team_id == snapshot.id)
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
                    if let Some(snapshot) = raffle.team_snapshots.iter().find(|s| s.id == team_id) {
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
                }
                println!("Supporter seats:");
                for &team_id in &result.counted {
                    if let Some(snapshot) = raffle.team_snapshots.iter().find(|s| s.id == team_id) {
                        if let TeamStatus::Supporter = snapshot.status {
                            let best_score = raffle.tickets.iter()
                                .filter(|t| t.team_id == team_id)
                                .map(|t| t.score)
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .unwrap_or(0.0);
                            println!("  {} (score: {})", snapshot.name, best_score);
                        }
                    }
                }
                println!("Total counted seats: {} (Earners: {}, Supporters: {})", 
                         result.counted.len(), earner_count, result.counted.len() - earner_count);

                println!("Uncounted seats:");
                println!("Earner seats:");
                for &team_id in &result.uncounted {
                    if let Some(snapshot) = raffle.team_snapshots.iter().find(|s| s.id == team_id) {
                        if let TeamStatus::Earner { .. } = snapshot.status {
                            let best_score = raffle.tickets.iter()
                                .filter(|t| t.team_id == team_id)
                                .map(|t| t.score)
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .unwrap_or(0.0);
                            println!("  {} (score: {})", snapshot.name, best_score);
                        }
                    }
                }
                println!("Supporter seats:");
                for &team_id in &result.uncounted {
                    if let Some(snapshot) = raffle.team_snapshots.iter().find(|s| s.id == team_id) {
                        if let TeamStatus::Supporter = snapshot.status {
                            let best_score = raffle.tickets.iter()
                                .filter(|t| t.team_id == team_id)
                                .map(|t| t.score)
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .unwrap_or(0.0);
                            println!("  {} (score: {})", snapshot.name, best_score);
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
            let (raffle_id, tickets) = budget_system.prepare_raffle(&proposal_name, excluded_teams.clone())?;

            println!("Generated RaffleTickets:");
            for (team_name, start, end) in budget_system.group_tickets_by_team(&tickets) {
                println!("  {} ballot range [{}..{}]", team_name, start, end);
            }

            if let Some(excluded) = excluded_teams {
                println!("Excluded teams: {:?}", excluded);
            }

            let current_block = budget_system.ethereum_service.get_current_block().await?;
            println!("Current block number: {}", current_block);

            let target_block = current_block + block_offset.unwrap_or(budget_system.ethereum_service.future_block_offset);
            println!("Target block for randomness: {}", target_block);

            // Wait for target block
            println!("Waiting for target block...");
            let mut last_observed_block = current_block;
            while budget_system.ethereum_service.get_current_block().await? < target_block {
                tokio::time::sleep(Duration::from_secs(1)).await;
                let new_block = budget_system.ethereum_service.get_current_block().await?;
                if new_block != last_observed_block {
                    println!("Latest observed block: {}", new_block);
                    last_observed_block = new_block;
                }
            }

            // FINALIZATION PHASE
            let randomness = budget_system.ethereum_service.get_randomness(target_block).await?;
            println!("Block randomness: {}", randomness);
            println!("Etherscan URL: https://etherscan.io/block/{}#consensusinfo", target_block);

            let raffle = budget_system.finalize_raffle(raffle_id, target_block, randomness).await?;

            // Print results (similar to ImportHistoricalRaffle)
            println!("Raffle results for proposal '{}' (Raffle ID: {})", proposal_name, raffle_id);

            // Print raffle results
            if let Some(result) = &raffle.result {
                println!("Counted seats:");
                println!("Earner seats:");
                let mut earner_count = 0;
                for &team_id in &result.counted {
                    if let Some(snapshot) = raffle.team_snapshots.iter().find(|s| s.id == team_id) {
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
                }
                println!("Supporter seats:");
                for &team_id in &result.counted {
                    if let Some(snapshot) = raffle.team_snapshots.iter().find(|s| s.id == team_id) {
                        if let TeamStatus::Supporter = snapshot.status {
                            let best_score = raffle.tickets.iter()
                                .filter(|t| t.team_id == team_id)
                                .map(|t| t.score)
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .unwrap_or(0.0);
                            println!("  {} (score: {})", snapshot.name, best_score);
                        }
                    }
                }
                println!("Total counted seats: {} (Earners: {}, Supporters: {})", 
                         result.counted.len(), earner_count, result.counted.len() - earner_count);

                println!("Uncounted seats:");
                println!("Earner seats:");
                for &team_id in &result.uncounted {
                    if let Some(snapshot) = raffle.team_snapshots.iter().find(|s| s.id == team_id) {
                        if let TeamStatus::Earner { .. } = snapshot.status {
                            let best_score = raffle.tickets.iter()
                                .filter(|t| t.team_id == team_id)
                                .map(|t| t.score)
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .unwrap_or(0.0);
                            println!("  {} (score: {})", snapshot.name, best_score);
                        }
                    }
                }
                println!("Supporter seats:");
                for &team_id in &result.uncounted {
                    if let Some(snapshot) = raffle.team_snapshots.iter().find(|s| s.id == team_id) {
                        if let TeamStatus::Supporter = snapshot.status {
                            let best_score = raffle.tickets.iter()
                                .filter(|t| t.team_id == team_id)
                                .map(|t| t.score)
                                .max_by(|a, b| a.partial_cmp(b).unwrap())
                                .unwrap_or(0.0);
                            println!("  {} (score: {})", snapshot.name, best_score);
                        }
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