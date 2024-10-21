use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::core::models::{ PaymentStatus, VoteChoice };

// #[derive(Debug, Deserialize)]
// struct RawCommand {
//     #[serde(rename = "type")]
//     command_type: String,
//     params: serde_json::Value,
// }

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "params")]
pub enum Command {
    CreateEpoch {
        name: String,
        start_date:
        DateTime<Utc>,
        end_date: DateTime<Utc>,
    },
    ActivateEpoch {
        name: String
    },
    SetEpochReward {
        token: String,
        amount: f64,
    },
    AddTeam {
        name: String,
        representative: String,
        trailing_monthly_revenue: Option<Vec<u64>>,
    },
    UpdateTeam {
        team_name: String,
        updates: UpdateTeamDetails,
    },
    AddProposal {
        title: String,
        url: Option<String>,
        budget_request_details: Option<BudgetRequestDetailsCommand>,
        announced_at: Option<NaiveDate>,
        published_at: Option<NaiveDate>,
        is_historical: Option<bool>,
    },
    UpdateProposal {
        proposal_name: String,
        updates: UpdateProposalDetails,
    },
    ImportPredefinedRaffle {
        proposal_name: String,
        counted_teams: Vec<String>,
        uncounted_teams: Vec<String>,
        total_counted_seats: usize,
        max_earner_seats: usize,
    },
    ImportHistoricalVote {
        proposal_name: String,
        passed: bool,
        participating_teams: Vec<String>,
        non_participating_teams: Vec<String>,
        counted_points: Option<u32>,
        uncounted_points: Option<u32>,
    },
    ImportHistoricalRaffle {
        proposal_name: String,
        initiation_block: u64,
        randomness_block: u64,
        team_order: Option<Vec<String>>,
        excluded_teams: Option<Vec<String>>,
        total_counted_seats: Option<usize>,
        max_earner_seats: Option<usize>,
    },
    PrintTeamReport,
    PrintEpochState,
    PrintTeamVoteParticipation {
        team_name: String,
        epoch_name: Option<String> 
    },
    CloseProposal {
        proposal_name: String,
        resolution: String,
    },
    CreateRaffle {
        proposal_name: String,
        block_offset: Option<u64>,
        excluded_teams: Option<Vec<String>>,
    },
    CreateAndProcessVote {
        proposal_name: String,
        counted_votes: HashMap<String, VoteChoice>,
        uncounted_votes: HashMap<String, VoteChoice>,
        vote_opened: Option<NaiveDate>,
        vote_closed: Option<NaiveDate>,
    },
    GenerateReportsForClosedProposals {
        epoch_name: String
    },
    GenerateReportForProposal {
        proposal_name: String
    },
    PrintPointReport {
        epoch_name: Option<String>
     },
    CloseEpoch {
        epoch_name: Option<String>
    },
    GenerateEndOfEpochReport {
        epoch_name: String
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTeamDetails {
    pub name: Option<String>,
    pub representative: Option<String>,
    pub status: Option<String>,
    pub trailing_monthly_revenue: Option<Vec<u64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BudgetRequestDetailsCommand {
    pub team: Option<String>,
    pub request_amounts: Option<HashMap<String, f64>>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub payment_status: Option<PaymentStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProposalDetails {
    pub title: Option<String>,
    pub url: Option<String>,
    pub budget_request_details: Option<BudgetRequestDetailsCommand>,
    pub announced_at: Option<NaiveDate>,
    pub published_at: Option<NaiveDate>,
    pub resolved_at: Option<NaiveDate>,
}

pub trait CommandExecutor {
    fn execute_command(&mut self, command: Command) -> Result<String, Box<dyn std::error::Error>>;
}