use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};

use super::team::{Team, TeamStatus};
use crate::AppConfig;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Raffle {
    pub id: Uuid,
    pub config: RaffleConfig,
    pub team_snapshots: Vec<TeamSnapshot>,
    pub tickets: Vec<RaffleTicket>,
    pub result: Option<RaffleResult>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RaffleConfig {
    pub proposal_id: Uuid,
    pub epoch_id: Uuid,
    pub initiation_block: u64,
    pub randomness_block: u64,
    pub block_randomness: String,
    pub total_counted_seats: usize,
    pub max_earner_seats: usize,
    pub excluded_teams: Vec<Uuid>,
    pub custom_allocation: Option<HashMap<Uuid, u64>>,
    pub custom_team_order: Option<Vec<Uuid>>,
    pub is_historical: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TeamSnapshot {
    pub id: Uuid,
    pub name: String,
    pub representative: String,
    pub status: TeamStatus,
    pub snapshot_time: DateTime<Utc>,
    pub raffle_status: RaffleParticipationStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RaffleTicket {
    pub team_id: Uuid,
    pub index: u64,
    pub score: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RaffleBuilder {
    pub config: RaffleConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RaffleResult {
    pub counted: Vec<Uuid>,
    pub uncounted: Vec<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RaffleParticipationStatus {
    Included,
    Excluded,
}

impl RaffleBuilder {
    fn new(proposal_id: Uuid, epoch_id: Uuid, app_config: &AppConfig) -> Self {
        RaffleBuilder {
            config: RaffleConfig {
                proposal_id,
                epoch_id,
                initiation_block: 0,
                randomness_block: 0,
                block_randomness: String::new(),
                total_counted_seats: app_config.default_total_counted_seats,
                max_earner_seats: app_config.default_max_earner_seats,
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
    pub fn new(config: RaffleConfig, teams: &HashMap<Uuid, Team>) -> Result<Self, &'static str> {
        let mut team_snapshots = Vec::new();
        let mut tickets = Vec::new();

        // Create team snapshots
        let mut active_teams: Vec<_> = teams.values()
            .filter(|team| team.is_active())
            .collect();

        // Sort teams based on custom order or by name
        if let Some(custom_order) = &config.custom_team_order {
            active_teams.sort_by_key(|team| custom_order.iter().position(|&id| id == team.id()).unwrap_or(usize::MAX));
        } else {
            active_teams.sort_by(|a, b| a.name().cmp(&b.name()));
        }

        // Create snapshots and tickets
        for team in active_teams {
            let snapshot = TeamSnapshot {
                id: team.id(),
                name: team.name().to_string().clone(),
                representative: team.representative().to_string().clone(),
                status: team.status().clone(),
                snapshot_time: Utc::now(),
                raffle_status: if config.excluded_teams.contains(&team.id()) {
                    RaffleParticipationStatus::Excluded
                } else {
                    RaffleParticipationStatus::Included
                },
            };
            team_snapshots.push(snapshot);

            let ticket_count = match &team.status() {
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
                tickets.push(RaffleTicket::new(team.id(), tickets.len() as u64));
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

    pub fn generate_scores(&mut self) -> Result<(), &'static str> {
        for ticket in &mut self.tickets {
            if !self.config.excluded_teams.contains(&ticket.team_id) {
                ticket.score = Self::generate_random_score_from_seed(&self.config.block_randomness, ticket.index);
            }
            // Excluded teams keep their score as 0.0
        }
        Ok(())
    }

    pub fn select_teams(&mut self) {
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

    pub fn generate_random_score_from_seed(randomness: &str, index: u64) -> f64 {
        let combined_seed = format!("{}_{}", randomness, index);
        let mut hasher = Sha256::new();

        hasher.update(combined_seed.as_bytes());
        let result = hasher.finalize();

        let hash_num = u64::from_be_bytes(result[..8].try_into().unwrap());
        let max_num = u64::MAX as f64;
        hash_num as f64 / max_num
    }

    pub fn get_deciding_teams(&self) -> Vec<Uuid> {
        self.result.as_ref()
            .map(|result| result.counted.clone())
            .unwrap_or_default()
    }

    pub fn get_etherscan_url(&self) -> String {
        format!("https://etherscan.io/block/{}#consensusinfo", self.config.randomness_block)
    }
}

impl RaffleTicket {
    pub fn new(team_id: Uuid, index: u64) -> Self {
        RaffleTicket {
            team_id,
            index,
            score: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use uuid::Uuid;
    use chrono::Utc;

    // Helper function to create a mock team
    fn create_mock_team(name: &str, status: TeamStatus) -> Team {
        Team::new(name.to_string(), "Representative".to_string(), match status {
            TeamStatus::Earner { .. } => Some(vec![1000, 2000, 3000]),
            _ => None,
        }).unwrap()
    }

    // Helper function to create 9 mock teams (5 earners, 4 supporters)
    fn create_mock_teams() -> HashMap<Uuid, Team> {
        let mut teams = HashMap::new();
        for i in 1..=5 {
            teams.insert(Uuid::new_v4(), create_mock_team(&format!("Earner{}", i), TeamStatus::Earner { trailing_monthly_revenue: vec![1000 * i as u64, 2000 * i as u64, 3000 * i as u64] }));
        }
        for i in 1..=4 {
            teams.insert(Uuid::new_v4(), create_mock_team(&format!("Supporter{}", i), TeamStatus::Supporter));
        }
        teams
    }

    #[test]
    fn test_raffle_builder() {
        let proposal_id = Uuid::new_v4();
        let epoch_id = Uuid::new_v4();
        let builder = RaffleBuilder::new(proposal_id, epoch_id, &AppConfig::default())
            .with_seats(7, 5)
            .with_randomness(100, 110, "test_randomness".to_string())
            .with_excluded_teams(vec![Uuid::new_v4()])
            .with_custom_allocation(HashMap::new())
            .with_custom_team_order(vec![Uuid::new_v4()]);

        assert_eq!(builder.config.proposal_id, proposal_id);
        assert_eq!(builder.config.epoch_id, epoch_id);
        assert_eq!(builder.config.total_counted_seats, 7);
        assert_eq!(builder.config.max_earner_seats, 5);
        assert_eq!(builder.config.initiation_block, 100);
        assert_eq!(builder.config.randomness_block, 110);
        assert_eq!(builder.config.block_randomness, "test_randomness");
        assert_eq!(builder.config.excluded_teams.len(), 1);
        assert!(builder.config.custom_allocation.is_some());
        assert!(builder.config.custom_team_order.is_some());
    }

    #[test]
    fn test_raffle_creation() {
        let teams = create_mock_teams();
        let config = create_test_config();

        let raffle = Raffle::new(config, &teams).unwrap();

        assert_eq!(raffle.team_snapshots.len(), 9);
        assert!(raffle.tickets.len() >= 9); // At least 1 ticket per team, more for earners
    }

    #[test]
    fn test_generate_scores() {
        let mut raffle = create_test_raffle();
        raffle.generate_scores().unwrap();

        for ticket in &raffle.tickets {
            assert!(ticket.score > 0.0 && ticket.score <= 1.0);
        }
    }

    #[test]
    fn test_select_teams() {
        let mut raffle = create_test_raffle();
        raffle.generate_scores().unwrap();
        raffle.select_teams();

        assert!(raffle.result.is_some());
        let result = raffle.result.as_ref().unwrap();
        assert_eq!(result.counted.len(), 7); // Based on total_counted_seats
        assert_eq!(result.uncounted.len(), 2); // The remaining teams
    }

    #[test]
    fn test_max_earner_seats() {
        let mut raffle = create_test_raffle();
        raffle.generate_scores().unwrap();
        raffle.select_teams();

        let result = raffle.result.as_ref().unwrap();
        let counted_earners = result.counted.iter()
            .filter(|&team_id| raffle.team_snapshots.iter().any(|s| s.id == *team_id && matches!(s.status, TeamStatus::Earner { .. })))
            .count();

        assert!(counted_earners <= 5); // max_earner_seats
    }

    #[test]
    fn test_get_deciding_teams() {
        let mut raffle = create_test_raffle();
        raffle.generate_scores().unwrap();
        raffle.select_teams();

        let deciding_teams = raffle.get_deciding_teams();
        assert_eq!(deciding_teams.len(), 7); // Based on total_counted_seats
    }

    #[test]
    fn test_get_etherscan_url() {
        let raffle = create_test_raffle();
        let url = raffle.get_etherscan_url();
        assert_eq!(url, "https://etherscan.io/block/110#consensusinfo");
    }

    #[test]
    fn test_generate_random_score_from_seed() {
        let score1 = Raffle::generate_random_score_from_seed("test_seed", 1);
        let score2 = Raffle::generate_random_score_from_seed("test_seed", 2);

        assert!(score1 > 0.0 && score1 <= 1.0);
        assert!(score2 > 0.0 && score2 <= 1.0);
        assert_ne!(score1, score2);
    }

    #[test]
    fn test_custom_team_order() {
        let teams = create_mock_teams();
        let custom_order: Vec<String> = teams.values().map(|team| team.name().to_string()).collect();
        
        let mut config = create_test_config();
        config.custom_team_order = Some(teams.keys().cloned().collect());

        let raffle = Raffle::new(config, &teams).unwrap();

        // Check that the order of team names in snapshots matches the custom order
        let snapshot_names: Vec<String> = raffle.team_snapshots.iter().map(|s| s.name.clone()).collect();
        
        assert_eq!(snapshot_names, custom_order, "Team snapshots should be in the specified custom order");
        assert_eq!(snapshot_names.len(), 9, "There should be 9 team snapshots");
    }

    #[test]
    fn test_raffle_with_excluded_teams() {
        let teams = create_mock_teams();
        let excluded_team_id = *teams.keys().next().unwrap();

        let mut config = create_test_config();
        config.excluded_teams = vec![excluded_team_id];

        let mut raffle = Raffle::new(config, &teams).unwrap();
        raffle.generate_scores().unwrap();
        raffle.select_teams();

        assert!(raffle.result.is_some());
        let result = raffle.result.as_ref().unwrap();

        // Verify that we have 7 counted teams
        assert_eq!(result.counted.len(), 7, "Should have 7 counted teams");

        // Verify that the excluded team is not in the counted list
        assert!(!result.counted.contains(&excluded_team_id), "Excluded team should not be counted");

        // Verify that we have 2 uncounted teams
        assert_eq!(result.uncounted.len(), 2, "Should have 2 uncounted teams");

        // Verify that the total number of teams (counted + uncounted) is still 9
        assert_eq!(result.counted.len() + result.uncounted.len(), 9, 
                "Total of counted and uncounted teams should equal 9");

        // Check if the excluded team is in the uncounted list (this may or may not be true)
        if result.uncounted.contains(&excluded_team_id) {
            println!("Note: The excluded team is present in the uncounted list.");
        } else {
            println!("Note: The excluded team is not present in either counted or uncounted list.");
        }

        // Verify that all teams have unique IDs
        let mut all_team_ids: Vec<Uuid> = result.counted.iter().chain(result.uncounted.iter()).cloned().collect();
        assert_eq!(all_team_ids.len(), 9, "Should have 9 team IDs");
        
        // Sort the team IDs
        all_team_ids.sort();
        
        // Check for uniqueness by comparing adjacent elements
        for i in 1..all_team_ids.len() {
            assert_ne!(all_team_ids[i-1], all_team_ids[i], "All team IDs should be unique");
        }
    }

    // Helper function to create a test raffle
    fn create_test_raffle() -> Raffle {
        let teams = create_mock_teams();
        let config = create_test_config();
        Raffle::new(config, &teams).unwrap()
    }

    // Helper function to create a test config
    fn create_test_config() -> RaffleConfig {
        RaffleConfig {
            proposal_id: Uuid::new_v4(),
            epoch_id: Uuid::new_v4(),
            initiation_block: 100,
            randomness_block: 110,
            block_randomness: "test_randomness".to_string(),
            total_counted_seats: 7,
            max_earner_seats: 5,
            excluded_teams: vec![],
            custom_allocation: None,
            custom_team_order: None,
            is_historical: false,
        }
    }
}