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