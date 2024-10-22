use uuid::Uuid;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};

use super::team::{Team, TeamStatus};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Raffle {
    id: Uuid,
    config: RaffleConfig,
    team_snapshots: Vec<TeamSnapshot>,
    tickets: Vec<RaffleTicket>,
    result: Option<RaffleResult>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RaffleConfig {
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
pub struct TeamSnapshot {
    id: Uuid,
    name: String,
    representative: String,
    status: TeamStatus,
    snapshot_time: DateTime<Utc>,
    raffle_status: RaffleParticipationStatus,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RaffleTicket {
    team_id: Uuid,
    index: u64,
    score: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RaffleResult {
    counted: Vec<Uuid>,
    uncounted: Vec<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum RaffleParticipationStatus {
    Included,
    Excluded,
}

impl Raffle {
    pub fn new(config: RaffleConfig, teams: &HashMap<Uuid, Team>) -> Result<Self, &'static str> {

        if config.max_earner_seats() > config.total_counted_seats() {
            return Err("Max earner seats cannot exceed total counted seats");
        }

        let mut team_snapshots = Vec::new();
        let mut tickets = Vec::new();

        // Create team snapshots
        let mut active_teams: Vec<_> = teams.values()
            .filter(|team| team.is_active())
            .collect();

        // Sort teams based on custom order or by name
        if let Some(custom_order) = config.custom_team_order() {
            active_teams.sort_by_key(|team| custom_order.iter().position(|&id| id == team.id()).unwrap_or(usize::MAX));
        } else {
            active_teams.sort_by(|a, b| a.name().cmp(&b.name()));
        }

        // Create snapshots and tickets
        for team in active_teams {
            let snapshot = TeamSnapshot::new(
                team.id(),
                team.name().to_string(),
                team.representative().to_string(),
                team.status().clone(),
                if config.excluded_teams().contains(&team.id()) {
                    RaffleParticipationStatus::Excluded
                } else {
                    RaffleParticipationStatus::Included
                },
            );
            team_snapshots.push(snapshot);

            let ticket_count = match team.status() {
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

    // Getter methods
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn config(&self) -> &RaffleConfig {
        &self.config
    }

    pub fn config_mut(&mut self) -> &mut RaffleConfig {
        &mut self.config
    }

    pub fn team_snapshots(&self) -> &[TeamSnapshot] {
        &self.team_snapshots
    }

    pub fn tickets(&self) -> &[RaffleTicket] {
        &self.tickets
    }

    pub fn result(&self) -> Option<&RaffleResult> {
        self.result.as_ref()
    }

    pub fn deciding_teams(&self) -> Vec<Uuid> {
        self.result.as_ref()
            .map(|result| result.counted.clone())
            .unwrap_or_default()
    }

    pub fn etherscan_url(&self) -> String {
        format!("https://etherscan.io/block/{}#consensusinfo", self.config.randomness_block)
    }

    pub fn generate_ticket_scores(&mut self) -> Result<(), &'static str> {
        for ticket in &mut self.tickets {
            if !self.config.excluded_teams().contains(&ticket.team_id()) {
                let score = Self::generate_random_score_from_seed(self.config.block_randomness(), ticket.index());
                ticket.set_score(score);
            }
            // Excluded teams keep their score as 0.0
        }
        Ok(())
    }

    pub fn select_deciding_teams(&mut self) {
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

    // Setter methods
    pub fn set_result(&mut self, result: RaffleResult) {
        self.result = Some(result);
    }

    // Helper methods
    pub fn is_historical(&self) -> bool {
        self.config.is_historical
    }

    pub fn is_completed(&self) -> bool {
        self.result.is_some()
    }
}

impl RaffleConfig {
    pub fn new(
        proposal_id: Uuid,
        epoch_id: Uuid,
        total_counted_seats: usize,
        max_earner_seats: usize,
        initiation_block: Option<u64>,
        randomness_block: Option<u64>,
        block_randomness: Option<String>,
        excluded_teams: Option<Vec<Uuid>>,
        custom_allocation: Option<HashMap<Uuid, u64>>,
        custom_team_order: Option<Vec<Uuid>>,
        is_historical: bool,
    ) -> Self {
        Self {
            proposal_id,
            epoch_id,
            initiation_block: initiation_block.unwrap_or(0),
            randomness_block: randomness_block.unwrap_or(0),
            block_randomness: block_randomness.unwrap_or_else(String::new),
            total_counted_seats,
            max_earner_seats,
            excluded_teams: excluded_teams.unwrap_or_default(),
            custom_allocation,
            custom_team_order,
            is_historical,
        }
    }

    // Getter methods
    pub fn proposal_id(&self) -> Uuid { self.proposal_id }
    pub fn epoch_id(&self) -> Uuid { self.epoch_id }
    pub fn initiation_block(&self) -> u64 { self.initiation_block }
    pub fn randomness_block(&self) -> u64 { self.randomness_block }
    pub fn block_randomness(&self) -> &str { &self.block_randomness }
    pub fn total_counted_seats(&self) -> usize { self.total_counted_seats }
    pub fn max_earner_seats(&self) -> usize { self.max_earner_seats }
    pub fn excluded_teams(&self) -> &[Uuid] { &self.excluded_teams }
    pub fn custom_allocation(&self) -> Option<&HashMap<Uuid, u64>> { self.custom_allocation.as_ref() }
    pub fn custom_team_order(&self) -> Option<&[Uuid]> { self.custom_team_order.as_deref() }
    pub fn is_historical(&self) -> bool { self.is_historical }

    // Setter methods
    pub fn set_initiation_block(&mut self, block: u64) { self.initiation_block = block; }
    pub fn set_randomness_block(&mut self, block: u64) { self.randomness_block = block; }
    pub fn set_block_randomness(&mut self, randomness: String) { self.block_randomness = randomness; }
    pub fn set_excluded_teams(&mut self, teams: Vec<Uuid>) { self.excluded_teams = teams; }
    pub fn set_custom_allocation(&mut self, allocation: Option<HashMap<Uuid, u64>>) { self.custom_allocation = allocation; }
    pub fn set_custom_team_order(&mut self, order: Option<Vec<Uuid>>) { self.custom_team_order = order; }
}

impl RaffleTicket {
    pub fn new(team_id: Uuid, index: u64) -> Self {
        Self {
            team_id,
            index,
            score: 0.0,
        }
    }

    // Getter methods
    pub fn team_id(&self) -> Uuid { self.team_id }
    pub fn index(&self) -> u64 { self.index }
    pub fn score(&self) -> f64 { self.score }

    // Setter methods
    pub fn set_score(&mut self, score: f64) { self.score = score; }
}

impl TeamSnapshot {
    pub fn new(
        id: Uuid,
        name: String,
        representative: String,
        status: TeamStatus,
        raffle_status: RaffleParticipationStatus,
    ) -> Self {
        Self {
            id,
            name,
            representative,
            status,
            snapshot_time: Utc::now(),
            raffle_status,
        }
    }

    // Getter methods
    pub fn id(&self) -> Uuid { self.id }
    pub fn name(&self) -> &str { &self.name }
    pub fn representative(&self) -> &str { &self.representative }
    pub fn status(&self) -> &TeamStatus { &self.status }
    pub fn snapshot_time(&self) -> DateTime<Utc> { self.snapshot_time }
    pub fn raffle_status(&self) -> &RaffleParticipationStatus { &self.raffle_status }

    // No setter methods as this is a snapshot and should not be modified after creation
}

impl RaffleResult {
    pub fn new(counted: Vec<Uuid>, uncounted: Vec<Uuid>) -> Self {
        Self { counted, uncounted }
    }

    // Getter methods
    pub fn counted(&self) -> &[Uuid] { &self.counted }
    pub fn uncounted(&self) -> &[Uuid] { &self.uncounted }

    // No setter methods as the result should not be modified after creation
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use uuid::Uuid;

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
        raffle.generate_ticket_scores().unwrap();

        for ticket in &raffle.tickets {
            assert!(ticket.score > 0.0 && ticket.score <= 1.0);
        }
    }

    #[test]
    fn test_select_teams() {
        let mut raffle = create_test_raffle();
        raffle.generate_ticket_scores().unwrap();
        raffle.select_deciding_teams();

        assert!(raffle.result.is_some());
        let result = raffle.result.as_ref().unwrap();
        assert_eq!(result.counted.len(), 7); // Based on total_counted_seats
        assert_eq!(result.uncounted.len(), 2); // The remaining teams
    }

    #[test]
    fn test_max_earner_seats() {
        let mut raffle = create_test_raffle();
        raffle.generate_ticket_scores().unwrap();
        raffle.select_deciding_teams();

        let result = raffle.result.as_ref().unwrap();
        let counted_earners = result.counted.iter()
            .filter(|&team_id| raffle.team_snapshots.iter().any(|s| s.id == *team_id && matches!(s.status, TeamStatus::Earner { .. })))
            .count();

        assert!(counted_earners <= 5); // max_earner_seats
    }

    #[test]
    fn test_get_deciding_teams() {
        let mut raffle = create_test_raffle();
        raffle.generate_ticket_scores().unwrap();
        raffle.select_deciding_teams();

        let deciding_teams = raffle.deciding_teams();
        assert_eq!(deciding_teams.len(), 7); // Based on total_counted_seats
    }

    #[test]
    fn test_get_etherscan_url() {
        let raffle = create_test_raffle();
        let url = raffle.etherscan_url();
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
        raffle.generate_ticket_scores().unwrap();
        raffle.select_deciding_teams();

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