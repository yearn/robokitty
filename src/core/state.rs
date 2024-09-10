// src/core/state.rs

use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::core::models::{Team, Proposal, Raffle, Vote, Epoch};


#[derive(Clone, Serialize, Deserialize)]
pub struct SystemState {
    pub teams: HashMap<Uuid, Team>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
pub struct BudgetSystemState {
    pub current_state: SystemState,
    pub history: Vec<SystemState>,
    pub proposals: HashMap<Uuid, Proposal>,
    pub raffles: HashMap<Uuid, Raffle>,
    pub votes: HashMap<Uuid, Vote>,
    pub epochs: HashMap<Uuid, Epoch>,
    pub current_epoch: Option<Uuid>,
}

impl SystemState {
    pub fn new() -> Self {
        SystemState {
            teams: HashMap::new(),
            timestamp: Utc::now(),
        }
    }
}

impl BudgetSystemState {
    pub fn new() -> Self {
        BudgetSystemState {
            current_state: SystemState::new(),
            history: Vec::new(),
            proposals: HashMap::new(),
            raffles: HashMap::new(),
            votes: HashMap::new(),
            epochs: HashMap::new(),
            current_epoch: None,
        }
    }
}