use chrono::{DateTime, Utc};
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


// TODO: Change rev to a float and do k in order to match original imp

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
enum TeamStatus {
    Earner { trailing_monthly_revenue: Vec<u64>},
    Supporter,
}

#[derive(Clone, Serialize, Deserialize)]
struct Team {
    name: String,
    representative: String,
    status: TeamStatus
}

#[derive(Clone, Serialize, Deserialize)]
struct SystemState {
    teams: HashMap<String, Team>,
    timestamp: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
struct BudgetSystem {
    current_state: SystemState,
    history: Vec<SystemState>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
enum RaffleTeamStatus {
    Earner { trailing_monthly_revenue: Vec<u64> },
    Supporter,
    Excluded, // For teams with conflict of interest in a particular Vote
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RaffleTeam {
    name: String,
    status: RaffleTeamStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RaffleTicket {
    team_name: String,
    index: u64,
    score: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Raffle {
    tickets: Vec<RaffleTicket>,
    teams: HashMap<String, RaffleTeam>,
    total_counted_seats: usize,
    max_earner_seats: usize,
    block_randomness: String,
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
            name,
            representative,
            status
        })
    }

    fn get_revenue_data(&self) -> Option<&Vec<u64>> {
        match &self.status {
            TeamStatus::Earner { trailing_monthly_revenue } => Some(trailing_monthly_revenue),
            TeamStatus::Supporter => None,
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


}


impl BudgetSystem {
    fn new() -> Self {
        BudgetSystem {
            current_state: SystemState {
                teams: HashMap::new(),
                timestamp: Utc::now(),
            },
            history: Vec::new(),
        }

    }

    fn add_team(&mut self, name: String, representative: String, trailing_monthly_revenue: Option<Vec<u64>>) -> Result<(), &'static str> {
        if self.current_state.teams.contains_key(&name) {
            return Err("A team with this name already exists");
        }
        let team = Team::new(name.clone(), representative, trailing_monthly_revenue)?;
        self.current_state.teams.insert(name, team);
        self.save_state();
        Ok(())
    }

    fn remove_team(&mut self, team_name: &str) -> Result<(), &'static str> {
        if self.current_state.teams.remove(team_name).is_some() {
            self.save_state();
            Ok(())
        } else {
            Err("Team not found")
        }
    }

    fn update_team_status(&mut self, team_name: &str, new_status: TeamStatus) -> Result<(), &'static str> {
        match self.current_state.teams.get_mut(team_name) {
            Some(team) => {
                team.change_status(new_status);
                self.save_state();
                Ok(())
            },
            None => Err("Team not found"),
        }
    }

    fn update_team_representative(&mut self, team_name: &str, new_representative: String) -> Result<(), &'static str> {
        match self.current_state.teams.get_mut(team_name) {
            Some(team) => {
                team.representative = new_representative;
                self.save_state();
                Ok(())
            },
            None => Err("Team not found"),
        }
    }

    fn update_team_revenue(&mut self, team_name: &str, new_revenue: Vec<u64>) -> Result<(), &'static str> {
        match self.current_state.teams.get_mut(team_name) {
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

    fn conduct_raffle(&self, block_randomness: String, excluded_teams: &[String]) -> Result<Raffle, &'static str> {
        self.conduct_raffle_with_seats(Raffle::DEFAULT_TOTAL_COUNTED_SEATS, Raffle::DEFAULT_MAX_EARNER_SEATS, block_randomness, excluded_teams)
    }

    fn conduct_raffle_with_seats(&self, total_counted_seats: usize, max_earner_seats: usize, block_randomness: String, excluded_teams: &[String]) -> Result<Raffle, &'static str> {
        if max_earner_seats > total_counted_seats {
            return Err("Earner seats cannot be greater than the total number of seats");
        }
        let mut raffle = Raffle::with_seats(&self.current_state.teams, excluded_teams, total_counted_seats, max_earner_seats, block_randomness);
        raffle.allocate_tickets()?;
        raffle.generate_scores()?;
        Ok(raffle)
    }

}

impl Raffle {
    const DEFAULT_TOTAL_COUNTED_SEATS: usize = 7;
    const DEFAULT_MAX_EARNER_SEATS: usize = 5;

    // Initiates a Raffle with default seat allocations
    fn new(teams: &HashMap<String, Team>, excluded_teams: &[String], block_randomness: String) -> Self {
        Self::with_seats(teams, excluded_teams, Self::DEFAULT_TOTAL_COUNTED_SEATS, Self::DEFAULT_MAX_EARNER_SEATS, block_randomness)
    }
    
