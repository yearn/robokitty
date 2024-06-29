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
    raffles: HashMap<Uuid, Raffle>
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
struct Raffle {
    id: Uuid,
    proposal_id: Uuid,
    tickets: Vec<RaffleTicket>,
    teams: HashMap<Uuid, RaffleTeam>,
    total_counted_seats: usize,
    max_earner_seats: usize,
    block_randomness: String,
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
        if !self.proposals.contains_key(&proposal_id) {
            return Err("Proposal not found");
        }

        let active_teams: HashMap<Uuid, Team> = self.current_state.teams.iter()
            .filter(|(_, team)| !matches!(team.status, TeamStatus::Inactive))
            .map(|(id, team)| (*id, team.clone()))
            .collect();

        let mut raffle = Raffle::new(
            proposal_id,
            &active_teams,
            excluded_teams,
            block_randomness
        );

        raffle.allocate_tickets()?;
        raffle.generate_scores()?;

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
        if !self.proposals.contains_key(&proposal_id) {
            return Err("Proposal not found");
        }

        if max_earner_seats > total_counted_seats {
            return Err("Earner seats cannot be greater than the total number of seats");
        }

        let active_teams: HashMap<Uuid, Team> = self.current_state.teams.iter()
            .filter(|(_, team)| !matches!(team.status, TeamStatus::Inactive))
            .map(|(id, team)| (*id, team.clone()))
            .collect();

        let mut raffle = Raffle::with_custom_seats(
            proposal_id,
            &active_teams,
            excluded_teams,
            total_counted_seats,
            max_earner_seats,
            block_randomness
        );

        raffle.allocate_tickets()?;
        raffle.generate_scores()?;

        let raffle_id = raffle.id;
        self.raffles.insert(raffle_id, raffle);
        self.save_state();

        Ok(raffle_id)
    }

    fn add_proposal(&mut self, title: String, url: Option<String>, budget_request_details: Option<BudgetRequestDetails>) -> Result<Uuid, &'static str> {
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
        let id = proposal.id;
        self.proposals.insert(id, proposal);
        self.save_state();
        Ok(id)
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


}

impl Raffle {
    const DEFAULT_TOTAL_COUNTED_SEATS: usize = 7;
    const DEFAULT_MAX_EARNER_SEATS: usize = 5;

    // Initiates a Raffle with default seat allocations
    fn new(proposal_id: Uuid, teams: &HashMap<Uuid, Team>, excluded_teams: &[Uuid], block_randomness: String) -> Self {
        Self::with_custom_seats(
            proposal_id,
            teams,
            excluded_teams,
            Self::DEFAULT_TOTAL_COUNTED_SEATS,
            Self::DEFAULT_MAX_EARNER_SEATS,
            block_randomness
        )
    }
    
    // Clones the Teams into Raffle Teams and initiates a Raffle.
    // Supports non-default seat allocations.
    fn with_custom_seats(
        proposal_id: Uuid,
        teams: &HashMap<Uuid, Team>,
        excluded_teams: &[Uuid],
        total_counted_seats: usize,
        max_earner_seats: usize,
        block_randomness: String
    ) -> Self {
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

        Raffle {
            id: Uuid::new_v4(),
            proposal_id,
            tickets: Vec::new(),
            teams: raffle_teams,
            total_counted_seats,
            max_earner_seats,
            block_randomness,
        }
    }

    fn allocate_tickets(&mut self) -> Result<(), &'static str> {
        self.tickets.clear();
        for (id, team) in &self.teams {
            let ticket_count: Result<u64, &'static str> = match &team.status {
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
            };
            
