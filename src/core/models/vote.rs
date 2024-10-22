use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use std::collections::HashMap;
use super::common::NameMatches;
use super::RaffleResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vote {
    id: Uuid,
    proposal_id: Uuid,
    epoch_id: Uuid,
    vote_type: VoteType,
    status: VoteStatus,
    participation: VoteParticipation,
    result: Option<VoteResult>,
    opened_at: DateTime<Utc>,
    closed_at: Option<DateTime<Utc>>,
    is_historical: bool,
    votes: HashMap<Uuid, VoteChoice> // leave private, temporarily stored
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VoteType {
    Formal {
        raffle_id: Uuid,
        total_eligible_seats: u32,
        threshold: f64,
        counted_points: u32,
        uncounted_points: u32,
    },
    Informal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VoteStatus {
    Open,
    Closed,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Serialize, Deserialize)]
pub enum VoteChoice {
    Yes,
    No,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VoteParticipation {
    Formal {
        counted: Vec<Uuid>,
        uncounted: Vec<Uuid>,
    },
    Informal(Vec<Uuid>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VoteResult {
    Formal {
        counted: VoteCount,
        uncounted: VoteCount,
        passed: bool,
    },
    Informal {
        count: VoteCount,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct VoteCount {
    yes: u32,
    no: u32,
}

impl Vote {
    // Constructor
    pub fn new(
        proposal_id: Uuid,
        epoch_id: Uuid,
        vote_type: VoteType,
        is_historical: bool,
    ) -> Self {
        let participation = match &vote_type {
            VoteType::Formal { .. } => VoteParticipation::Formal { 
                counted: Vec::new(), 
                uncounted: Vec::new() 
            },
            VoteType::Informal => VoteParticipation::Informal(Vec::new()),
        };

        Self {
            id: Uuid::new_v4(),
            proposal_id,
            epoch_id,
            vote_type,
            status: VoteStatus::Open,
            participation,
            result: None,
            opened_at: Utc::now(),
            closed_at: None,
            is_historical,
            votes: HashMap::new(),
        }
    }

    // Getter methods
    pub fn id(&self) -> Uuid { self.id }
    pub fn proposal_id(&self) -> Uuid { self.proposal_id }
    pub fn epoch_id(&self) -> Uuid { self.epoch_id }
    pub fn vote_type(&self) -> &VoteType { &self.vote_type }
    pub fn status(&self) -> &VoteStatus { &self.status }
    pub fn participation(&self) -> &VoteParticipation { &self.participation }
    pub fn result(&self) -> Option<&VoteResult> { self.result.as_ref() }
    pub fn opened_at(&self) -> DateTime<Utc> { self.opened_at }
    pub fn closed_at(&self) -> Option<DateTime<Utc>> { self.closed_at }
    pub fn is_historical(&self) -> bool { self.is_historical }

    pub fn vote_counts(&self) -> Option<(VoteCount, VoteCount)> {
        match &self.result {
            Some(VoteResult::Formal { counted, uncounted, .. }) => Some((*counted, *uncounted)),
            _ => None,
        }
    }

    // Setter methods
    pub fn set_status(&mut self, status: VoteStatus) { self.status = status; }
    pub fn set_result(&mut self, result: Option<VoteResult>) { self.result = result; }
    pub fn set_opened_at(&mut self, date: DateTime<Utc>) { self.opened_at = date; }
    pub fn set_closed_at(&mut self, date: Option<DateTime<Utc>>) { self.closed_at = date; }

    // Core functionality
    pub fn cast_vote(&mut self, team_id: Uuid, choice: VoteChoice, raffle_result: Option<&RaffleResult>) -> Result<(), &'static str> {
        if self.is_closed() {
            return Err("Vote is closed");
        }

        self.votes.insert(team_id, choice);

        match &mut self.participation {
            VoteParticipation::Formal { counted, uncounted } => {
                if let (VoteType::Formal { .. }, Some(raffle_result)) = (&self.vote_type, raffle_result) {
                    if raffle_result.counted().contains(&team_id) {
                        if !counted.contains(&team_id) {
                            counted.push(team_id);
                        }
                    } else if raffle_result.uncounted().contains(&team_id) {
                        if !uncounted.contains(&team_id) {
                            uncounted.push(team_id);
                        }
                    } else {
                        return Err("Team not eligible to vote");
                    }
                } else if raffle_result.is_none() {
                    return Err("Raffle result required for formal votes");
                }
            },
            VoteParticipation::Informal(participants) => {
                if !participants.contains(&team_id) {
                    participants.push(team_id);
                }
            },
        }

        Ok(())
    }

    pub fn close(&mut self) -> Result<(), &'static str> {
        if self.is_closed() {
            return Err("Vote is already closed");
        }

        self.status = VoteStatus::Closed;
        self.closed_at = Some(Utc::now());

        self.calculate_result()?;
        self.votes.clear();

        Ok(())
    }

    pub fn add_participant(&mut self, team_id: Uuid, is_counted: bool) -> Result<(), &'static str> {
        match &mut self.participation {
            VoteParticipation::Formal { counted, uncounted } => {
                if is_counted {
                    if !counted.contains(&team_id) {
                        counted.push(team_id);
                    }
                } else {
                    if !uncounted.contains(&team_id) {
                        uncounted.push(team_id);
                    }
                }
            },
            VoteParticipation::Informal(participants) => {
                if !participants.contains(&team_id) {
                    participants.push(team_id);
                }
            },
        }
        Ok(())
    }

    // Helper methods
    pub fn is_closed(&self) -> bool {
        matches!(self.status, VoteStatus::Closed)
    }

    fn calculate_result(&mut self) -> Result<(), &'static str> {
        self.result = Some(match &self.vote_type {
            VoteType::Formal { total_eligible_seats, threshold, .. } => {
                let (counted, uncounted) = self.count_formal_votes();
                let passed = (counted.yes() as f64 / *total_eligible_seats as f64) >= *threshold;
                VoteResult::Formal { counted, uncounted, passed }
            },
            VoteType::Informal => {
                let count = self.count_informal_votes();
                VoteResult::Informal { count }
            },
        });

        Ok(())
    }

    pub fn count_formal_votes(&self) -> (VoteCount, VoteCount) {
        let mut counted = VoteCount::new();
        let mut uncounted = VoteCount::new();

        if let VoteParticipation::Formal { counted: counted_teams, uncounted: uncounted_teams } = &self.participation {
            for (&team_id, &choice) in &self.votes {
                if counted_teams.contains(&team_id) {
                    match choice {
                        VoteChoice::Yes => counted.increment_yes(),
                        VoteChoice::No => counted.increment_no(),
                    }
                } else if uncounted_teams.contains(&team_id) {
                    match choice {
                        VoteChoice::Yes => uncounted.increment_yes(),
                        VoteChoice::No => uncounted.increment_no(),
                    }
                }
            }
        }

        (counted, uncounted)
    }

    fn count_informal_votes(&self) -> VoteCount {
        let mut count = VoteCount::new();

        for &choice in self.votes.values() {
            match choice {
                VoteChoice::Yes => count.increment_yes(),
                VoteChoice::No => count.increment_no(),
            }
        }

        count
    }

    // pub fn get_result(&self) -> Option<bool> {
    //     self.result.as_ref().map(|r| match r {
    //         VoteResult::Formal { passed, .. } => *passed,
    //         VoteResult::Informal { .. } => false, // Informal votes don't have a pass/fail status
    //     })
    // }



    // pub fn is_vote_count_available(&self) -> bool {
    //     !self.is_historical
    // }
    
}

impl NameMatches for Vote {
    fn name_matches(&self, name: &str) -> bool {
        self.id.to_string() == name
    }
}

impl VoteCount {
    // Constructor
    pub fn new() -> Self {
        Self { yes: 0, no: 0 }
    }

    // Getter methods
    pub fn yes(&self) -> u32 {
        self.yes
    }

    pub fn no(&self) -> u32 {
        self.no
    }

    // Increment methods
    pub fn increment_yes(&mut self) {
        self.yes += 1;
    }

    pub fn increment_no(&mut self) {
        self.no += 1;
    }

    // Helper methods
    pub fn total(&self) -> u32 {
        self.yes + self.no
    }

    pub fn yes_percentage(&self) -> f64 {
        if self.total() == 0 {
            0.0
        } else {
            (self.yes as f64 / self.total() as f64) * 100.0
        }
    }
}

impl Default for VoteCount {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    // Helper function to create a test vote
    fn create_test_vote(vote_type: VoteType) -> Vote {
        Vote::new(
            Uuid::new_v4(),
            Uuid::new_v4(),
            vote_type,
            false
        )
    }

    #[test]
    fn test_vote_creation() {
        let formal_vote = create_test_vote(VoteType::Formal {
            raffle_id: Uuid::new_v4(),
            total_eligible_seats: 10,
            threshold: 0.5,
            counted_points: 2,
            uncounted_points: 1,
        });
        assert!(matches!(formal_vote.vote_type(), VoteType::Formal { .. }));
        assert_eq!(formal_vote.status(), &VoteStatus::Open);

        let informal_vote = create_test_vote(VoteType::Informal);
        assert!(matches!(informal_vote.vote_type(), VoteType::Informal));
        assert_eq!(informal_vote.status(), &VoteStatus::Open);
    }

    #[test]
    fn test_vote_type_and_status() {
        let mut vote = create_test_vote(VoteType::Informal);
        assert!(!vote.is_closed());
        
        vote.set_status(VoteStatus::Closed);
        assert!(vote.is_closed());
    }

    #[test]
    fn test_vote_participation() {
        let mut vote = create_test_vote(VoteType::Formal {
            raffle_id: Uuid::new_v4(),
            total_eligible_seats: 10,
            threshold: 0.5,
            counted_points: 2,
            uncounted_points: 1,
        });

        let team_id = Uuid::new_v4();
        vote.add_participant(team_id, true).unwrap();

        if let VoteParticipation::Formal { counted, uncounted } = vote.participation() {
            assert!(counted.contains(&team_id));
            assert!(!uncounted.contains(&team_id));
        } else {
            panic!("Expected Formal participation");
        }
    }

    #[test]
    fn test_vote_casting() {
        let mut vote = create_test_vote(VoteType::Formal {
            raffle_id: Uuid::new_v4(),
            total_eligible_seats: 10,
            threshold: 0.5,
            counted_points: 2,
            uncounted_points: 1,
        });

        let team_id = Uuid::new_v4();
        let raffle_result = RaffleResult::new(vec![team_id], vec![]);

        vote.cast_vote(team_id, VoteChoice::Yes, Some(&raffle_result)).unwrap();

        let (counted, _) = vote.count_formal_votes();
        assert_eq!(counted.yes(), 1);
        assert_eq!(counted.no(), 0);
    }

    #[test]
    fn test_vote_closing() {
        let mut vote = create_test_vote(VoteType::Informal);
        
        vote.cast_vote(Uuid::new_v4(), VoteChoice::Yes, None).unwrap();
        vote.cast_vote(Uuid::new_v4(), VoteChoice::No, None).unwrap();

        vote.close().unwrap();

        assert!(vote.is_closed());
        assert!(vote.result().is_some());
    }

    #[test]
    fn test_vote_counting() {
        let mut vote = create_test_vote(VoteType::Formal {
            raffle_id: Uuid::new_v4(),
            total_eligible_seats: 10,
            threshold: 0.5,
            counted_points: 2,
            uncounted_points: 1,
        });

        let raffle_result = RaffleResult::new(vec![Uuid::new_v4(), Uuid::new_v4()], vec![Uuid::new_v4()]);

        vote.cast_vote(raffle_result.counted()[0], VoteChoice::Yes, Some(&raffle_result)).unwrap();
        vote.cast_vote(raffle_result.counted()[1], VoteChoice::No, Some(&raffle_result)).unwrap();
        vote.cast_vote(raffle_result.uncounted()[0], VoteChoice::Yes, Some(&raffle_result)).unwrap();

        let (counted, uncounted) = vote.count_formal_votes();
        assert_eq!(counted.yes(), 1);
        assert_eq!(counted.no(), 1);
        assert_eq!(uncounted.yes(), 1);
        assert_eq!(uncounted.no(), 0);
    }

    #[test]
    fn test_vote_results() {
        let mut vote = create_test_vote(VoteType::Formal {
            raffle_id: Uuid::new_v4(),
            total_eligible_seats: 3,
            threshold: 0.5,
            counted_points: 2,
            uncounted_points: 1,
        });

        let raffle_result = RaffleResult::new(vec![Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4()], vec![]);

        vote.cast_vote(raffle_result.counted()[0], VoteChoice::Yes, Some(&raffle_result)).unwrap();
        vote.cast_vote(raffle_result.counted()[1], VoteChoice::Yes, Some(&raffle_result)).unwrap();
        vote.cast_vote(raffle_result.counted()[2], VoteChoice::No, Some(&raffle_result)).unwrap();

        vote.close().unwrap();

        if let Some(VoteResult::Formal { passed, .. }) = vote.result() {
            assert!(passed);
        } else {
            panic!("Expected Formal vote result");
        }
    }

    #[test]
    fn test_edge_cases_and_error_handling() {
        let mut vote = create_test_vote(VoteType::Formal {
            raffle_id: Uuid::new_v4(),
            total_eligible_seats: 3,
            threshold: 0.5,
            counted_points: 2,
            uncounted_points: 1,
        });

        // Attempt to cast vote without raffle result
        assert!(vote.cast_vote(Uuid::new_v4(), VoteChoice::Yes, None).is_err());

        // Attempt to cast vote for ineligible team
        let raffle_result = RaffleResult::new(vec![Uuid::new_v4()], vec![]);
        assert!(vote.cast_vote(Uuid::new_v4(), VoteChoice::Yes, Some(&raffle_result)).is_err());

        // Close the vote
        vote.close().unwrap();

        // Attempt to cast vote after closing
        assert!(vote.cast_vote(raffle_result.counted()[0], VoteChoice::Yes, Some(&raffle_result)).is_err());

        // Attempt to close an already closed vote
        assert!(vote.close().is_err());
    }
}