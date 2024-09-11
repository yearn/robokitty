// src/core/state.rs

use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::core::models::{Team, Proposal, Raffle, Vote, Epoch};


#[derive(Clone, Serialize, Deserialize)]
pub struct SystemState {
    teams: HashMap<Uuid, Team>,
    timestamp: DateTime<Utc>,
}

#[derive(Serialize, Deserialize)]
pub struct BudgetSystemState {
    current_state: SystemState,
    history: Vec<SystemState>,
    proposals: HashMap<Uuid, Proposal>,
    raffles: HashMap<Uuid, Raffle>,
    votes: HashMap<Uuid, Vote>,
    epochs: HashMap<Uuid, Epoch>,
    current_epoch: Option<Uuid>,
}

impl SystemState {
    // Constructor
    pub fn new(teams: HashMap<Uuid, Team>) -> Self {
        Self {
            teams,
            timestamp: Utc::now(),
        }
    }

    // Getter methods
    pub fn teams(&self) -> &HashMap<Uuid, Team> {
        &self.teams
    }

    pub fn timestamp(&self) -> DateTime<Utc> {
        self.timestamp
    }

    // Setter methods
    pub fn add_team(&mut self, team: Team) -> Uuid {
        let id = team.id();
        self.teams.insert(id, team);
        id
    }

    pub fn remove_team(&mut self, id: Uuid) -> Option<Team> {
        self.teams.remove(&id)
    }

    pub fn update_team(&mut self, id: Uuid, updated_team: Team) -> Result<(), &'static str> {
        if self.teams.contains_key(&id) {
            self.teams.insert(id, updated_team);
            Ok(())
        } else {
            Err("Team not found")
        }
    }

    pub fn update_timestamp(&mut self) {
        self.timestamp = Utc::now();
    }

    // Helper methods
    pub fn get_team(&self, id: &Uuid) -> Option<&Team> {
        self.teams.get(id)
    }

    pub fn get_team_mut(&mut self, id: &Uuid) -> Option<&mut Team> {
        self.teams.get_mut(id)
    }

    pub fn team_count(&self) -> usize {
        self.teams.len()
    }
}

impl BudgetSystemState {
    pub fn new() -> Self {
        Self {
            current_state: SystemState::new(HashMap::new()),
            history: Vec::new(),
            proposals: HashMap::new(),
            raffles: HashMap::new(),
            votes: HashMap::new(),
            epochs: HashMap::new(),
            current_epoch: None,
        }
    }

    // Getters
    pub fn current_state(&self) -> &SystemState {
        &self.current_state
    }

    pub fn history(&self) -> &[SystemState] {
        &self.history
    }

    pub fn proposals(&self) -> &HashMap<Uuid, Proposal> {
        &self.proposals
    }

    pub fn raffles(&self) -> &HashMap<Uuid, Raffle> {
        &self.raffles
    }

    pub fn votes(&self) -> &HashMap<Uuid, Vote> {
        &self.votes
    }

    pub fn epochs(&self) -> &HashMap<Uuid, Epoch> {
        &self.epochs
    }

    pub fn current_epoch(&self) -> Option<Uuid> {
        self.current_epoch
    }

    // Setters and modifiers
    pub fn update_current_state(&mut self, new_state: SystemState) {
        self.history.push(self.current_state.clone());
        self.current_state = new_state;
        self.current_state.update_timestamp();
    }

    pub fn add_team(&mut self, team: Team) -> Uuid {
        self.current_state.add_team(team)
    }

    pub fn remove_team(&mut self, id: Uuid) -> Option<Team> {
        self.current_state.remove_team(id)
    }

    pub fn update_team(&mut self, id: Uuid, updated_team: Team) -> Result<(), &'static str> {
        self.current_state.update_team(id, updated_team)
    }

    pub fn get_team(&self, id: &Uuid) -> Option<&Team> {
        self.current_state.get_team(id)
    }

    pub fn get_team_mut(&mut self, id: &Uuid) -> Option<&mut Team> {
        self.current_state.get_team_mut(id)
    }

    pub fn add_proposal(&mut self, proposal: &Proposal) -> Uuid {
        let id = proposal.id();
        self.proposals.insert(id, proposal.clone());
        id
    }

    pub fn remove_proposal(&mut self, id: Uuid) -> Option<Proposal> {
        self.proposals.remove(&id)
    }

    pub fn add_raffle(&mut self, raffle: &Raffle) -> Uuid {
        let id = raffle.id();
        self.raffles.insert(id, raffle.clone());
        id
    }

    pub fn remove_raffle(&mut self, id: Uuid) -> Option<Raffle> {
        self.raffles.remove(&id)
    }

    pub fn add_vote(&mut self, vote: &Vote) -> Uuid {
        let id = vote.id();
        self.votes.insert(id, vote.clone());
        id
    }

    pub fn remove_vote(&mut self, id: Uuid) -> Option<Vote> {
        self.votes.remove(&id)
    }

    pub fn add_epoch(&mut self, epoch: &Epoch) -> Uuid {
        let id = epoch.id();
        self.epochs.insert(id, epoch.clone());
        id
    }

    pub fn remove_epoch(&mut self, id: Uuid) -> Option<Epoch> {
        self.epochs.remove(&id)
    }

    pub fn set_current_epoch(&mut self, epoch_id: Option<Uuid>) {
        self.current_epoch = epoch_id;
    }

    // Helper methods
    pub fn get_proposal(&self, id: &Uuid) -> Option<&Proposal> {
        self.proposals.get(id)
    }

    pub fn get_proposal_mut(&mut self, id: &Uuid) -> Option<&mut Proposal> {
        self.proposals.get_mut(id)
    }

    pub fn get_raffle(&self, id: &Uuid) -> Option<&Raffle> {
        self.raffles.get(id)
    }

    pub fn get_raffle_mut(&mut self, id: &Uuid) -> Option<&mut Raffle> {
        self.raffles.get_mut(id)
    }

    pub fn get_vote(&self, id: &Uuid) -> Option<&Vote> {
        self.votes.get(id)
    }

    pub fn get_vote_mut(&mut self, id: &Uuid) -> Option<&mut Vote> {
        self.votes.get_mut(id)
    }

    pub fn get_epoch(&self, id: &Uuid) -> Option<&Epoch> {
        self.epochs.get(id)
    }

    pub fn get_epoch_mut(&mut self, id: &Uuid) -> Option<&mut Epoch> {
        self.epochs.get_mut(id)
    }

    pub fn proposal_count(&self) -> usize {
        self.proposals.len()
    }

    pub fn raffle_count(&self) -> usize {
        self.raffles.len()
    }

    pub fn vote_count(&self) -> usize {
        self.votes.len()
    }

    pub fn epoch_count(&self) -> usize {
        self.epochs.len()
    }
}