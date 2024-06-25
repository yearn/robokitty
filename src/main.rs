use chrono::{DateTime, Utc};
use ethers::prelude::*;
//use eyre::{Ok, Result};
use serde::{Serialize, Deserialize};
use sha2::{Sha256, Digest};
use std::{
    collections::HashMap,
    fs,
    str,
    sync::Arc,
};
use tokio::{
    self,
    time::{sleep, Duration},
};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
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
async fn main() -> eyre::Result<()> {

    // Connect to reth via ipc
    let provider = Provider::connect_ipc("/tmp/reth.ipc").await?;
    let client = Arc::new(provider);

    // Get current latest block number from chain
    let latest_block = client.get_block_number().await?.as_u64();
    println!("Current block height: {}", latest_block);

    // Get randomness from latest block
    match client.get_block(latest_block).await {
        Ok(Some(block)) => {
            match block.mix_hash {
                Some(mix_hash) => {
                    println!("Randomness: {:x}", mix_hash);
                }
                None => {
                    println!("Randomness not found for block {}", latest_block);
                }
            }
        }
        Ok(None) => {
            println!("Block number {} not found", latest_block);
        }
        Err(e) => {
            eprintln!("Error fetching block {}:{:?}", latest_block, e);
        }
    }

    let test_randomness = "0xd0cb380f49b60f392631607e78ba2cd1094fa8069918edcfc97455b7ad029db4";
    let test_index: u64 = 0;
    println!("Test draw output:{}", draw_with(test_randomness, test_index));

    let mut system = BudgetSystem::new();

    // Adding teams
    system.add_team("Team A".to_string(), "Alice".to_string(), Some(vec![100000])).unwrap();
    system.add_team("Team B".to_string(), "Bob".to_string(), None).unwrap();

    // Updating team status
    system.update_team_status("Team B", TeamStatus::Earner { trailing_monthly_revenue: vec![50000] }).unwrap();

    // Updating team revenue
    system.update_team_revenue("Team A", vec![120000, 5858]).unwrap();

    // Removing a team
    system.remove_team("Team B").unwrap();

    // Load system from file
    let loaded_system = BudgetSystem::load_from_file().unwrap();

    // Check historical state
    if let Some(historical_state) = loaded_system.get_state_at(1) {
        println!("Historical state at index 1: {:?}", historical_state.timestamp);
        println!("Number of teams: {}", historical_state.teams.len());
    }

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
}