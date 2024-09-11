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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;
    use crate::core::models::{TeamStatus, RaffleConfig, VoteType};

    // Helper functions to create test entities
    fn create_test_team(name: &str) -> Team {
        Team::new(name.to_string(), "Representative".to_string(), Some(vec![1000, 2000, 3000])).unwrap()
    }

    fn create_test_raffle() -> Raffle {
        let config = RaffleConfig::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            7,
            5,
            Some(100),
            Some(110),
            Some("test_randomness".to_string()),
            None,
            None,
            None,
            false
        );
        Raffle::new(config, &HashMap::new()).unwrap()
    }

    fn create_test_vote() -> Vote {
        Vote::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            VoteType::Informal,
            false
        )
    }

    fn create_test_epoch() -> Epoch {
        Epoch::new(
            "Test Epoch".to_string(),
            Utc::now(),
            Utc::now() + chrono::Duration::days(30)
        ).unwrap()
    }

    // SystemState Tests
    #[test]
    fn test_system_state_creation() {
        let teams = HashMap::new();
        let state = SystemState::new(teams);
        assert_eq!(state.teams().len(), 0);
        assert!(state.timestamp() <= Utc::now());
    }

    #[test]
    fn test_add_team() {
        let mut state = SystemState::new(HashMap::new());
        let team = create_test_team("Test Team");
        let id = state.add_team(team);
        assert_eq!(state.teams().len(), 1);
        assert!(state.teams().contains_key(&id));
    }

    #[test]
    fn test_remove_team() {
        let mut state = SystemState::new(HashMap::new());
        let team = create_test_team("Test Team");
        let id = state.add_team(team);
        assert_eq!(state.teams().len(), 1);
        let removed_team = state.remove_team(id);
        assert!(removed_team.is_some());
        assert_eq!(state.teams().len(), 0);
    }

    #[test]
    fn test_update_team() {
        let mut state = SystemState::new(HashMap::new());
        let team = create_test_team("Test Team");
        let id = state.add_team(team);
        let mut updated_team = create_test_team("Updated Team");
        updated_team.set_representative("New Representative".to_string());
        assert!(state.update_team(id, updated_team).is_ok());
        let updated = state.get_team(&id).unwrap();
        assert_eq!(updated.name(), "Updated Team");
        assert_eq!(updated.representative(), "New Representative");
    }

    #[test]
    fn test_get_team() {
        let mut state = SystemState::new(HashMap::new());
        let team = create_test_team("Test Team");
        let id = state.add_team(team);
        let retrieved_team = state.get_team(&id);
        assert!(retrieved_team.is_some());
        assert_eq!(retrieved_team.unwrap().name(), "Test Team");
    }

    #[test]
    fn test_team_count() {
        let mut state = SystemState::new(HashMap::new());
        assert_eq!(state.team_count(), 0);
        state.add_team(create_test_team("Team 1"));
        state.add_team(create_test_team("Team 2"));
        assert_eq!(state.team_count(), 2);
    }

    #[test]
    fn test_update_timestamp() {
        let mut state = SystemState::new(HashMap::new());
        let initial_timestamp = state.timestamp();
        std::thread::sleep(std::time::Duration::from_secs(1));
        state.update_timestamp();
        assert!(state.timestamp() > initial_timestamp);
    }

    // BudgetSystemState Tests
    #[test]
    fn test_budget_system_state_creation() {
        let state = BudgetSystemState::new();
        assert_eq!(state.current_state().teams().len(), 0);
        assert!(state.history().is_empty());
        assert!(state.proposals().is_empty());
        assert!(state.raffles().is_empty());
        assert!(state.votes().is_empty());
        assert!(state.epochs().is_empty());
        assert!(state.current_epoch().is_none());
    }

    #[test]
    fn test_update_current_state() {
        let mut state = BudgetSystemState::new();
        let mut new_system_state = SystemState::new(HashMap::new());
        new_system_state.add_team(create_test_team("New Team"));
        state.update_current_state(new_system_state);
        assert_eq!(state.current_state().teams().len(), 1);
        assert_eq!(state.history().len(), 1);
    }

    #[test]
    fn test_state_add_team() {
        let mut state = BudgetSystemState::new();
        let team = create_test_team("Test Team");
        let id = state.add_team(team);
        assert_eq!(state.current_state().teams().len(), 1);
        assert!(state.current_state().teams().contains_key(&id));
    }

    #[test]
    fn test_state_remove_team() {
        let mut state = BudgetSystemState::new();
        let team = create_test_team("Test Team");
        let id = state.add_team(team);
        assert_eq!(state.current_state().teams().len(), 1);
        let removed_team = state.remove_team(id);
        assert!(removed_team.is_some());
        assert_eq!(state.current_state().teams().len(), 0);
    }

    #[test]
    fn test_state_update_team() {
        let mut state = BudgetSystemState::new();
        let team = create_test_team("Test Team");
        let id = state.add_team(team);
        let mut updated_team = create_test_team("Updated Team");
        updated_team.set_representative("New Representative".to_string());
        assert!(state.update_team(id, updated_team).is_ok());
        let updated = state.get_team(&id).unwrap();
        assert_eq!(updated.name(), "Updated Team");
        assert_eq!(updated.representative(), "New Representative");
    }

    #[test]
    fn test_add_proposal() {
        let mut state = BudgetSystemState::new();
        let proposal = Proposal::new(
            Uuid::new_v4(),
            "Test Proposal".to_string(),
            Some("http://example.com".to_string()),
            None,
            Some(Utc::now().date_naive()),
            Some(Utc::now().date_naive()),
            None,
        );
        let id = state.add_proposal(&proposal);
        assert_eq!(state.proposals().len(), 1);
        assert!(state.proposals().contains_key(&id));
    }

    #[test]
    fn test_remove_proposal() {
        let mut state = BudgetSystemState::new();
        let proposal = Proposal::new(
            Uuid::new_v4(),
            "Test Proposal".to_string(),
            Some("http://example.com".to_string()),
            None,
            Some(Utc::now().date_naive()),
            Some(Utc::now().date_naive()),
            None,
        );
        let id = state.add_proposal(&proposal);
        assert_eq!(state.proposals().len(), 1);
        let removed_proposal = state.remove_proposal(id);
        assert!(removed_proposal.is_some());
        assert_eq!(state.proposals().len(), 0);
    }

    #[test]
    fn test_add_raffle() {
        let mut state = BudgetSystemState::new();
        let raffle = create_test_raffle();
        let id = state.add_raffle(&raffle);
        assert_eq!(state.raffles().len(), 1);
        assert!(state.raffles().contains_key(&id));
    }

    #[test]
    fn test_remove_raffle() {
        let mut state = BudgetSystemState::new();
        let raffle = create_test_raffle();
        let id = state.add_raffle(&raffle);
        assert_eq!(state.raffles().len(), 1);
        let removed_raffle = state.remove_raffle(id);
        assert!(removed_raffle.is_some());
        assert_eq!(state.raffles().len(), 0);
    }

    #[test]
    fn test_get_raffle() {
        let mut state = BudgetSystemState::new();
        let raffle = create_test_raffle();
        let id = state.add_raffle(&raffle);
        let retrieved_raffle = state.get_raffle(&id);
        assert!(retrieved_raffle.is_some());
        assert_eq!(retrieved_raffle.unwrap().id(), raffle.id());
    }

    #[test]
    fn test_raffle_count() {
        let mut state = BudgetSystemState::new();
        assert_eq!(state.raffle_count(), 0);
        state.add_raffle(&create_test_raffle());
        state.add_raffle(&create_test_raffle());
        assert_eq!(state.raffle_count(), 2);
    }

    #[test]
    fn test_add_vote() {
        let mut state = BudgetSystemState::new();
        let vote = create_test_vote();
        let id = state.add_vote(&vote);
        assert_eq!(state.votes().len(), 1);
        assert!(state.votes().contains_key(&id));
    }

    #[test]
    fn test_remove_vote() {
        let mut state = BudgetSystemState::new();
        let vote = create_test_vote();
        let id = state.add_vote(&vote);
        assert_eq!(state.votes().len(), 1);
        let removed_vote = state.remove_vote(id);
        assert!(removed_vote.is_some());
        assert_eq!(state.votes().len(), 0);
    }

    #[test]
    fn test_get_vote() {
        let mut state = BudgetSystemState::new();
        let vote = create_test_vote();
        let id = state.add_vote(&vote);
        let retrieved_vote = state.get_vote(&id);
        assert!(retrieved_vote.is_some());
        assert_eq!(retrieved_vote.unwrap().id(), vote.id());
    }

    #[test]
    fn test_vote_count() {
        let mut state = BudgetSystemState::new();
        assert_eq!(state.vote_count(), 0);
        state.add_vote(&create_test_vote());
        state.add_vote(&create_test_vote());
        assert_eq!(state.vote_count(), 2);
    }

    #[test]
    fn test_add_epoch() {
        let mut state = BudgetSystemState::new();
        let epoch = create_test_epoch();
        let id = state.add_epoch(&epoch);
        assert_eq!(state.epochs().len(), 1);
        assert!(state.epochs().contains_key(&id));
    }

    #[test]
    fn test_remove_epoch() {
        let mut state = BudgetSystemState::new();
        let epoch = create_test_epoch();
        let id = state.add_epoch(&epoch);
        assert_eq!(state.epochs().len(), 1);
        let removed_epoch = state.remove_epoch(id);
        assert!(removed_epoch.is_some());
        assert_eq!(state.epochs().len(), 0);
    }

    #[test]
    fn test_get_epoch() {
        let mut state = BudgetSystemState::new();
        let epoch = create_test_epoch();
        let id = state.add_epoch(&epoch);
        let retrieved_epoch = state.get_epoch(&id);
        assert!(retrieved_epoch.is_some());
        assert_eq!(retrieved_epoch.unwrap().id(), epoch.id());
    }

    #[test]
    fn test_epoch_count() {
        let mut state = BudgetSystemState::new();
        assert_eq!(state.epoch_count(), 0);
        state.add_epoch(&create_test_epoch());
        state.add_epoch(&create_test_epoch());
        assert_eq!(state.epoch_count(), 2);
    }

    #[test]
    fn test_set_current_epoch() {
        let mut state = BudgetSystemState::new();
        let epoch = Epoch::new(
            "Test Epoch".to_string(),
            Utc::now(),
            Utc::now() + chrono::Duration::days(30),
        ).unwrap();
        let id = state.add_epoch(&epoch);
        state.set_current_epoch(Some(id));
        assert_eq!(state.current_epoch(), Some(id));
    }

    #[test]
    fn test_get_proposal() {
        let mut state = BudgetSystemState::new();
        let proposal = Proposal::new(
            Uuid::new_v4(),
            "Test Proposal".to_string(),
            Some("http://example.com".to_string()),
            None,
            Some(Utc::now().date_naive()),
            Some(Utc::now().date_naive()),
            None,
        );
        let id = state.add_proposal(&proposal);
        let retrieved_proposal = state.get_proposal(&id);
        assert!(retrieved_proposal.is_some());
        assert_eq!(retrieved_proposal.unwrap().title(), "Test Proposal");
    }


    #[test]
    fn test_proposal_count() {
        let mut state = BudgetSystemState::new();
        assert_eq!(state.proposal_count(), 0);
        let proposal1 = Proposal::new(
            Uuid::new_v4(),
            "Proposal 1".to_string(),
            None,
            None,
            None,
            None,
            None,
        );
        let proposal2 = Proposal::new(
            Uuid::new_v4(),
            "Proposal 2".to_string(),
            None,
            None,
            None,
            None,
            None,
        );
        state.add_proposal(&proposal1);
        state.add_proposal(&proposal2);
        assert_eq!(state.proposal_count(), 2);
    }

    // Edge Case Tests
    #[test]
    fn test_remove_non_existent_team() {
        let mut state = SystemState::new(HashMap::new());
        let non_existent_id = Uuid::new_v4();
        let result = state.remove_team(non_existent_id);
        assert!(result.is_none());
    }

    #[test]
    fn test_update_non_existent_team() {
        let mut state = SystemState::new(HashMap::new());
        let non_existent_id = Uuid::new_v4();
        let team = create_test_team("Test Team");
        let result = state.update_team(non_existent_id, team);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Team not found");
    }

    #[test]
    fn test_get_non_existent_team() {
        let state = SystemState::new(HashMap::new());
        let non_existent_id = Uuid::new_v4();
        let result = state.get_team(&non_existent_id);
        assert!(result.is_none());
    }

    #[test]
    fn test_remove_non_existent_proposal() {
        let mut state = BudgetSystemState::new();
        let non_existent_id = Uuid::new_v4();
        let result = state.remove_proposal(non_existent_id);
        assert!(result.is_none());
    }

    #[test]
    fn test_remove_non_existent_raffle() {
        let mut state = BudgetSystemState::new();
        let non_existent_id = Uuid::new_v4();
        let result = state.remove_raffle(non_existent_id);
        assert!(result.is_none());
    }

    #[test]
    fn test_remove_non_existent_vote() {
        let mut state = BudgetSystemState::new();
        let non_existent_id = Uuid::new_v4();
        let result = state.remove_vote(non_existent_id);
        assert!(result.is_none());
    }

    #[test]
    fn test_remove_non_existent_epoch() {
        let mut state = BudgetSystemState::new();
        let non_existent_id = Uuid::new_v4();
        let result = state.remove_epoch(non_existent_id);
        assert!(result.is_none());
    }

    #[test]
    fn test_set_current_epoch_non_existent() {
        let mut state = BudgetSystemState::new();
        let non_existent_id = Uuid::new_v4();
        state.set_current_epoch(Some(non_existent_id));
        assert_eq!(state.current_epoch(), Some(non_existent_id));
        // Note: This test demonstrates that set_current_epoch doesn't validate the epoch existence
    }

    #[test]
    fn test_update_empty_state() {
        let mut state = BudgetSystemState::new();
        let empty_system_state = SystemState::new(HashMap::new());
        state.update_current_state(empty_system_state);
        assert!(state.current_state().teams().is_empty());
        assert_eq!(state.history().len(), 1);
    }

    // Error Handling Tests
    #[test]
    fn test_update_team_invalid_id() {
        let mut state = BudgetSystemState::new();
        let invalid_id = Uuid::new_v4();
        let team = create_test_team("Test Team");
        let result = state.update_team(invalid_id, team);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Team not found");
    }

    #[test]
    fn test_get_proposal_invalid_id() {
        let state = BudgetSystemState::new();
        let invalid_id = Uuid::new_v4();
        let result = state.get_proposal(&invalid_id);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_raffle_invalid_id() {
        let state = BudgetSystemState::new();
        let invalid_id = Uuid::new_v4();
        let result = state.get_raffle(&invalid_id);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_vote_invalid_id() {
        let state = BudgetSystemState::new();
        let invalid_id = Uuid::new_v4();
        let result = state.get_vote(&invalid_id);
        assert!(result.is_none());
    }

    #[test]
    fn test_get_epoch_invalid_id() {
        let state = BudgetSystemState::new();
        let invalid_id = Uuid::new_v4();
        let result = state.get_epoch(&invalid_id);
        assert!(result.is_none());
    }

    // Additional edge case tests

    #[test]
    fn test_add_team_with_existing_id() {
        let mut state = BudgetSystemState::new();
        let team1 = create_test_team("Team 1");
        let id1 = state.add_team(team1);
        let team2 = create_test_team("Team 2");
        let id2 = state.add_team(team2);
        assert_ne!(id1, id2);
        assert_eq!(state.current_state().team_count(), 2);
    }

    #[test]
    fn test_update_team_status() {
        let mut state = BudgetSystemState::new();
        let mut team = create_test_team("Test Team");
        let id = state.add_team(team.clone());
        
        team.set_status(TeamStatus::Inactive).unwrap();
        state.update_team(id, team).unwrap();
        
        let updated_team = state.get_team(&id).unwrap();
        assert!(matches!(updated_team.status(), TeamStatus::Inactive));
    }

    #[test]
    fn test_budget_system_state_history() {
        let mut state = BudgetSystemState::new();
        assert_eq!(state.history().len(), 0, "Initial history should be empty");

        let team = create_test_team("Test Team");
        state.add_team(team);
        assert_eq!(state.history().len(), 0, "Adding a team should not affect history");

        let current_state = state.current_state().clone();
        state.update_current_state(current_state);
        assert_eq!(state.history().len(), 1, "Updating current state should add to history");

        let another_team = create_test_team("Another Team");
        state.add_team(another_team);
        assert_eq!(state.history().len(), 1, "Adding another team should not affect history");

        let new_current_state = state.current_state().clone();
        state.update_current_state(new_current_state);
        assert_eq!(state.history().len(), 2, "Updating current state again should add to history");
    }

    #[test]
    fn test_system_state_timestamp_update() {
        let mut state = SystemState::new(HashMap::new());
        let initial_timestamp = state.timestamp();
        
        std::thread::sleep(std::time::Duration::from_millis(10));
        state.update_timestamp();
        
        assert!(state.timestamp() > initial_timestamp);
    }

}