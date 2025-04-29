// src/core/reporting.rs

use crate::core::models::{Epoch, Proposal, Vote, Team, VoteResult, Resolution};
use crate::core::state::BudgetSystemState;
use chrono::{NaiveDate, Utc, DateTime};
use std::collections::HashMap;
use uuid::Uuid;
use std::error::Error;

// --- Structs for Aggregated Data ---

#[derive(Debug, Default)]
pub struct OverallStats {
    pub total_epochs_included: usize,
    pub num_active_planned: usize,
    pub num_closed: usize,
    pub first_epoch_start_date: Option<DateTime<Utc>>,
    pub latest_epoch_end_date: Option<DateTime<Utc>>, // Might be end date of last closed or current date for active
    pub total_allocated_budget: HashMap<String, f64>,
    pub total_requested_budget: HashMap<String, f64>,
    pub total_paid_budget: HashMap<String, f64>,
    pub total_proposals: usize,
    pub total_resolved_proposals: usize,
    pub total_approved_proposals: usize,
    pub total_paid_proposals: usize,
    pub overall_approval_rate: Option<f64>,
    pub overall_avg_resolution_time_days: Option<f64>,
    pub overall_avg_payment_time_days: Option<f64>,
    pub overall_avg_yes_votes_passed: Option<f64>,
    pub overall_avg_no_votes_rejected: Option<f64>,
    pub total_active_teams_current: usize,
}

#[derive(Debug)]
pub struct EpochStats {
    pub epoch_id: Uuid,
    pub name: String,
    pub status: String, // Planned, Active, Closed
    pub start_date: DateTime<Utc>,
    pub end_date: DateTime<Utc>,
    pub allocated_budget: HashMap<String, f64>,
    pub requested_budget: HashMap<String, f64>,
    pub paid_budget: HashMap<String, f64>,
    pub num_proposals: usize,
    pub num_resolved: usize,
    pub num_approved: usize,
    pub approval_rate: Option<f64>,
    pub avg_resolution_time_days: Option<f64>,
    pub avg_payment_time_days: Option<f64>,
    pub avg_yes_votes_passed: Option<f64>,
    pub avg_no_votes_rejected: Option<f64>,
}

#[derive(Debug)]
pub struct TeamPerformanceSummary {
    pub team_id: Uuid,
    pub team_name: String,
    pub current_status: String,
    pub total_proposals_submitted: usize,
    pub total_proposals_approved: usize,
    pub total_budget_paid: HashMap<String, f64>,
    pub total_points_earned: u32,
}

#[derive(Debug)]
pub struct PaidFundingData {
    // Token -> Epoch ID -> Team ID -> Amount
    pub funding: HashMap<String, HashMap<Uuid, HashMap<Uuid, f64>>>,
    pub team_totals: HashMap<String, HashMap<Uuid, f64>>, // Token -> Team ID -> Total Amount
    pub epoch_totals: HashMap<String, HashMap<Uuid, f64>>, // Token -> Epoch ID -> Total Amount
    pub grand_totals: HashMap<String, f64>, // Token -> Grand Total Amount
}

// Placeholder for formatting functions (Step 5)
pub fn format_report(
    stats: OverallStats,
    epoch_stats: Vec<EpochStats>,
    team_stats: Vec<TeamPerformanceSummary>,
    paid_funding: PaidFundingData,
    scope: &str,
) -> String {
    // TODO: Implement Markdown formatting in Step 5
    format!(
        "# All Epochs Summary Report ({})\n\n**Generated:** {}\n\n*Data Aggregated. Formatting pending Step 5.*\n\nOverall Stats: {:?}\n\nEpoch Stats: {:?}\n\nTeam Stats: {:?}\n\nPaid Funding: {:?}",
        scope,
        Utc::now().to_rfc3339(),
        stats,
        epoch_stats,
        team_stats,
        paid_funding,
    )
}

// --- End Structs ---

// Helper function to safely calculate averages
fn calculate_average(sum: f64, count: usize) -> Option<f64> {
    if count > 0 {
        Some(sum / count as f64)
    } else {
        None
    }
}

// Helper to calculate duration in days
fn calculate_days_between(start_opt: Option<NaiveDate>, end_opt: Option<NaiveDate>) -> Option<i64> {
    match (start_opt, end_opt) {
        (Some(start), Some(end)) if end >= start => Some((end - start).num_days()),
        _ => None,
    }
}