    // Clones the Teams into Raffle Teams and initiates a Raffle.
    // Supports non-default seat allocations.
    fn with_seats(teams: &HashMap<String, Team>, excluded_teams: &[String], total_counted_seats: usize, max_earner_seats: usize, block_randomness: String) -> Self {
        let raffle_teams = teams.iter().map(|(name, team)| {
            let status = if excluded_teams.contains(name) {
                RaffleTeamStatus::Excluded
            } else {
                match &team.status {
                    TeamStatus::Earner { trailing_monthly_revenue } => 
                        RaffleTeamStatus::Earner { trailing_monthly_revenue: trailing_monthly_revenue.clone() },
                    TeamStatus::Supporter => RaffleTeamStatus::Supporter,
                }
            };
            (name.clone(), RaffleTeam { name: name.clone(), status})
        }).collect();

        Raffle {
            tickets: Vec::new(),
            teams: raffle_teams,
            total_counted_seats,
            max_earner_seats,
            block_randomness,
        }
    }

    fn allocate_tickets(&mut self) -> Result<(), &'static str> {
        self.tickets.clear();
        for (name, team) in &self.teams {
            let ticket_count: Result<u64, &'static str> = match &team.status {
                RaffleTeamStatus::Earner { trailing_monthly_revenue } => {
                    if trailing_monthly_revenue.len() > 3 { 
                        return Err("Trailing monthly revenue cannot exceed 3 entries");
                    }
    
                    let sum: u64 = trailing_monthly_revenue.iter().sum();
                    let quarterly_average = sum as f64 / 3.0;
                    let ticket_count = quarterly_average.sqrt().floor() as u64;
    
                    Ok(ticket_count.max(1))
                },
                RaffleTeamStatus::Supporter => Ok(1),
                RaffleTeamStatus::Excluded => Ok(0),
            };
            
