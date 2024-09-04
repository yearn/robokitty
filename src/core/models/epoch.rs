use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Epoch {
    pub id: Uuid,
    pub name: String,
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub status: EpochStatus,
    pub associated_proposals: Vec<Uuid>,
    pub reward: Option<EpochReward>,
    pub team_rewards: HashMap<Uuid, TeamReward>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum EpochStatus {
    Planned,
    Active,
    Closed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EpochReward {
    pub token: String,
    pub amount: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TeamReward {
   pub percentage: f64,
    pub amount: f64,
}

impl Epoch {
    pub fn new(name: String, start_date: DateTime<Utc>, end_date: DateTime<Utc>) -> Result<Self, &'static str> {
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

    // Getter methods
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn set_reward(&mut self, token: String, amount: f64) -> Result<(), &'static str> {
        self.reward = Some(EpochReward { token, amount });
        Ok(())
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn start_date(&self) -> DateTime<Utc> {
        self.start_date
    }

    pub fn end_date(&self) -> DateTime<Utc> {
        self.end_date
    }

    pub fn status(&self) -> &EpochStatus {
        &self.status
    }

}