/// Selects epochs based on the filter criteria and sorts them.
pub fn select_epochs<'a>(state: &'a BudgetSystemState, only_closed: bool) -> Vec<&'a Epoch> {
    let mut selected: Vec<&Epoch> = state.epochs().values()
        .filter(|epoch| !only_closed || epoch.is_closed())
        .collect();
    selected.sort_by_key(|epoch| epoch.start_date());
    selected
}

/// Gathers proposals relevant to the selected epochs.
pub fn get_relevant_proposals<'a>(state: &'a BudgetSystemState, selected_epoch_ids: &[Uuid]) -> Vec<&'a Proposal> {
    let epoch_id_set: std::collections::HashSet<Uuid> = selected_epoch_ids.iter().cloned().collect();
    state.proposals().values()
        .filter(|proposal| epoch_id_set.contains(&proposal.epoch_id()))
        .collect()
}

/// Gathers votes relevant to the selected proposals.
pub fn get_relevant_votes<'a>(state: &'a BudgetSystemState, relevant_proposal_ids: &[Uuid]) -> Vec<&'a Vote> {
    let proposal_id_set: std::collections::HashSet<Uuid> = relevant_proposal_ids.iter().cloned().collect();
    state.votes().values()
        .filter(|vote| proposal_id_set.contains(&vote.proposal_id()))
        .collect()
}

/// Calculates the overall summary statistics for Section I.
pub fn calculate_overall_summary_stats(
    state: &BudgetSystemState,
    selected_epochs: &[&Epoch],
    relevant_proposals: &[&Proposal],
    relevant_votes: &[&Vote],
) -> OverallStats {
    let mut stats = OverallStats::default();
    stats.total_epochs_included = selected_epochs.len();

    let mut total_resolution_time_days_sum = 0.0;
    let mut resolved_proposal_count_for_avg = 0;
    let mut total_payment_time_days_sum = 0.0;
    let mut paid_proposal_count_for_avg = 0;
    let mut total_yes_votes_passed_sum = 0.0;
    let mut passed_formal_vote_count = 0;
    let mut total_no_votes_rejected_sum = 0.0;
    let mut rejected_formal_vote_count = 0;

    for epoch in selected_epochs {
        match epoch.status() {
            crate::core::models::EpochStatus::Closed => stats.num_closed += 1,
            _ => stats.num_active_planned += 1,
        }

        if let Some(reward) = epoch.reward() {
            *stats.total_allocated_budget.entry(reward.token().to_string()).or_insert(0.0) += reward.amount();
        }

        if stats.first_epoch_start_date.is_none() || epoch.start_date() < stats.first_epoch_start_date.unwrap() {
            stats.first_epoch_start_date = Some(epoch.start_date());
        }
        if stats.latest_epoch_end_date.is_none() || epoch.end_date() > stats.latest_epoch_end_date.unwrap() {
             stats.latest_epoch_end_date = Some(epoch.end_date());
        }
    }

    for proposal in relevant_proposals {
        stats.total_proposals += 1;
        let is_resolved = proposal.resolution().is_some();
        let is_approved = proposal.is_approved();
        let is_paid = proposal.budget_request_details().map_or(false, |d| d.is_paid());

        if is_resolved {
            stats.total_resolved_proposals += 1;
        }
        if is_approved {
            stats.total_approved_proposals += 1;
            if let Some(details) = proposal.budget_request_details() {
                for (token, amount) in details.request_amounts() {
                    *stats.total_requested_budget.entry(token.clone()).or_insert(0.0) += amount;
                }
                if is_paid {
                     stats.total_paid_proposals += 1;
                    for (token, amount) in details.request_amounts() {
                        *stats.total_paid_budget.entry(token.clone()).or_insert(0.0) += amount;
                    }
                     // Payment time calculation
                     if let Some(days) = calculate_days_between(proposal.resolved_at(), details.payment_date()) {
                        total_payment_time_days_sum += days as f64;
                        paid_proposal_count_for_avg += 1;
                    }
                }
            }
        }

        // Resolution time calculation
        let start_date = proposal.published_at().or(proposal.announced_at());
        if is_resolved {
            if let Some(days) = calculate_days_between(start_date, proposal.resolved_at()) {
                total_resolution_time_days_sum += days as f64;
                resolved_proposal_count_for_avg += 1;
            }
        }

        // Voting average calculations
        if let Some(vote) = relevant_votes.iter().find(|v| v.proposal_id() == proposal.id()) {
            if let Some(VoteResult::Formal{ counted, .. }) = vote.result() {
                if is_approved {
                    total_yes_votes_passed_sum += counted.yes() as f64;
                    passed_formal_vote_count += 1;
                } else if proposal.is_rejected() { // Only count 'No' votes if the proposal was actually Rejected
                    total_no_votes_rejected_sum += counted.no() as f64;
                    rejected_formal_vote_count += 1;
                }
            }
        }
    }

    stats.overall_approval_rate = calculate_average(stats.total_approved_proposals as f64 * 100.0, stats.total_resolved_proposals);
    stats.overall_avg_resolution_time_days = calculate_average(total_resolution_time_days_sum, resolved_proposal_count_for_avg);
    stats.overall_avg_payment_time_days = calculate_average(total_payment_time_days_sum, paid_proposal_count_for_avg);
    stats.overall_avg_yes_votes_passed = calculate_average(total_yes_votes_passed_sum, passed_formal_vote_count);
    stats.overall_avg_no_votes_rejected = calculate_average(total_no_votes_rejected_sum, rejected_formal_vote_count);

    stats.total_active_teams_current = state.current_state().teams().values().filter(|t| t.is_active()).count();

    stats
}


