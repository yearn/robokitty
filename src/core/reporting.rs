// src/core/reporting.rs

use crate::core::models::{Epoch, Proposal, Vote, Team, TeamStatus, VoteResult, Resolution};
use crate::core::state::BudgetSystemState;
use chrono::{NaiveDate, Utc, DateTime};
use std::collections::HashMap;
use uuid::Uuid;
use std::error::Error;
use itertools::Itertools;

// --- Structs for Aggregated Data ---

#[derive(Debug, Default)]
pub struct OverallStats {
    pub total_epochs_included: usize,
    pub num_active_planned: usize,
    pub num_closed: usize,
    pub first_epoch_start_date: Option<DateTime<Utc>>,
    pub latest_epoch_end_date: Option<DateTime<Utc>>, // Might be end date of last closed or current date for active
    pub total_allocated_budget: HashMap<String, f64>,
    pub total_requested_budget: HashMap<String, f64>, // Non-loan requested
    pub total_paid_budget: HashMap<String, f64>,       // Non-loan paid
    pub total_loan_requested_budget: HashMap<String, f64>, // Loan requested (approved)
    pub total_loan_paid_budget: HashMap<String, f64>,      // Loan paid
    pub total_proposals: usize,
    pub total_resolved_proposals: usize,
    pub total_approved_proposals: usize, // Includes both funding and loan approvals
    pub total_paid_proposals: usize,     // Non-loan paid
    pub total_paid_loans: usize,         // Loan paid count
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
    pub requested_budget: HashMap<String, f64>, // Remains total approved (funding + loan) for now, or split if needed later
    pub paid_funding_budget: HashMap<String, f64>,
    pub paid_loans_budget: HashMap<String, f64>,
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
    pub total_funding_paid: HashMap<String, f64>,
    pub total_loans_paid: HashMap<String, f64>,
    pub total_points_earned: u32,
}

#[derive(Debug, Default, Clone)]
pub struct PaidFundingData {
    // Token -> Epoch ID -> Team ID -> Amount
    pub funding: HashMap<String, HashMap<Uuid, HashMap<Uuid, f64>>>,
    pub team_totals: HashMap<String, HashMap<Uuid, f64>>, // Token -> Team ID -> Total Amount
    pub epoch_totals: HashMap<String, HashMap<Uuid, f64>>, // Token -> Epoch ID -> Total Amount
    pub grand_totals: HashMap<String, f64>, // Token -> Grand Total Amount
}

/// Formats the complete All Epochs Summary report.
pub fn format_report(
    stats: OverallStats,
    epoch_stats: Vec<EpochStats>,
    team_stats: Vec<TeamPerformanceSummary>,
    paid_funding_data: PaidFundingData,
    paid_loan_data: PaidFundingData,
    scope: &str,
    // Pass necessary state components for formatting section IV
    teams: &HashMap<Uuid, Team>,
    selected_epochs: &[&Epoch],
) -> String {
    let mut report = String::new();

    report.push_str(&format!("# RoboKitty Budget System - All Epochs Summary Report\n\n"));
    report.push_str(&format!("**Generated:** {}\n\n", Utc::now().format("%Y-%m-%d %H:%M:%S UTC")));
    if scope == "All Epochs" {
         report.push_str("This report summarizes key financial, performance, and voting metrics across all relevant epochs managed by the RoboKitty budget system. By default, all epochs (Active and Closed) are included. Use the `--only-closed` flag to view data for completed epochs only.\n\n");
    } else {
         report.push_str("This report summarizes key financial, performance, and voting metrics across all **completed (Closed)** epochs managed by the RoboKitty budget system.\n\n");
    }
    
    report.push_str(&format!("*Note: '{}' category aggregates the following tokens: {}.*\n", STABLES_KEY, STABLECOINS.join(", ")));
    report.push_str("---\n\n");

    // Append sections
    report.push_str(&format_section_i(&stats, scope));
    report.push_str(&format_section_ii(&epoch_stats, scope));
    report.push_str(&format_section_iii(&team_stats, scope));
    // --- Pass split data to Section IV formatter ---
    report.push_str(&format_section_iv(
        &paid_funding_data, // Pass funding
        &paid_loan_data,    // Pass loans
        selected_epochs,
        teams,
        scope,
    ));

    report
}

// --- End Structs ---

// --- Stablecoin Definition ---
const STABLECOINS: [&str; 4] = ["DAI", "USDC", "USD", "yv-mkUSD"];
const STABLES_KEY: &str = "Stables";