            for _ in 0..ticket_count? {
                self.tickets.push(RaffleTicket {
                    team_name: name.clone(),
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

    fn select_teams(&self) -> (HashSet<String>, HashSet<String>) {
        let mut earner_teams: Vec<_> = self.tickets.iter()
            .filter(|ticket| matches!(self.teams[&ticket.team_name].status, RaffleTeamStatus::Earner { .. }))
            .map(|ticket| (&ticket.team_name, ticket.score))
            .collect();
        earner_teams.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        earner_teams.dedup_by(|a, b| a.0 == b.0);

        let mut supporter_teams: Vec<_> = self.tickets.iter()
        .filter(|ticket| matches!(self.teams[&ticket.team_name].status, RaffleTeamStatus::Supporter))
        .map(|ticket| (&ticket.team_name, ticket.score))
        .collect();
        supporter_teams.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        supporter_teams.dedup_by(|a, b| a.0 == b.0);

        let mut counted_voters = HashSet::new();
        let mut uncounted_voters = HashSet::new();

        // Select earner teams for counted seats
        let earner_seats = earner_teams.len().min(self.max_earner_seats);
        counted_voters.extend(earner_teams.iter().take(earner_seats).map(|(name, _)| (*name).to_string()));

        // Fill remaining counted seats with supporter teams
        let supporter_seats = self.total_counted_seats - counted_voters.len();
        counted_voters.extend(supporter_teams.iter().take(supporter_seats).map(|(name, _)| (*name).to_string()));

        // Assign remaining teams to uncounted voters
        uncounted_voters.extend(earner_teams.iter().skip(earner_seats).map(|(name, _)| (*name).to_string()));
        uncounted_voters.extend(supporter_teams.iter().skip(supporter_seats).map(|(name, _)| (*name).to_string()));

        // Add excluded teams to uncounted voters
        uncounted_voters.extend(
            self.teams.iter()
                .filter(|(_, team)| matches!(team.status, RaffleTeamStatus::Excluded))
                .map(|(name, _)| name.clone())
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize the BudgetSystem
    let mut system = BudgetSystem::new();

    // Add teams
    system.add_team("Team A".to_string(), "Alice".to_string(), Some(vec![100000, 120000, 110000]))?;
    system.add_team("Team B".to_string(), "Bob".to_string(), Some(vec![90000, 95000, 100000]))?;
    system.add_team("Team C".to_string(), "Charlie".to_string(), None)?;
    system.add_team("Team D".to_string(), "David".to_string(), Some(vec![150000, 160000, 170000]))?;
    system.add_team("Team E".to_string(), "Eve".to_string(), None)?;

    println!("Teams added to the system:");
    for (name, team) in &system.current_state.teams {
        println!("- {}: {:?}", name, team.status);
    }

    // Connect to Ethereum node and get randomness
    let provider = Provider::connect_ipc("/tmp/reth.ipc").await?;
    let client = Arc::new(provider);
    let latest_block = client.get_block_number().await?.as_u64();
    println!("\nCurrent block height: {}", latest_block);

    let block_randomness = match client.get_block(latest_block).await? {
        Some(block) => block.mix_hash.map(|h| format!("{:x}", h)).unwrap_or_else(|| "default_randomness".to_string()),
        None => "default_randomness".to_string(),
    };
    println!("Block randomness: {}", block_randomness);

    // Conduct a raffle
    let excluded_teams = vec!["Team C".to_string()]; // Exclude Team C for this raffle
    let raffle = system.conduct_raffle(block_randomness, &excluded_teams)?;

    println!("\nRaffle conducted. Results:");
    println!("Total tickets allocated: {}", raffle.tickets.len());

    // Display ticket allocation
    for (team_name, team) in &raffle.teams {
        let ticket_count = raffle.tickets.iter().filter(|t| t.team_name == *team_name).count();
        println!("- {}: {} tickets", team_name, ticket_count);
    }

    // Select teams
    let (counted_voters, uncounted_voters) = raffle.select_teams();

    println!("\nSelected teams:");
    println!("Counted voters:");
    for team in &counted_voters {
        println!("- {}", team);
    }
    println!("Uncounted voters:");
    for team in &uncounted_voters {
        println!("- {}", team);
    }

    // Verify results
    println!("\nVerification:");
    println!("Total counted voters: {} (should be {})", counted_voters.len(), raffle.total_counted_seats);
    println!("Max earner seats: {}", raffle.max_earner_seats);
    println!("Excluded team (Team C) in uncounted voters: {}", uncounted_voters.contains("Team C"));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_team() {
        let mut system = BudgetSystem::new();
        let result = system.add_team("Team A".to_string(), "Alice".to_string(), None);
        assert!(result.is_ok());
        assert_eq!(system.current_state.teams.len(), 1);
    }

    #[test]
    fn test_add_team_with_revenue() {
        let mut system = BudgetSystem::new();
        let result = system.add_team("Team A".to_string(), "Alice".to_string(), Some(vec![100000]));
        assert!(result.is_ok());
        assert_eq!(system.current_state.teams.len(), 1);
        assert!(matches!(system.current_state.teams["Team A"].status, TeamStatus::Earner { .. }));
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
        system.add_team("Team A".to_string(), "Alice".to_string(), None).unwrap();
        let result = system.remove_team("Team A");
        assert!(result.is_ok());
        assert_eq!(system.current_state.teams.len(), 0);
    }

    #[test]
    fn test_update_team_status() {
        let mut system = BudgetSystem::new();
        system.add_team("Team A".to_string(), "Alice".to_string(), None).unwrap();
        let result = system.update_team_status("Team A", TeamStatus::Earner { trailing_monthly_revenue: vec![100000] });
        assert!(result.is_ok());
        assert!(matches!(system.current_state.teams["Team A"].status, TeamStatus::Earner { .. }));
    }

    #[test]
    fn test_update_team_revenue() {
        let mut system = BudgetSystem::new();
        system.add_team("Team A".to_string(), "Alice".to_string(), Some(vec![100000])).unwrap();
        let result = system.update_team_revenue("Team A", vec![120000]);
        assert!(result.is_ok());
        if let TeamStatus::Earner { trailing_monthly_revenue } = &system.current_state.teams["Team A"].status {
            assert_eq!(trailing_monthly_revenue, &vec![100000, 120000]);
        } else {
            panic!("Team A should be an Earner");
        }
    }

    fn setup_test_teams() -> HashMap<String, Team> {
        let mut teams = HashMap::new();
        teams.insert("Team A".to_string(), Team::new("Team A".to_string(), "Alice".to_string(), Some(vec![100000, 120000, 110000])).unwrap());
        teams.insert("Team B".to_string(), Team::new("Team B".to_string(), "Bob".to_string(), Some(vec![90000, 95000, 100000])).unwrap());
        teams.insert("Team C".to_string(), Team::new("Team C".to_string(), "Charlie".to_string(), None).unwrap());
        teams.insert("Team D".to_string(), Team::new("Team D".to_string(), "David".to_string(), Some(vec![150000, 160000, 170000])).unwrap());
        teams.insert("Team E".to_string(), Team::new("Team E".to_string(), "Eve".to_string(), None).unwrap());
        teams
    }

    #[test]
    fn test_raffle_creation() {
        let teams = setup_test_teams();
        let raffle = Raffle::new(&teams, &[], "test_randomness".to_string());
        assert_eq!(raffle.teams.len(), 5);
        assert_eq!(raffle.total_counted_seats, Raffle::DEFAULT_TOTAL_COUNTED_SEATS);
        assert_eq!(raffle.max_earner_seats, Raffle::DEFAULT_MAX_EARNER_SEATS);
    }

    #[test]
    fn test_raffle_with_excluded_teams() {
        let teams = setup_test_teams();
        let excluded_teams = vec!["Team C".to_string(), "Team E".to_string()];
        let raffle = Raffle::new(&teams, &excluded_teams, "test_randomness".to_string());
        assert_eq!(raffle.teams.len(), 5);
        assert!(matches!(raffle.teams["Team C"].status, RaffleTeamStatus::Excluded));
        assert!(matches!(raffle.teams["Team E"].status, RaffleTeamStatus::Excluded));
    }

    #[test]
    fn test_ticket_allocation() {
        let teams = setup_test_teams();
        let mut raffle = Raffle::new(&teams, &[], "test_randomness".to_string());
        raffle.allocate_tickets().unwrap();
        
        // Check if earner teams have more than 1 ticket
        assert!(raffle.tickets.iter().filter(|t| t.team_name == "Team A").count() > 1);
        assert!(raffle.tickets.iter().filter(|t| t.team_name == "Team B").count() > 1);
        assert!(raffle.tickets.iter().filter(|t| t.team_name == "Team D").count() > 1);
        
        // Check if supporter teams have exactly 1 ticket
        assert_eq!(raffle.tickets.iter().filter(|t| t.team_name == "Team C").count(), 1);
        assert_eq!(raffle.tickets.iter().filter(|t| t.team_name == "Team E").count(), 1);
    }

    #[test]
    fn test_score_generation() {
        let teams = setup_test_teams();
        let mut raffle = Raffle::new(&teams, &[], "test_randomness".to_string());
        raffle.allocate_tickets().unwrap();
        raffle.generate_scores().unwrap();
        
        for ticket in &raffle.tickets {
            assert!(ticket.score > 0.0 && ticket.score < 1.0);
        }
    }

    #[test]
    fn test_team_selection() {
        let teams = setup_test_teams();
        let mut raffle = Raffle::new(&teams, &[], "test_randomness".to_string());
        raffle.allocate_tickets().unwrap();
        raffle.generate_scores().unwrap();
        let (counted_voters, uncounted_voters) = raffle.select_teams();
        
        assert_eq!(counted_voters.len() + uncounted_voters.len(), teams.len());
        assert_eq!(counted_voters.len(), Raffle::DEFAULT_TOTAL_COUNTED_SEATS);
        assert!(counted_voters.len() <= Raffle::DEFAULT_MAX_EARNER_SEATS + 2); // Max earners + min 2 supporters
    }

    #[test]
    fn test_raffle_with_custom_seats() {
        let teams = setup_test_teams();
        let raffle = Raffle::with_seats(&teams, &[], 9, 6, "test_randomness".to_string());
        assert_eq!(raffle.total_counted_seats, 9);
        assert_eq!(raffle.max_earner_seats, 6);
    }

    #[test]
    fn test_raffle_with_fewer_teams_than_seats() {
        let mut teams = HashMap::new();
        teams.insert("Team A".to_string(), Team::new("Team A".to_string(), "Alice".to_string(), Some(vec![100000])).unwrap());
        teams.insert("Team B".to_string(), Team::new("Team B".to_string(), "Bob".to_string(), None).unwrap());
        
        let mut raffle = Raffle::new(&teams, &[], "test_randomness".to_string());
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
        let excluded_teams: Vec<String> = teams.keys().cloned().collect();
        let mut raffle = Raffle::new(&teams, &excluded_teams, "test_randomness".to_string());
        raffle.allocate_tickets().unwrap();
        raffle.generate_scores().unwrap();
        let (counted_voters, uncounted_voters) = raffle.select_teams();
        
        assert_eq!(counted_voters.len(), 0);
        assert_eq!(uncounted_voters.len(), teams.len());
    }
}