/// Calculates the statistics for each individual epoch.
pub fn calculate_epoch_by_epoch_stats(
    state: &BudgetSystemState,
    selected_epochs: &[&Epoch],
    relevant_proposals: &[&Proposal],
    relevant_votes: &[&Vote],
) -> Vec<EpochStats> {
    selected_epochs.iter().map(|epoch| {
        let epoch_proposals: Vec<&&Proposal> = relevant_proposals.iter()
            .filter(|p| p.epoch_id() == epoch.id())
            .collect();

        let epoch_votes: Vec<&&Vote> = relevant_votes.iter()
            .filter(|v| v.epoch_id() == epoch.id())
            .collect();

        let mut requested_budget = HashMap::new();
        let mut paid_budget = HashMap::new();
        let mut num_resolved = 0;
        let mut num_approved = 0;
        let mut total_resolution_time_days_sum = 0.0;
        let mut resolved_proposal_count_for_avg = 0;
        let mut total_payment_time_days_sum = 0.0;
        let mut paid_proposal_count_for_avg = 0;
        let mut total_yes_votes_passed_sum = 0.0;
        let mut passed_formal_vote_count = 0;
        let mut total_no_votes_rejected_sum = 0.0;
        let mut rejected_formal_vote_count = 0;


        for proposal in &epoch_proposals {
            let is_resolved = proposal.resolution().is_some();
            let is_approved = proposal.is_approved();
            let is_paid = proposal.budget_request_details().map_or(false, |d| d.is_paid());

            if is_resolved {
                num_resolved += 1;
            }
            if is_approved {
                num_approved += 1;
                if let Some(details) = proposal.budget_request_details() {
                    for (token, amount) in details.request_amounts() {
                        *requested_budget.entry(token.clone()).or_insert(0.0) += amount;
                    }
                    if is_paid {
                        for (token, amount) in details.request_amounts() {
                            *paid_budget.entry(token.clone()).or_insert(0.0) += amount;
                        }
                         // Payment time calculation
                         if let Some(days) = calculate_days_between(proposal.resolved_at(), details.payment_date()) {
                            total_payment_time_days_sum += days as f64;
                            paid_proposal_count_for_avg += 1;
                        }
                    }
                }
            }

             // Resolution time calculation
            let start_date = proposal.published_at().or(proposal.announced_at());
             if is_resolved {
                 if let Some(days) = calculate_days_between(start_date, proposal.resolved_at()) {
                    total_resolution_time_days_sum += days as f64;
                    resolved_proposal_count_for_avg += 1;
                }
             }

             // Voting average calculations
            if let Some(vote) = epoch_votes.iter().find(|v| v.proposal_id() == proposal.id()) {
                if let Some(VoteResult::Formal{ counted, .. }) = vote.result() {
                    if is_approved {
                        total_yes_votes_passed_sum += counted.yes() as f64;
                        passed_formal_vote_count += 1;
                    } else if proposal.is_rejected() {
                        total_no_votes_rejected_sum += counted.no() as f64;
                        rejected_formal_vote_count += 1;
                    }
                }
            }
        }

        let allocated_budget = epoch.reward().map_or_else(HashMap::new, |r| {
            HashMap::from([(r.token().to_string(), r.amount())])
        });

        EpochStats {
            epoch_id: epoch.id(),
            name: epoch.name().to_string(),
            status: format!("{:?}", epoch.status()),
            start_date: epoch.start_date(),
            end_date: epoch.end_date(),
            allocated_budget,
            requested_budget,
            paid_budget,
            num_proposals: epoch_proposals.len(),
            num_resolved,
            num_approved,
            approval_rate: calculate_average(num_approved as f64 * 100.0, num_resolved),
            avg_resolution_time_days: calculate_average(total_resolution_time_days_sum, resolved_proposal_count_for_avg),
            avg_payment_time_days: calculate_average(total_payment_time_days_sum, paid_proposal_count_for_avg),
            avg_yes_votes_passed: calculate_average(total_yes_votes_passed_sum, passed_formal_vote_count),
            avg_no_votes_rejected: calculate_average(total_no_votes_rejected_sum, rejected_formal_vote_count),
        }
    }).collect()
}