fn is_stablecoin(token: &str) -> bool {
    STABLECOINS.contains(&token.to_uppercase().as_str())
}

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
        // Apply stablecoin grouping to allocated budget
        if let Some(reward) = epoch.reward() {
            let token_key = if is_stablecoin(reward.token()) { STABLES_KEY.to_string() } else { reward.token().to_string() };
            *stats.total_allocated_budget.entry(token_key).or_insert(0.0) += reward.amount();
        }
        // Dates logic remains the same...
        if stats.first_epoch_start_date.is_none() || epoch.start_date() < stats.first_epoch_start_date.unwrap() {
            stats.first_epoch_start_date = Some(epoch.start_date());
        }
        if stats.latest_epoch_end_date.is_none() || epoch.end_date() > stats.latest_epoch_end_date.unwrap() {
             stats.latest_epoch_end_date = Some(epoch.end_date());
        }
    }


    // Reset accumulators used inside the proposal loop
    total_resolution_time_days_sum = 0.0;
    resolved_proposal_count_for_avg = 0;
    total_payment_time_days_sum = 0.0; // This will now track funding payment time
    paid_proposal_count_for_avg = 0; // Tracks paid funding proposals
    // No need for separate loan payment time average for now based on requirements
    total_yes_votes_passed_sum = 0.0;
    passed_formal_vote_count = 0;
    total_no_votes_rejected_sum = 0.0;
    rejected_formal_vote_count = 0;

    for proposal in relevant_proposals {
        stats.total_proposals += 1;
        let is_resolved = proposal.resolution().is_some();
        let is_approved = proposal.is_approved();
        let is_paid = proposal.budget_request_details().map_or(false, |d| d.is_paid());
        let is_loan = proposal.budget_request_details().map_or(false, |d| d.is_loan());

        if is_resolved {
            stats.total_resolved_proposals += 1;
        }
        if is_approved {
            stats.total_approved_proposals += 1; // Count all approvals
            if let Some(details) = proposal.budget_request_details() {
                for (token, amount) in details.request_amounts() {
                    let token_key = if is_stablecoin(token) { STABLES_KEY.to_string() } else { token.clone() };
                    // Requested budget (split loan/funding)
                    if is_loan {
                         *stats.total_loan_requested_budget.entry(token_key.clone()).or_insert(0.0) += amount;
                    } else {
                        *stats.total_requested_budget.entry(token_key.clone()).or_insert(0.0) += amount;
                    }

                    // Paid budget (split loan/funding)
                    if is_paid {
                        if is_loan {
                            *stats.total_loan_paid_budget.entry(token_key).or_insert(0.0) += amount;
                            // We also need a count of paid loans
                        } else {
                            *stats.total_paid_budget.entry(token_key).or_insert(0.0) += amount;
                            // We also need a count of paid funding proposals
                        }
                    }
                }
                // Count paid items separately
                if is_paid {
                    if is_loan {
                        stats.total_paid_loans += 1;
                    } else {
                        stats.total_paid_proposals += 1; // Rename this? This now means non-loan paid proposals.
                         // Payment time calculation (only for non-loan funding)
                        if let Some(days) = calculate_days_between(proposal.resolved_at(), details.payment_date()) {
                            total_payment_time_days_sum += days as f64;
                            paid_proposal_count_for_avg += 1;
                        }
                    }
                }
            }
        }

        // Resolution time calculation (remains the same)
        let start_date = proposal.published_at().or(proposal.announced_at());
        if is_resolved {
            if let Some(days) = calculate_days_between(start_date, proposal.resolved_at()) {
                total_resolution_time_days_sum += days as f64;
                resolved_proposal_count_for_avg += 1;
            }
        }

        // Voting average calculations (remains the same)
        if let Some(vote) = relevant_votes.iter().find(|v| v.proposal_id() == proposal.id()) {
            // ... (logic for voting averages remains the same) ...
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

    // Final average calculations (remain the same logic, but use updated counts/sums)
    stats.overall_approval_rate = calculate_average(stats.total_approved_proposals as f64 * 100.0, stats.total_resolved_proposals);
    stats.overall_avg_resolution_time_days = calculate_average(total_resolution_time_days_sum, resolved_proposal_count_for_avg);
    stats.overall_avg_payment_time_days = calculate_average(total_payment_time_days_sum, paid_proposal_count_for_avg); // Avg time for non-loan funding
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

        let mut requested_budget = HashMap::new(); // Still total approved requests for now
        let mut paid_funding_budget = HashMap::new(); // Renamed
        let mut paid_loans_budget = HashMap::new(); // New
        let mut num_resolved = 0;
        let mut num_approved = 0;
        let mut total_resolution_time_days_sum = 0.0;
        let mut resolved_proposal_count_for_avg = 0;
        let mut total_payment_time_days_sum = 0.0; // For funding
        let mut paid_proposal_count_for_avg = 0; // For funding
        let mut total_yes_votes_passed_sum = 0.0;
        let mut passed_formal_vote_count = 0;
        let mut total_no_votes_rejected_sum = 0.0;
        let mut rejected_formal_vote_count = 0;


        for proposal in &epoch_proposals {
            let is_resolved = proposal.resolution().is_some();
            let is_approved = proposal.is_approved();
            let is_paid = proposal.budget_request_details().map_or(false, |d| d.is_paid());
            let is_loan = proposal.budget_request_details().map_or(false, |d| d.is_loan());

            if is_resolved {
                num_resolved += 1;
            }
            if is_approved {
                num_approved += 1;
                if let Some(details) = proposal.budget_request_details() {
                    // Calculate total requested (could be split later if needed)
                    for (token, amount) in details.request_amounts() {
                         let token_key = if is_stablecoin(token) { STABLES_KEY.to_string() } else { token.clone() };
                        *requested_budget.entry(token_key).or_insert(0.0) += amount;
                    }

                    // Calculate paid funding vs paid loans
                    if is_paid {
                         for (token, amount) in details.request_amounts() {
                            let token_key = if is_stablecoin(token) { STABLES_KEY.to_string() } else { token.clone() };
                            if is_loan {
                                *paid_loans_budget.entry(token_key).or_insert(0.0) += amount;
                            } else {
                                *paid_funding_budget.entry(token_key).or_insert(0.0) += amount;
                                // Payment time calculation (only for non-loan funding)
                                if let Some(days) = calculate_days_between(proposal.resolved_at(), details.payment_date()) {
                                    total_payment_time_days_sum += days as f64;
                                    paid_proposal_count_for_avg += 1;
                                }
                            }
                        }
                    }
                }
            }

            // Resolution time calculation (remains the same)
             let start_date = proposal.published_at().or(proposal.announced_at());
             if is_resolved {
                 if let Some(days) = calculate_days_between(start_date, proposal.resolved_at()) {
                    total_resolution_time_days_sum += days as f64;
                    resolved_proposal_count_for_avg += 1;
                }
             }

            // Voting average calculations (remains the same)
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

        // Apply stablecoin grouping to allocated budget
        let allocated_budget = epoch.reward().map_or_else(HashMap::new, |r| {
            let token_key = if is_stablecoin(r.token()) { STABLES_KEY.to_string() } else { r.token().to_string() };
             HashMap::from([(token_key, r.amount())])
        });


        EpochStats {
            epoch_id: epoch.id(),
            name: epoch.name().to_string(),
            status: format!("{:?}", epoch.status()),
            start_date: epoch.start_date(),
            end_date: epoch.end_date(),
            allocated_budget,
            requested_budget, // Still total approved requests
            paid_funding_budget, // Renamed
            paid_loans_budget, // Added
            num_proposals: epoch_proposals.len(),
            num_resolved,
            num_approved,
            approval_rate: calculate_average(num_approved as f64 * 100.0, num_resolved),
            avg_resolution_time_days: calculate_average(total_resolution_time_days_sum, resolved_proposal_count_for_avg),
            avg_payment_time_days: calculate_average(total_payment_time_days_sum, paid_proposal_count_for_avg), // Avg time for funding
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
    team_total_points: &HashMap<Uuid, u32>,
) -> Vec<TeamPerformanceSummary> {
    let mut team_summaries = Vec::new();

    for (team_id, team) in state.current_state().teams() {
        let mut total_proposals_submitted = 0;
        let mut total_proposals_approved = 0;
        let mut total_funding_paid = HashMap::new(); // Renamed
        let mut total_loans_paid = HashMap::new();   // New

        for proposal in relevant_proposals {
            // Check if proposal belongs to one of the selected epochs
            if selected_epochs.iter().any(|e| e.id() == proposal.epoch_id()) {
                if let Some(details) = proposal.budget_request_details() {
                    if details.team() == Some(*team_id) {
                        total_proposals_submitted += 1;
                        if proposal.is_approved() {
                            total_proposals_approved += 1;
                             if details.is_paid() {
                                 for (token, amount) in details.request_amounts() {
                                    let token_key = if is_stablecoin(token) { STABLES_KEY.to_string() } else { token.clone() };
                                    if details.is_loan() {
                                        *total_loans_paid.entry(token_key).or_insert(0.0) += amount;
                                    } else {
                                         *total_funding_paid.entry(token_key).or_insert(0.0) += amount;
                                    }
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
            current_status: format_team_status_clean(team.status()), // Use clean formatter
            total_proposals_submitted,
            total_proposals_approved,
            total_funding_paid, // Renamed
            total_loans_paid, // Added
            total_points_earned,
        });
    }

    team_summaries.sort_by(|a, b| a.team_name.cmp(&b.team_name));
    team_summaries
}

/// Calculates the paid funding per team per epoch for Section IV.
/// Returns a tuple: (funding_data, loan_data)
pub fn calculate_paid_funding_per_team_epoch(
    state: &BudgetSystemState,
    selected_epochs: &[&Epoch],
    relevant_proposals: &[&Proposal],
) -> (PaidFundingData, PaidFundingData) {
    let mut funding_data = PaidFundingData::default();
    let mut loan_data = PaidFundingData::default();

    let selected_epoch_ids: Vec<Uuid> = selected_epochs.iter().map(|e| e.id()).collect();

    for proposal in relevant_proposals {
        if proposal.is_approved() {
            if let Some(details) = proposal.budget_request_details() {
                 if details.is_paid() {
                     if let Some(team_id) = details.team() {
                        let epoch_id = proposal.epoch_id();
                        // Ensure epoch is selected
                        if selected_epoch_ids.contains(&epoch_id) {
                             for (token, amount) in details.request_amounts() {
                                 if *amount > 0.0 {
                                    let token_key = if is_stablecoin(token) { STABLES_KEY.to_string() } else { token.clone() };
                                    let is_loan = details.is_loan();

                                    // Choose which data structure to update
                                    let target_data = if is_loan { &mut loan_data } else { &mut funding_data };

                                    // Per Team/Epoch/Token
                                    *target_data.funding
                                        .entry(token_key.clone()).or_default()
                                        .entry(epoch_id).or_default()
                                        .entry(team_id).or_insert(0.0) += amount;

                                    // Team Totals
                                    *target_data.team_totals
                                        .entry(token_key.clone()).or_default()
                                        .entry(team_id).or_insert(0.0) += amount;

                                    // Epoch Totals
                                    *target_data.epoch_totals
                                        .entry(token_key.clone()).or_default()
                                        .entry(epoch_id).or_insert(0.0) += amount;

                                    // Grand Totals
                                    *target_data.grand_totals
                                        .entry(token_key).or_insert(0.0) += amount;
                                }
                            }
                        }
                     }
                 }
            }
        }
    }

    (funding_data, loan_data)
}

/// Formats TeamStatus cleanly.
fn format_team_status_clean(status: &TeamStatus) -> String {
    match status {
        TeamStatus::Earner { .. } => "Earner".to_string(),
        TeamStatus::Supporter => "Supporter".to_string(),
        TeamStatus::Inactive => "Inactive".to_string(),
    }
}

/// Formats f64 as currency string with commas and 2 decimal places.
fn format_currency(amount: f64) -> String {
    let formatted = format!("{:.2}", amount); // Format to 2 decimal places first
    let parts: Vec<&str> = formatted.split('.').collect();
    let integer_part = parts[0];
    let decimal_part = if parts.len() > 1 { parts[1] } else { "00" };

    let mut integer_with_commas = String::new();
    let integer_len = integer_part.len();
    for (i, digit) in integer_part.chars().enumerate() {
        integer_with_commas.push(digit);
        let pos_from_end = integer_len - 1 - i;
        if pos_from_end > 0 && pos_from_end % 3 == 0 {
            integer_with_commas.push(',');
        }
    }

    format!("{}.{}", integer_with_commas, decimal_part)
}

/// Formats a map of tokens and amounts, grouping stables and using new number format.
fn format_token_amounts_grouped(amounts: &HashMap<String, f64>) -> String {
    if amounts.is_empty() {
        return "N/A".to_string();
    }
    // Group stables
    let mut grouped = HashMap::new();
    let mut stable_total = 0.0;
    for (token, amount) in amounts {
        if is_stablecoin(token) {
            stable_total += amount;
        } else {
            *grouped.entry(token.clone()).or_insert(0.0) += amount;
        }
    }
    if stable_total != 0.0 {
        grouped.insert(STABLES_KEY.to_string(), stable_total);
    }

    if grouped.is_empty() {
         return "N/A".to_string(); // Possible if only zero-value stables existed
    }

    grouped.iter()
        .sorted_by_key(|(token, _)| *token)
        .map(|(token, amount)| format!("{}: {}", token, format_currency(*amount)))
        .join(", ")
}

/// Formats a map of tokens and amounts into a string (e.g., "ETH: 10.50, USD: 5000.00")
fn format_token_amounts(amounts: &HashMap<String, f64>) -> String {
    if amounts.is_empty() {
        return "N/A".to_string();
    }
    amounts.iter()
        .sorted_by_key(|(token, _)| *token) // Sort for consistent output
        .map(|(token, amount)| format!("{}: {:.2}", token, amount))
        .join(", ")
}

/// Formats an optional f64, often used for averages or rates.
fn format_optional_f64(value: Option<f64>, suffix: &str) -> String {
    value.map_or("N/A".to_string(), |v| format!("{:.2}{}", v, suffix))
}

/// Formats an optional f64 representing days.
fn format_optional_days(value: Option<f64>) -> String {
    value.map_or("N/A".to_string(), |v| format!("{:.1}", v)) // One decimal place for days
}

/// Formats an optional average vote count.
fn format_optional_avg_votes(value: Option<f64>) -> String {
     value.map_or("N/A".to_string(), |v| format!("{:.1}", v)) // One decimal place for votes
}

// --- NEW: Section Formatting Functions ---

fn format_section_i(stats: &OverallStats, scope: &str) -> String {
    let mut section = format!("## I. Overall Summary ({})\n\n", scope);

    section.push_str(&format!(
        "*   **Epochs Included:** {} ({} Active/Planned, {} Closed)\n",
        stats.total_epochs_included, stats.num_active_planned, stats.num_closed
    ));

    let time_span = match (stats.first_epoch_start_date, stats.latest_epoch_end_date) {
        (Some(start), Some(end)) => format!("{} to {}", start.format("%Y-%m-%d"), end.format("%Y-%m-%d")),
        _ => "N/A".to_string(),
    };
    section.push_str(&format!("*   **Overall Time Span:** {}\n", time_span));

    section.push_str("*   **Total Budget Allocated (Epoch Rewards):**\n");
    if stats.total_allocated_budget.is_empty() {
        section.push_str("    *   N/A\n");
    } else {
        for (token, amount) in stats.total_allocated_budget.iter().sorted_by_key(|(t, _)| *t) {
            section.push_str(&format!("    *   {}: {:.2}\n", token, amount));
        }
    }

    section.push_str("*   **Total Budget Requested (Approved Proposals):**\n");
     if stats.total_requested_budget.is_empty() {
        section.push_str("    *   N/A\n");
    } else {
        for (token, amount) in stats.total_requested_budget.iter().sorted_by_key(|(t, _)| *t) {
            section.push_str(&format!("    *   {}: {:.2}\n", token, amount));
        }
    }

    section.push_str("*   **Total Budget Paid (Approved & Paid Proposals):**\n");
    if stats.total_paid_budget.is_empty() {
        section.push_str("    *   N/A\n");
    } else {
        for (token, amount) in stats.total_paid_budget.iter().sorted_by_key(|(t, _)| *t) {
            section.push_str(&format!("    *   {}: {:.2}\n", token, amount));
        }
    }

    section.push_str(&format!("*   **Total Proposals Submitted:** {}\n", stats.total_proposals));
    section.push_str(&format!("*   **Total Proposals Resolved:** {}\n", stats.total_resolved_proposals));
    section.push_str(&format!("*   **Total Proposals Approved:** {}\n", stats.total_approved_proposals));
    section.push_str(&format!("*   **Total Proposals Paid:** {}\n", stats.total_paid_proposals));
    section.push_str(&format!("*   **Overall Approval Rate:** {}\n", format_optional_f64(stats.overall_approval_rate, "%")));
    section.push_str(&format!("*   **Overall Avg. Resolution Time:** {} days\n", format_optional_days(stats.overall_avg_resolution_time_days)));
    section.push_str(&format!("*   **Overall Avg. Payment Time (Post-Approval):** {} days\n", format_optional_days(stats.overall_avg_payment_time_days)));
    section.push_str(&format!("*   **Overall Avg. 'Yes' Votes (Passed Proposals):** {}\n", format_optional_avg_votes(stats.overall_avg_yes_votes_passed)));
    section.push_str(&format!("*   **Overall Avg. 'No' Votes (Rejected Proposals):** {}\n", format_optional_avg_votes(stats.overall_avg_no_votes_rejected)));
    section.push_str(&format!("*   **Total Active Teams (Current):** {}\n", stats.total_active_teams_current));

    section.push_str("\n---\n\n");
    section
}


fn format_section_ii(epoch_stats: &[EpochStats], scope: &str) -> String {
    let mut section = format!("## II. Epoch-by-Epoch Summary ({})\n\n", scope);
    section.push_str("This table shows key metrics for each epoch included in the report scope. Epochs marked with `*` are currently Active or Planned.\n\n");

    section.push_str("| Epoch Name      | Status  | Dates (Start-End) | Allocated Budget | Requested Budget (Approved) | Paid Funding | Paid Loans | # Props | # Res | # Appr | Appr Rate (%) | Avg Res Time (d) | Avg Pay Time (d) | Avg Yes (Pass) | Avg No (Fail) |\n");
    section.push_str("| :-------------- | :------ | :---------------- | :--------------- | :-------------------------- | :----------- | :--------- | :------ | :---- | :----- | :------------ | :--------------- | :----------------- | :------------- | :------------ |\n");

    let mut total_proposals = 0;
    let mut total_resolved = 0;
    let mut total_approved = 0;
    let mut total_allocated = HashMap::new();
    let mut total_requested = HashMap::new();
    let mut total_paid_funding = HashMap::new();
    let mut total_paid_loans = HashMap::new();

    for stats in epoch_stats {
        let name_marker = if stats.status == "Closed" { stats.name.clone() } else { format!("{}*", stats.name) };
        let dates = format!("{} - {}", stats.start_date.format("%Y-%m-%d"), stats.end_date.format("%Y-%m-%d"));

        section.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            name_marker,
            stats.status,
            dates,
            format_token_amounts_grouped(&stats.allocated_budget), // Use grouped formatter
            format_token_amounts_grouped(&stats.requested_budget), // Use grouped formatter
            format_token_amounts_grouped(&stats.paid_funding_budget), // Use grouped formatter
            format_token_amounts_grouped(&stats.paid_loans_budget), // Use grouped formatter
            stats.num_proposals,
            stats.num_resolved,
            stats.num_approved,
            format_optional_f64(stats.approval_rate, "%"),
            format_optional_days(stats.avg_resolution_time_days),
            format_optional_days(stats.avg_payment_time_days), // Still avg time for FUNDING
            format_optional_avg_votes(stats.avg_yes_votes_passed),
            format_optional_avg_votes(stats.avg_no_votes_rejected)
        ));

        // Accumulate totals
        total_proposals += stats.num_proposals;
        total_resolved += stats.num_resolved;
        total_approved += stats.num_approved;
        for (token, amount) in &stats.allocated_budget { *total_allocated.entry(token.clone()).or_insert(0.0) += amount; }
        for (token, amount) in &stats.requested_budget { *total_requested.entry(token.clone()).or_insert(0.0) += amount; }
        for (token, amount) in &stats.paid_funding_budget { *total_paid_funding.entry(token.clone()).or_insert(0.0) += amount; }
        for (token, amount) in &stats.paid_loans_budget { *total_paid_loans.entry(token.clone()).or_insert(0.0) += amount; }
    }

    // Add Totals Row
    section.push_str(&format!(
        "| **Totals**      |         |                   | **{}** | **{}**           | **{}** | **{}** | **{}** | **{}** | **{}** |               |                  |                    |                |               |\n",
        format_token_amounts_grouped(&total_allocated),
        format_token_amounts_grouped(&total_requested),
        format_token_amounts_grouped(&total_paid_funding), // Use grouped formatter
        format_token_amounts_grouped(&total_paid_loans), // Use grouped formatter
        total_proposals,
        total_resolved,
        total_approved
    ));

    section.push_str("\n*Notes:*\n");
    section.push_str("*   Data includes epochs based on the selected scope (`All Epochs` or `Completed Epochs Only`).\n");
    section.push_str("*   Financial amounts are aggregated per token, with stablecoins grouped.\n");
    section.push_str("*   `Paid Funding` excludes loan amounts. `Paid Loans` shows only loan amounts.\n");
    section.push_str("*   `# Resolved`: Number of proposals within the epoch that have a resolution (Approved, Rejected, Invalid, Duplicate, Retracted).\n");
    section.push_str("*   `Approval Rate`: (# Approved / # Resolved) * 100 for the epoch.\n");
    section.push_str("*   `Avg. Res. Time`: Average days from proposal `published_at` (or `announced_at`) to `resolved_at` for resolved proposals in the epoch.\n");
    section.push_str("*   `Avg. Pay Time`: Average days from proposal `resolved_at` to `payment_date` for approved *and paid* budget requests in the epoch. Calculated for non-loan funding proposals only.\n");
    section.push_str("*   `Avg. Yes (Passed)`: Average number of 'Yes' votes in the *counted* group for formal votes on proposals that were ultimately *Approved* during the epoch.\n");
    section.push_str("*   `Avg. No (Failed)`: Average number of 'No' votes in the *counted* group for formal votes on proposals that were ultimately *Rejected* during the epoch.\n");
    section.push_str("*   Averages are displayed as 'N/A' if no relevant data exists for the calculation.\n");

    section.push_str("\n---\n\n");
    section
}


fn format_section_iii(team_stats: &[TeamPerformanceSummary], scope: &str) -> String {
    let mut section = format!("## III. Team Performance Summary ({})\n\n", scope);
    section.push_str("This table summarizes the overall activity for each team across the epochs included in this report.\n\n");

    section.push_str("| Team Name        | Status (Current) | Total Proposals Submitted | Total Proposals Approved | Total Funding Paid | Total Loans Paid | Total Points Earned |\n");
    section.push_str("| :--------------- | :--------------- | :------------------------ | :----------------------- | :----------------- | :--------------- | :------------------ |\n");

    for stats in team_stats {
        section.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} |\n",
            stats.team_name,
            stats.current_status, // Already uses clean format from calculation step
            stats.total_proposals_submitted,
            stats.total_proposals_approved,
            format_token_amounts_grouped(&stats.total_funding_paid), // Use grouped formatter
            format_token_amounts_grouped(&stats.total_loans_paid), // Use grouped formatter
            stats.total_points_earned
        ));
    }

    section.push_str("\n*Notes:*\n");
    section.push_str("*   *Status* reflects the team's status at the time the report was generated.\n");
    section.push_str("*   *Total Proposals Submitted/Approved* count proposals linked to the team via `BudgetRequestDetails` across the included epochs.\n");
    section.push_str("*   *Total Funding/Loans Paid* sums `request_amounts` from proposals submitted by the team, *approved*, and marked as *paid* across the included epochs (aggregated per token, stablecoins grouped).\n");
    section.push_str("*   *Total Points Earned* sums points awarded for voting participation across the included epochs.\n");
    section.push_str("\n---\n\n");
    section
}


fn format_section_iv(
    paid_funding_data: &PaidFundingData,
    paid_loan_data: &PaidFundingData,selected_epochs: &[&Epoch],
    teams: &HashMap<Uuid, Team>, // Need teams to get names
    scope: &str,
) -> String {
    let mut section = format!("## IV. Detailed Team Funding Paid per Epoch ({})\n\n", scope);
    // --- Funding Subsection ---
    section.push_str("### Paid Funding (Non-Loan)\n\n");
    section.push_str("This section breaks down the *paid funding* amounts (excluding loans) for each team within each epoch included in this report.\n\n");
    section.push_str("*(Note: A separate table is generated for each major token/group involved in paid funding requests.)*\n\n");

    if paid_funding_data.funding.is_empty() {
        section.push_str("No paid funding data found for the selected epochs.\n\n");
    } else {
        // Sort tokens for consistent table order
        let sorted_tokens: Vec<&String> = paid_funding_data.funding.keys().sorted().collect();
        for token in sorted_tokens {
             section.push_str(&format!("**Token: {}**\n\n", token));
            // Header
            section.push_str("| Team Name        ");
            for epoch in selected_epochs { section.push_str(&format!("| {} Paid ", epoch.name())); }
            section.push_str("| **Total Paid** |\n");
            // Separator
            section.push_str("| :--------------- ");
            for _ in selected_epochs { section.push_str("| :---------------------- "); }
            section.push_str("| :------------- |\n");
            // Team Rows
            let sorted_team_ids: Vec<&Uuid> = teams.keys().sorted_by_key(|id| teams.get(id).map(|t| t.name()).unwrap_or("")).collect();
            for team_id in sorted_team_ids {
                let team_name = teams.get(team_id).map_or("Unknown Team", |t| t.name());
                section.push_str(&format!("| {} ", team_name));
                for epoch in selected_epochs {
                    let amount = paid_funding_data.funding.get(token)
                        .and_then(|emap| emap.get(&epoch.id()))
                        .and_then(|tmap| tmap.get(team_id)).unwrap_or(&0.0);
                    section.push_str(&format!("| {} ", format_currency(*amount))); // Use currency format
                }
                let team_total = paid_funding_data.team_totals.get(token)
                    .and_then(|tmap| tmap.get(team_id)).unwrap_or(&0.0);
                section.push_str(&format!("| **{}** |\n", format_currency(*team_total))); // Use currency format
            }
            // Totals Row
            section.push_str("| **Totals**       ");
            for epoch in selected_epochs {
                let epoch_total = paid_funding_data.epoch_totals.get(token)
                    .and_then(|emap| emap.get(&epoch.id())).unwrap_or(&0.0);
                section.push_str(&format!("| **{}** ", format_currency(*epoch_total))); // Use currency format
            }
            let grand_total = paid_funding_data.grand_totals.get(token).unwrap_or(&0.0);
            section.push_str(&format!("| **{}** |\n", format_currency(*grand_total))); // Use currency format
            section.push_str("\n");
        }
    }

     // --- Loans Subsection ---
    section.push_str("### Paid Loans\n\n");
    section.push_str("This section breaks down the *paid loan* amounts for each team within each epoch included in this report.\n\n");
    section.push_str("*(Note: A separate table is generated for each major token/group involved in paid loan requests.)*\n\n");

    if paid_loan_data.funding.is_empty() { // Use the 'funding' field name within the PaidFundingData struct
        section.push_str("No paid loan data found for the selected epochs.\n");
    } else {
        // Sort tokens for consistent table order
        let sorted_tokens: Vec<&String> = paid_loan_data.funding.keys().sorted().collect();
        for token in sorted_tokens {
             section.push_str(&format!("**Token: {}**\n\n", token));
             // Header
            section.push_str("| Team Name        ");
            for epoch in selected_epochs { section.push_str(&format!("| {} Paid ", epoch.name())); }
            section.push_str("| **Total Paid** |\n");
            // Separator
            section.push_str("| :--------------- ");
            for _ in selected_epochs { section.push_str("| :---------------------- "); }
            section.push_str("| :------------- |\n");
             // Team Rows
            let sorted_team_ids: Vec<&Uuid> = teams.keys().sorted_by_key(|id| teams.get(id).map(|t| t.name()).unwrap_or("")).collect();
            for team_id in sorted_team_ids {
                let team_name = teams.get(team_id).map_or("Unknown Team", |t| t.name());
                section.push_str(&format!("| {} ", team_name));
                for epoch in selected_epochs {
                    let amount = paid_loan_data.funding.get(token) // Use the 'funding' field name
                        .and_then(|emap| emap.get(&epoch.id()))
                        .and_then(|tmap| tmap.get(team_id)).unwrap_or(&0.0);
                    section.push_str(&format!("| {} ", format_currency(*amount))); // Use currency format
                }
                let team_total = paid_loan_data.team_totals.get(token)
                    .and_then(|tmap| tmap.get(team_id)).unwrap_or(&0.0);
                section.push_str(&format!("| **{}** |\n", format_currency(*team_total))); // Use currency format
            }
            // Totals Row
            section.push_str("| **Totals**       ");
            for epoch in selected_epochs {
                let epoch_total = paid_loan_data.epoch_totals.get(token)
                    .and_then(|emap| emap.get(&epoch.id())).unwrap_or(&0.0);
                section.push_str(&format!("| **{}** ", format_currency(*epoch_total))); // Use currency format
            }
            let grand_total = paid_loan_data.grand_totals.get(token).unwrap_or(&0.0);
            section.push_str(&format!("| **{}** |\n", format_currency(*grand_total))); // Use currency format
            section.push_str("\n");
        }
    }


    section.push_str("*Notes:*\n");
    section.push_str("*   Table shows the sum of `request_amounts` for the specified token from proposals submitted by the team that were *approved*, marked as *paid*, and designated as a *loan* (or *not* a loan for the Funding section) during that specific epoch.\n");
    section.push_str("*   Amounts are shown for the specified token/group only.\n");
    section.push_str("\n---\n\n"); // End of report separator
    section
}