            for _ in 0..ticket_count? {
                self.tickets.push(RaffleTicket {
                    team_id: *id,
                    index: self.tickets.len() as u64,
                    score: 0.0 // Is set in generate_scores
                });
            }
        }
        Ok(())
    }

    fn generate_scores(&mut self) -> Result<(), &'static str> {
        for ticket in &mut self.tickets {
            ticket.score = generate_random_score_from_seed(&self.block_randomness, ticket.index);
        }
        Ok(())
    }

    fn select_teams(&self) -> (HashSet<Uuid>, HashSet<Uuid>) {
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

        let mut counted_voters = HashSet::new();
        let mut uncounted_voters = HashSet::new();

        // Select earner teams for counted seats
        let earner_seats = earner_teams.len().min(self.max_earner_seats);
        counted_voters.extend(earner_teams.iter().take(earner_seats).map(|(id, _)| *id));

        // Fill remaining counted seats with supporter teams
        let supporter_seats = self.total_counted_seats - counted_voters.len();
        counted_voters.extend(supporter_teams.iter().take(supporter_seats).map(|(id, _)| *id));

        // Assign remaining teams to uncounted voters
        uncounted_voters.extend(earner_teams.iter().skip(earner_seats).map(|(id, _)| *id));
        uncounted_voters.extend(supporter_teams.iter().skip(supporter_seats).map(|(id, _)| *id));

        // Add excluded teams to uncounted voters
        uncounted_voters.extend(
            self.teams.iter()
                .filter(|(_, team)| matches!(team.status, RaffleTeamStatus::Excluded))
                .map(|(id, _)| *id)
        );

        (counted_voters, uncounted_voters)
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
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize the BudgetSystem
    let mut system = BudgetSystem::new();

    // Add teams (unchanged)
    let team_a_id = system.add_team("Team A".to_string(), "Alice".to_string(), Some(vec![100000, 120000, 110000]))?;
    let team_b_id = system.add_team("Team B".to_string(), "Bob".to_string(), Some(vec![90000, 95000, 100000]))?;
    let team_c_id = system.add_team("Team C".to_string(), "Charlie".to_string(), None)?;
    let team_d_id = system.add_team("Team D".to_string(), "David".to_string(), Some(vec![150000, 160000, 170000]))?;
    let team_e_id = system.add_team("Team E".to_string(), "Eve".to_string(), None)?;

    println!("Teams added to the system:");
    for (id, team) in &system.current_state.teams {
        println!("- {} ({}): {:?}", team.name, id, team.status);
    }

    // Add a proposal
    let proposal_id = system.add_proposal(
        "Q3 Budget Request".to_string(),
        Some("https://example.com/proposal".to_string()),
        Some(BudgetRequestDetails {
            team: Some(team_a_id),
            request_amount: 500000.0,
            request_token: "USD".to_string(),
            start_date: Some(NaiveDate::from_ymd_opt(2024, 7, 1).unwrap()),
            end_date: Some(NaiveDate::from_ymd_opt(2024, 9, 30).unwrap()),
            payment_status: None,
        })
    )?;

    println!("\nProposal added:");
    println!("{:?}", system.get_proposal(proposal_id).unwrap());

    // Connect to Ethereum node and get randomness (unchanged)
    let provider = Provider::connect_ipc("/tmp/reth.ipc").await?;
    let client = Arc::new(provider);
    let latest_block = client.get_block_number().await?.as_u64();
    println!("\nCurrent block height: {}", latest_block);

    let block_randomness = match client.get_block(latest_block).await? {
        Some(block) => block.mix_hash.map(|h| format!("{:x}", h)).unwrap_or_else(|| "default_randomness".to_string()),
        None => "default_randomness".to_string(),
    };
    println!("Block randomness: {}", block_randomness);

    // Conduct a raffle (unchanged)
    let excluded_teams = vec![team_c_id];
    let raffle_id = system.conduct_raffle(proposal_id, block_randomness, &excluded_teams)?;
    let raffle = system.raffles.get(&raffle_id).unwrap();

    println!("\nRaffle conducted. Results:");
    println!("Total tickets allocated: {}", raffle.tickets.len());

    // Display ticket allocation (unchanged)
    for (id, team) in &raffle.teams {
        let ticket_count = raffle.tickets.iter().filter(|t| t.team_id == *id).count();
        println!("- {} ({}): {} tickets", team.name, id, ticket_count);
    }

    // Select teams (unchanged)
    let (counted_voters, uncounted_voters) = raffle.select_teams();

    println!("\nSelected teams:");
    println!("Counted voters:");
    for id in &counted_voters {
        println!("- {} ({})", raffle.teams[id].name, id);
    }
    println!("Uncounted voters:");
    for id in &uncounted_voters {
        println!("- {} ({})", raffle.teams[id].name, id);
    }

    // Demonstrate new proposal management flow
    println!("\nDemonstrating proposal management flow:");

    // Approve the proposal
    system.approve(proposal_id)?;
    println!("Proposal approved:");
    println!("{:?}", system.get_proposal(proposal_id).unwrap());

    // Try to approve again (should fail)
    match system.approve(proposal_id) {
        Ok(_) => println!("Unexpected: Proposal approved again"),
        Err(e) => println!("Expected error when approving again: {}", e),
    }

    // Mark the proposal as paid
    system.mark_proposal_as_paid(proposal_id)?;
    println!("Proposal marked as paid:");
    println!("{:?}", system.get_proposal(proposal_id).unwrap());

    // Try to retract resolution (should fail because it's paid)
    match system.retract_resolution(proposal_id) {
        Ok(_) => println!("Unexpected: Resolution retracted on paid proposal"),
        Err(e) => println!("Expected error when retracting resolution on paid proposal: {}", e),
    }

    // Close the proposal
    system.close(proposal_id)?;
    println!("Proposal closed:");
    println!("{:?}", system.get_proposal(proposal_id).unwrap());

    // Try to reopen (should work)
    system.reopen(proposal_id)?;
    println!("Proposal reopened:");
    println!("{:?}", system.get_proposal(proposal_id).unwrap());

    // Add a new proposal for rejection demonstration
    let reject_proposal_id = system.add_proposal(
        "Proposal to be rejected".to_string(),
        None,
        None
    )?;

    // Reject the new proposal
    system.reject(reject_proposal_id)?;
    println!("New proposal rejected:");
    println!("{:?}", system.get_proposal(reject_proposal_id).unwrap());

    // Demonstrate close_with_reason
    let another_proposal_id = system.add_proposal(
        "Another proposal".to_string(),
        None,
        None
    )?;
    system.close_with_reason(another_proposal_id, Resolution::Invalid)?;
    println!("Another proposal closed as invalid:");
    println!("{:?}", system.get_proposal(another_proposal_id).unwrap());

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn setup_system() -> BudgetSystem {
        let mut system = BudgetSystem::new();
        system.add_team("Team A".to_string(), "Alice".to_string(), Some(vec![100000])).unwrap();
        system.add_team("Team B".to_string(), "Bob".to_string(), Some(vec![90000])).unwrap();
        system.add_team("Team C".to_string(), "Charlie".to_string(), None).unwrap();
        system
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
        let mut system = setup_system();
        let team_id = *system.current_state.teams.keys().next().unwrap();
        system.deactivate_team(team_id).unwrap();
        assert!(matches!(system.current_state.teams[&team_id].status, TeamStatus::Inactive));
    }

    #[test]
    fn test_reactivate_team() {
        let mut system = setup_system();
        let team_id = *system.current_state.teams.keys().next().unwrap();
        system.deactivate_team(team_id).unwrap();
        system.reactivate_team(team_id).unwrap();
        assert!(matches!(system.current_state.teams[&team_id].status, TeamStatus::Supporter));
    }

    #[test]
    fn test_conduct_raffle() {
        let mut system = setup_system();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        let raffle_id = system.conduct_raffle(proposal_id, "test_randomness".to_string(), &[]).unwrap();
        let raffle = system.raffles.get(&raffle_id).unwrap();
        assert_eq!(raffle.teams.len(), 3);
    }

    #[test]
    fn test_raffle_ignores_inactive_team() {
        let mut system = setup_system();
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
        let mut system = setup_system();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        let raffle_id = system.conduct_raffle_with_custom_seats(proposal_id, 9, 6, "test_randomness".to_string(), &[]).unwrap();
        let raffle = system.raffles.get(&raffle_id).unwrap();
        assert_eq!(raffle.teams.len(), 3);
        assert_eq!(raffle.total_counted_seats, 9);
        assert_eq!(raffle.max_earner_seats, 6);
    }

    #[test]
    fn test_raffle_with_custom_seats_ignores_inactive_team() {
        let mut system = setup_system();
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
        let mut system = setup_system();
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
        let mut system = setup_system();
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

    fn setup_test_teams() -> HashMap<Uuid, Team> {
        let mut teams = HashMap::new();
        let team_a = Team::new("Team A".to_string(), "Alice".to_string(), Some(vec![100000, 120000, 110000])).unwrap();
        let team_b = Team::new("Team B".to_string(), "Bob".to_string(), Some(vec![90000, 95000, 100000])).unwrap();
        let team_c = Team::new("Team C".to_string(), "Charlie".to_string(), None).unwrap();
        let team_d = Team::new("Team D".to_string(), "David".to_string(), Some(vec![150000, 160000, 170000])).unwrap();
        let team_e = Team::new("Team E".to_string(), "Eve".to_string(), None).unwrap();
        teams.insert(team_a.id, team_a);
        teams.insert(team_b.id, team_b);
        teams.insert(team_c.id, team_c);
        teams.insert(team_d.id, team_d);
        teams.insert(team_e.id, team_e);
        teams
    }

    #[test]
    fn test_raffle_creation() {
        let teams = setup_test_teams();
        let raffle = Raffle::new(Uuid::new_v4(), &teams, &[], "test_randomness".to_string());
        assert_eq!(raffle.teams.len(), 5);
        assert_eq!(raffle.total_counted_seats, Raffle::DEFAULT_TOTAL_COUNTED_SEATS);
        assert_eq!(raffle.max_earner_seats, Raffle::DEFAULT_MAX_EARNER_SEATS);
    }

    #[test]
    fn test_raffle_with_custom_seats() {
        let teams = setup_test_teams();
        let raffle = Raffle::with_custom_seats(Uuid::new_v4(), &teams, &[], 9, 6, "test_randomness".to_string());
        assert_eq!(raffle.total_counted_seats, 9);
        assert_eq!(raffle.max_earner_seats, 6);
    }

    #[test]
    fn test_raffle_with_excluded_teams() {
        let teams = setup_test_teams();
        let excluded_teams: Vec<Uuid> = teams.values()
            .filter(|t| t.name == "Team C" || t.name == "Team E")
            .map(|t| t.id)
            .collect();
        let raffle = Raffle::new(Uuid::new_v4(), &teams, &excluded_teams, "test_randomness".to_string());
        assert_eq!(raffle.teams.len(), 5);
        assert!(raffle.teams.values().any(|t| t.name == "Team C" && matches!(t.status, RaffleTeamStatus::Excluded)));
        assert!(raffle.teams.values().any(|t| t.name == "Team E" && matches!(t.status, RaffleTeamStatus::Excluded)));
    }

    #[test]
    fn test_ticket_allocation() {
        let teams = setup_test_teams();
        let mut raffle = Raffle::new(Uuid::new_v4(), &teams, &[], "test_randomness".to_string());
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
        let teams = setup_test_teams();
        let mut raffle = Raffle::new(Uuid::new_v4(), &teams, &[], "test_randomness".to_string());
        raffle.allocate_tickets().unwrap();
        raffle.generate_scores().unwrap();
        
        for ticket in &raffle.tickets {
            assert!(ticket.score > 0.0 && ticket.score < 1.0);
        }
    }

    #[test]
    fn test_team_selection() {
        let teams = setup_test_teams();
        let mut raffle = Raffle::new(Uuid::new_v4(), &teams, &[], "test_randomness".to_string());
        raffle.allocate_tickets().unwrap();
        raffle.generate_scores().unwrap();
        let (counted_voters, uncounted_voters) = raffle.select_teams();
        
        assert_eq!(counted_voters.len() + uncounted_voters.len(), teams.len());
        assert_eq!(counted_voters.len(), Raffle::DEFAULT_TOTAL_COUNTED_SEATS);
        assert!(counted_voters.len() <= Raffle::DEFAULT_MAX_EARNER_SEATS + 2); // Max earners + min 2 supporters
    }

    #[test]
    fn test_raffle_with_fewer_teams_than_seats() {
        let mut teams = HashMap::new();
        let team_a = Team::new("Team A".to_string(), "Alice".to_string(), Some(vec![100000])).unwrap();
        let team_b = Team::new("Team B".to_string(), "Bob".to_string(), None).unwrap();
        teams.insert(team_a.id, team_a);
        teams.insert(team_b.id, team_b);
        
        let mut raffle = Raffle::new(Uuid::new_v4(), &teams, &[], "test_randomness".to_string());
        raffle.allocate_tickets().unwrap();
        raffle.generate_scores().unwrap();
        let (counted_voters, uncounted_voters) = raffle.select_teams();
        
        assert_eq!(counted_voters.len() + uncounted_voters.len(), teams.len());
        assert_eq!(counted_voters.len(), teams.len());
        assert_eq!(uncounted_voters.len(), 0);
    }

    #[test]
    fn test_raffle_with_all_excluded_teams() {
        let teams = setup_test_teams();
        let excluded_teams: Vec<Uuid> = teams.keys().cloned().collect();
        let mut raffle = Raffle::new(Uuid::new_v4(), &teams, &excluded_teams, "test_randomness".to_string());
        raffle.allocate_tickets().unwrap();
        raffle.generate_scores().unwrap();
        let (counted_voters, uncounted_voters) = raffle.select_teams();
        
        assert_eq!(counted_voters.len(), 0);
        assert_eq!(uncounted_voters.len(), teams.len());
    }
    
    #[test]
    fn test_add_proposal() {
        let mut system = setup_system();
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
        let mut system = setup_system();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        
        system.approve(proposal_id).unwrap();
        let proposal = system.get_proposal(proposal_id).unwrap();
        assert_eq!(proposal.resolution, Some(Resolution::Approved));
        assert_eq!(proposal.status, ProposalStatus::Open);
    }

    #[test]
    fn test_reject_proposal() {
        let mut system = setup_system();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        
        system.reject(proposal_id).unwrap();
        let proposal = system.get_proposal(proposal_id).unwrap();
        assert_eq!(proposal.resolution, Some(Resolution::Rejected));
        assert_eq!(proposal.status, ProposalStatus::Closed);
    }

    #[test]
    fn test_close_proposal() {
        let mut system = setup_system();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        
        system.close(proposal_id).unwrap();
        let proposal = system.get_proposal(proposal_id).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Closed);
    }

    #[test]
    fn test_reopen_proposal() {
        let mut system = setup_system();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        
        system.close(proposal_id).unwrap();
        system.reopen(proposal_id).unwrap();
        let proposal = system.get_proposal(proposal_id).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Reopened);
        assert!(proposal.resolution.is_none());
    }

    #[test]
    fn test_retract_resolution() {
        let mut system = setup_system();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        
        system.approve(proposal_id).unwrap();
        system.retract_resolution(proposal_id).unwrap();
        let proposal = system.get_proposal(proposal_id).unwrap();
        assert!(proposal.resolution.is_none());
    }

    #[test]
    fn test_mark_proposal_as_paid() {
        let mut system = setup_system();
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
        let mut system = setup_system();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        
        system.approve(proposal_id).unwrap();
        assert!(system.approve(proposal_id).is_err());
    }

    #[test]
    fn test_cannot_reject_paid_proposal() {
        let mut system = setup_system();
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
        let mut system = setup_system();
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
        let mut system = setup_system();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        
        assert!(system.reopen(proposal_id).is_err());
    }

    #[test]
    fn test_full_proposal_lifecycle() {
        let mut system = setup_system();
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

    // New tests

    #[test]
    fn test_close_with_reason() {
        let mut system = setup_system();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        
        system.close_with_reason(proposal_id, Resolution::Invalid).unwrap();
        let proposal = system.get_proposal(proposal_id).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Closed);
        assert_eq!(proposal.resolution, Some(Resolution::Invalid));
    }

    #[test]
    fn test_cannot_close_with_reason_already_closed_proposal() {
        let mut system = setup_system();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        
        system.close(proposal_id).unwrap();
        assert!(system.close_with_reason(proposal_id, Resolution::Duplicate).is_err());
    }

    #[test]
    fn test_cannot_close_with_reason_paid_proposal() {
        let mut system = setup_system();
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
        let mut system = setup_system();
        let proposal_id = system.add_proposal("Test Proposal".to_string(), None, None).unwrap();
        
        system.close_with_reason(proposal_id, Resolution::Invalid).unwrap();
        system.reopen(proposal_id).unwrap();
        let proposal = system.get_proposal(proposal_id).unwrap();
        assert_eq!(proposal.status, ProposalStatus::Reopened);
        assert!(proposal.resolution.is_none());
    }

    #[test]
    fn test_cannot_approve_non_existent_proposal() {
        let mut system = setup_system();
        let non_existent_id = Uuid::new_v4();
        assert!(system.approve(non_existent_id).is_err());
    }

    #[test]
    fn test_cannot_reject_non_existent_proposal() {
        let mut system = setup_system();
        let non_existent_id = Uuid::new_v4();
        assert!(system.reject(non_existent_id).is_err());
    }

    #[test]
    fn test_cannot_close_non_existent_proposal() {
        let mut system = setup_system();
        let non_existent_id = Uuid::new_v4();
        assert!(system.close(non_existent_id).is_err());
    }

    #[test]
    fn test_cannot_reopen_non_existent_proposal() {
        let mut system = setup_system();
        let non_existent_id = Uuid::new_v4();
        assert!(system.reopen(non_existent_id).is_err());
    }

    #[test]
    fn test_cannot_retract_resolution_non_existent_proposal() {
        let mut system = setup_system();
        let non_existent_id = Uuid::new_v4();
        assert!(system.retract_resolution(non_existent_id).is_err());
    }

    #[test]
    fn test_cannot_mark_as_paid_non_existent_proposal() {
        let mut system = setup_system();
        let non_existent_id = Uuid::new_v4();
        assert!(system.mark_proposal_as_paid(non_existent_id).is_err());
    }

    #[test]
    fn test_cannot_close_with_reason_non_existent_proposal() {
        let mut system = setup_system();
        let non_existent_id = Uuid::new_v4();
        assert!(system.close_with_reason(non_existent_id, Resolution::Invalid).is_err());
    }

}