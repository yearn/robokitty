use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use uuid::Uuid;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Epoch {
    id: Uuid,
    name: String,
    start_date: DateTime<Utc>,
    end_date: DateTime<Utc>,
    status: EpochStatus,
    associated_proposals: Vec<Uuid>,
    reward: Option<EpochReward>,
    team_rewards: HashMap<Uuid, TeamReward>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum EpochStatus {
    Planned,
    Active,
    Closed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EpochReward {
    token: String,
    amount: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TeamReward {
    percentage: f64,
    amount: f64,
}

impl Epoch {
    // Constructor
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
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn start_date(&self) -> DateTime<Utc> {
        self.start_date
    }

    pub fn end_date(&self) -> DateTime<Utc> {
        self.end_date
    }

    pub fn status(&self) -> EpochStatus {
        self.status
    }

    pub fn associated_proposals(&self) -> &[Uuid] {
        &self.associated_proposals
    }

    pub fn reward(&self) -> Option<&EpochReward> {
        self.reward.as_ref()
    }

    pub fn team_rewards(&self) -> &HashMap<Uuid, TeamReward> {
        &self.team_rewards
    }

    // Setter methods
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn set_dates(&mut self, start_date: DateTime<Utc>, end_date: DateTime<Utc>) -> Result<(), &'static str> {
        if start_date >= end_date {
            return Err("Start date must be before end date");
        }
        self.start_date = start_date;
        self.end_date = end_date;
        Ok(())
    }

    pub fn set_status(&mut self, status: EpochStatus) {
        self.status = status;
    }

    // Methods for managing associated proposals
    pub fn add_proposal(&mut self, proposal_id: Uuid) {
        if !self.associated_proposals.contains(&proposal_id) {
            self.associated_proposals.push(proposal_id);
        }
    }

    pub fn remove_proposal(&mut self, proposal_id: Uuid) {
        self.associated_proposals.retain(|&id| id != proposal_id);
    }

    // Methods for managing rewards
    pub fn set_reward(&mut self, token: String, amount: f64) -> Result<(), &'static str> {
        self.reward = Some(EpochReward::new(token, amount)?);
        Ok(())
    }

    pub fn remove_reward(&mut self) {
        self.reward = None;
    }

    pub fn set_team_reward(&mut self, team_id: Uuid, percentage: f64, amount: f64) -> Result<(), &'static str> {
        if percentage < 0.0 || percentage > 100.0 {
            return Err("Percentage must be between 0 and 100");
        }
        if amount < 0.0 {
            return Err("Amount must be non-negative");
        }
        self.team_rewards.insert(team_id, TeamReward { percentage, amount });
        Ok(())
    }

    pub fn remove_team_reward(&mut self, team_id: &Uuid) {
        self.team_rewards.remove(team_id);
    }

    // Helper methods
    pub fn activate(&mut self) -> Result<(), &'static str> {
        if self.is_planned() {
            self.status = EpochStatus::Active;
            Ok(())
        } else {
            Err("Only planned epochs can be activated")
        }
    }

    pub fn close(&mut self) -> Result<(), &'static str> {
        if self.is_active() {
            self.status = EpochStatus::Closed;
            Ok(())
        } else {
            Err("Only active epochs can be closed")
        }
    }

    pub fn is_proposal_associated(&self, proposal_id: Uuid) -> bool {
        self.associated_proposals.contains(&proposal_id)
    }

    pub fn total_reward_amount(&self) -> f64 {
        self.reward.as_ref().map_or(0.0, |r| r.amount)
    }

    pub fn distributed_reward_amount(&self) -> f64 {
        self.team_rewards.values().map(|r| r.amount).sum()
    }

    pub fn remaining_reward_amount(&self) -> f64 {
        self.total_reward_amount() - self.distributed_reward_amount()
    }

    pub fn is_planned(&self) -> bool {
        matches!(self.status, EpochStatus::Planned)
    }

    pub fn is_active(&self) -> bool {
        matches!(self.status, EpochStatus::Active)
    }

    pub fn is_closed(&self) -> bool {
        matches!(self.status, EpochStatus::Closed)
    }

}

impl EpochReward {
    pub fn new(token: String, amount: f64) -> Result<Self, &'static str> {
        if amount < 0.0 {
            return Err("Reward amount must be non-negative");
        }
        Ok(Self { token, amount })
    }

    pub fn token(&self) -> &str {
        &self.token
    }

    pub fn amount(&self) -> f64 {
        self.amount
    }
}

impl TeamReward {
    pub fn new(percentage: f64, amount: f64) -> Result<Self, &'static str> {
        if percentage < 0.0 || percentage > 100.0 {
            return Err("Percentage must be between 0 and 100");
        }
        if amount < 0.0 {
            return Err("Amount must be non-negative");
        }
        Ok(Self { percentage, amount })
    }

    pub fn percentage(&self) -> f64 {
        self.percentage
    }

    pub fn amount(&self) -> f64 {
        self.amount
    }
}