/// Calculates the team performance summary for Section III.
pub fn calculate_team_performance_summary(
    state: &BudgetSystemState,
    selected_epochs: &[&Epoch],
    relevant_proposals: &[&Proposal],
    // ADD: Parameter for pre-calculated points
    team_total_points: &HashMap<Uuid, u32>,
) -> Vec<TeamPerformanceSummary> {
    let mut team_summaries = Vec::new();

    // Iterate through teams from the current state
    for (team_id, team) in state.current_state().teams() {
        let mut total_proposals_submitted = 0;
        let mut total_proposals_approved = 0;
        let mut total_budget_paid = HashMap::new();

        // Calculate proposal stats
        for proposal in relevant_proposals {
            // Check if proposal belongs to one of the selected epochs
            if selected_epochs.iter().any(|e| e.id() == proposal.epoch_id()) {
                if proposal.budget_request_details().map_or(false, |d| d.team() == Some(*team_id)) {
                    total_proposals_submitted += 1;
                    if proposal.is_approved() {
                        total_proposals_approved += 1;
                        if let Some(details) = proposal.budget_request_details() {
                             if details.is_paid() {
                                for (token, amount) in details.request_amounts() {
                                    *total_budget_paid.entry(token.clone()).or_insert(0.0) += amount;
                                }
                            }
                        }
                    }
                }
            }
        }

        let total_points_earned = *team_total_points.get(team_id).unwrap_or(&0);

        team_summaries.push(TeamPerformanceSummary {
            team_id: *team_id,
            team_name: team.name().to_string(),
            current_status: format!("{:?}", team.status()), // Get current status from state
            total_proposals_submitted,
            total_proposals_approved,
            total_budget_paid,
            total_points_earned,
        });
    }

    team_summaries.sort_by(|a, b| a.team_name.cmp(&b.team_name));
    team_summaries
}

/// Calculates the paid funding per team per epoch for Section IV.
pub fn calculate_paid_funding_per_team_epoch(
    state: &BudgetSystemState,
    selected_epochs: &[&Epoch],
    relevant_proposals: &[&Proposal],
) -> PaidFundingData {
    let mut funding: HashMap<String, HashMap<Uuid, HashMap<Uuid, f64>>> = HashMap::new();
    let mut team_totals: HashMap<String, HashMap<Uuid, f64>> = HashMap::new();
    let mut epoch_totals: HashMap<String, HashMap<Uuid, f64>> = HashMap::new();
    let mut grand_totals: HashMap<String, f64> = HashMap::new();

    let selected_epoch_ids: Vec<Uuid> = selected_epochs.iter().map(|e| e.id()).collect();

    for proposal in relevant_proposals {
        if proposal.is_approved() {
            if let Some(details) = proposal.budget_request_details() {
                 if details.is_paid() {
                     if let Some(team_id) = details.team() {
                        for (token, amount) in details.request_amounts() {
                             if *amount > 0.0 { // Only record non-zero payments
                                let epoch_id = proposal.epoch_id();

                                // Ensure epoch is selected
                                if selected_epoch_ids.contains(&epoch_id) {
                                    // Per Team/Epoch/Token
                                    *funding
                                        .entry(token.clone()).or_default()
                                        .entry(epoch_id).or_default()
                                        .entry(team_id).or_insert(0.0) += amount;

                                    // Team Totals
                                    *team_totals
                                        .entry(token.clone()).or_default()
                                        .entry(team_id).or_insert(0.0) += amount;

                                    // Epoch Totals
                                    *epoch_totals
                                        .entry(token.clone()).or_default()
                                        .entry(epoch_id).or_insert(0.0) += amount;

                                    // Grand Totals
                                    *grand_totals
                                        .entry(token.clone()).or_insert(0.0) += amount;
                                }
                            }
                        }
                     }
                 }
            }
        }
    }

    PaidFundingData { funding, team_totals, epoch_totals, grand_totals }
}