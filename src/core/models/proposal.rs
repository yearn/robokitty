use uuid::Uuid;
use chrono::NaiveDate;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::commands::cli::UpdateProposalDetails;
use crate::commands::cli::BudgetRequestDetailsScript;
use super::common::NameMatches;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Proposal {
    id: Uuid,
    epoch_id: Uuid,
    title: String,
    url: Option<String>,
    status: ProposalStatus,
    resolution: Option<Resolution>,
    budget_request_details: Option<BudgetRequestDetails>,
    announced_at: Option<NaiveDate>,
    published_at: Option<NaiveDate>,
    resolved_at: Option<NaiveDate>,
    is_historical: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalStatus {
    Open,
    Closed,
    Reopened,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Resolution {
    Approved,
    Rejected,
    Invalid,
    Duplicate,
    Retracted
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BudgetRequestDetails {
    team: Option<Uuid>,
    request_amounts: HashMap<String, f64>,
    start_date: Option<NaiveDate>,
    end_date: Option<NaiveDate>,
    payment_status: Option<PaymentStatus>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaymentStatus {
    Unpaid,
    Paid
}

impl Proposal {
    pub fn new(
        epoch_id: Uuid,
        title: String,
        url: Option<String>,
        budget_request_details: Option<BudgetRequestDetails>,
        announced_at: Option<NaiveDate>,
        published_at: Option<NaiveDate>,
        is_historical: Option<bool>) -> Self {
        let is_historical = is_historical.unwrap_or(false);

        Proposal {
            id: Uuid::new_v4(),
            epoch_id,
            title,
            url,
            status: ProposalStatus::Open,
            resolution: None,
            budget_request_details,
            announced_at,
            published_at,
            resolved_at: None,
            is_historical,
        }
    }

    // Getter methods
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn epoch_id(&self) -> Uuid {
        self.epoch_id
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    pub fn status(&self) -> ProposalStatus {
        self.status.clone()
    }

    pub fn resolution(&self) -> Option<Resolution> {
        self.resolution.clone()
    }

    pub fn budget_request_details(&self) -> Option<&BudgetRequestDetails> {
        self.budget_request_details.as_ref()
    }

    pub fn announced_at(&self) -> Option<NaiveDate> {
        self.announced_at
    }

    pub fn published_at(&self) -> Option<NaiveDate> {
        self.published_at
    }

    pub fn resolved_at(&self) -> Option<NaiveDate> {
        self.resolved_at
    }

    pub fn is_historical(&self) -> bool {
        self.is_historical
    }

    // Setter methods
    pub fn set_title(&mut self, title: String) {
        self.title = title;
    }

    pub fn set_url(&mut self, url: Option<String>) {
        self.url = url;
    }

    pub fn set_status(&mut self, status: ProposalStatus) {
        self.status = status;
    }

    pub fn set_resolution(&mut self, resolution: Option<Resolution>) {
        self.resolution = resolution;
    }

    pub fn set_budget_request_details(&mut self, details: Option<BudgetRequestDetails>) {
        self.budget_request_details = details;
    }

    pub fn set_announced_at(&mut self, date: Option<NaiveDate>) {
        self.announced_at = date;
    }

    pub fn set_published_at(&mut self, date: Option<NaiveDate>) {
        self.published_at = date;
    }

    pub fn set_resolved_at(&mut self, date: Option<NaiveDate>) {
        self.resolved_at = date;
    }
    
    pub fn set_dates(&mut self, announced_at: Option<NaiveDate>, published_at: Option<NaiveDate>, resolved_at: Option<NaiveDate>) -> Result<(), &'static str> {
        if let (Some(announced), Some(published)) = (announced_at, published_at) {
            if announced > published {
                return Err("Announced date cannot be after published date");
            }
        }
        if let (Some(published), Some(resolved)) = (published_at, resolved_at) {
            if published > resolved {
                return Err("Published date cannot be after resolved date");
            }
        }
        
        if let Some(date) = announced_at {
            self.set_announced_at(Some(date));
        }
        if let Some(date) = published_at {
            self.set_published_at(Some(date));
        }
        if let Some(date) = resolved_at {
            self.set_resolved_at(Some(date));
        }
        
        Ok(())
    }
    
    pub fn set_historical(&mut self, is_historical: bool) {
        self.is_historical = is_historical;
    }

    pub fn set_payment_status(&mut self, status: PaymentStatus) -> Result<(), &'static str> {
        match (&self.status, &self.resolution, &mut self.budget_request_details) {
            (_, Some(Resolution::Approved), Some(details)) => {
                details.set_payment_status(Some(status));
                Ok(())
            }
            (_, Some(Resolution::Approved), None) => Err("Cannot set payment status: Not a budget request"),
            _ => Err("Cannot set payment status: Proposal is not approved")
        }
    }

    // Helper methods
    pub fn is_open(&self) -> bool {
        matches!(self.status, ProposalStatus::Open)
    }

    pub fn is_closed(&self) -> bool {
        matches!(self.status, ProposalStatus::Closed)
    }

    pub fn is_reopened(&self) -> bool {
        matches!(self.status, ProposalStatus::Reopened)
    }

    pub fn is_approved(&self) -> bool {
        matches!(self.resolution, Some(Resolution::Approved))
    }

    pub fn is_rejected(&self) -> bool {
        matches!(self.resolution, Some(Resolution::Rejected))
    }

    pub fn is_budget_request(&self) -> bool {
        self.budget_request_details.is_some()
    }

    pub fn is_actionable(&self) -> bool {
        matches!(self.status, ProposalStatus::Open | ProposalStatus::Reopened)
    }

    pub fn duration(&self) -> Option<chrono::Duration> {
        match (self.announced_at, self.resolved_at) {
            (Some(start), Some(end)) => Some(end.signed_duration_since(start)),
            _ => None,
        }
    }

    pub fn approve(&mut self) -> Result<(), &'static str> {
        if !self.is_actionable() {
            return Err("Proposal is not in a state that can be approved");
        }
        self.status = ProposalStatus::Closed;
        self.resolution = Some(Resolution::Approved);
        Ok(())
    }

    pub fn reject(&mut self) -> Result<(), &'static str> {
        if !self.is_actionable() {
            return Err("Proposal is not in a state that can be rejected");
        }
        self.status = ProposalStatus::Closed;
        self.resolution = Some(Resolution::Rejected);
        Ok(())
    }

    pub fn update(&mut self, updates: UpdateProposalDetails, team_id: Option<Uuid>) -> Result<(), &'static str> {
        if let Some(title) = updates.title {
            self.set_title(title);
        }
        if let Some(url) = updates.url {
            self.set_url(Some(url));
        }
        
        let new_announced_at = updates.announced_at.or(self.announced_at);
        let new_published_at = updates.published_at.or(self.published_at);
        let new_resolved_at = updates.resolved_at.or(self.resolved_at);
        
        self.set_dates(new_announced_at, new_published_at, new_resolved_at)?;
        
        if let Some(budget_details) = updates.budget_request_details {
            self.update_budget_request_details(&budget_details, team_id)?;
        }
    
        Ok(())
    }

    fn update_budget_request_details(&mut self, updates: &BudgetRequestDetailsScript, team_id: Option<Uuid>) -> Result<(), &'static str> {
        let details = self.budget_request_details.get_or_insert_with(BudgetRequestDetails::default);

        if updates.team.is_some() {
            details.set_team(team_id);
        }

        if let Some(request_amounts) = &updates.request_amounts {
            for (token, &amount) in request_amounts {
                details.add_request_amount(token.clone(), amount)?;
            }
        }
        if updates.start_date.is_some() || updates.end_date.is_some() {
            details.set_dates(updates.start_date, updates.end_date)?;
        }
        if let Some(payment_status) = &updates.payment_status {
            details.set_payment_status(Some(payment_status.clone()));
        }

        details.validate()?;

        Ok(())
    }
    
}

impl NameMatches for Proposal {
    fn name_matches(&self, name: &str) -> bool {
        self.title() == name
    }
}

impl BudgetRequestDetails {
    // Constructor
    pub fn new(
        team: Option<Uuid>,
        request_amounts: HashMap<String, f64>,
        start_date: Option<NaiveDate>,
        end_date: Option<NaiveDate>,
        payment_status: Option<PaymentStatus>
    ) -> Result<Self, &'static str> {
        let brd = BudgetRequestDetails {
            team,
            request_amounts,
            start_date,
            end_date,
            payment_status,
        };
        brd.validate()?;
        Ok(brd)
    }

    fn validate(&self) -> Result<(), &'static str> {

        // Ensure payment_status is None for new proposals
        if self.payment_status().is_some() {
            return Err("New proposals should not have a payment status");
        }

        // Validate request amounts
        if self.request_amounts.is_empty() {
            return Err("Request amounts cannot be empty");
        }
        for &amount in self.request_amounts.values() {
            if amount <= 0.0 {
                return Err("Request amounts must be positive");
            }
        }

        // Validate dates
        if let (Some(start), Some(end)) = (self.start_date, self.end_date) {
            if start > end {
                return Err("Start date must be before or equal to end date");
            }
        }

        Ok(())
    }

    pub fn default() -> Self {
        BudgetRequestDetails {
            team: None,
            request_amounts: HashMap::new(),
            start_date: None,
            end_date: None,
            payment_status: None,
        }
    }

    // Getter methods
    pub fn team(&self) -> Option<Uuid> {
        self.team
    }

    pub fn request_amounts(&self) -> &HashMap<String, f64> {
        &self.request_amounts
    }

    pub fn start_date(&self) -> Option<NaiveDate> {
        self.start_date
    }

    pub fn end_date(&self) -> Option<NaiveDate> {
        self.end_date
    }

    pub fn payment_status(&self) -> Option<PaymentStatus> {
        self.payment_status.clone()
    }

    // Setter methods
    pub fn set_team(&mut self, team: Option<Uuid>) {
        self.team = team;
    }

    pub fn set_payment_status(&mut self, status: Option<PaymentStatus>) {
        self.payment_status = status;
    }

    pub fn add_request_amount(&mut self, token: String, amount: f64) -> Result<(), &'static str> {
        if amount < 0.0 {
            return Err("Request amount must be non-negative");
        }
        self.request_amounts.insert(token, amount);
        Ok(())
    }

    pub fn remove_request_amount(&mut self, token: &str) -> Option<f64> {
        self.request_amounts.remove(token)
    }

    pub fn set_dates(&mut self, start_date: Option<NaiveDate>, end_date: Option<NaiveDate>) -> Result<(), &'static str> {
        if let (Some(start), Some(end)) = (start_date, end_date) {
            if start > end {
                return Err("Start date cannot be after end date");
            }
        }
        self.start_date = start_date;
        self.end_date = end_date;
        Ok(())
    }

    // Helper methods

    pub fn is_paid(&self) -> bool {
        matches!(self.payment_status, Some(PaymentStatus::Paid))
    }

    pub fn total_request_amount(&self) -> f64 {
        self.request_amounts.values().sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    // Helper function to create a basic proposal
    fn create_test_proposal() -> Proposal {
        Proposal::new(
            Uuid::new_v4(),
            "Test Proposal".to_string(),
            Some("http://example.com".to_string()),
            None,
            Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
            Some(NaiveDate::from_ymd_opt(2023, 1, 5).unwrap()),
            None,
        )
    }

    #[test]
    fn test_proposal_creation() {
        let proposal = create_test_proposal();
        assert_eq!(proposal.title(), "Test Proposal");
        assert_eq!(proposal.url(), Some("http://example.com"));
        assert_eq!(proposal.status(), ProposalStatus::Open);
        assert!(proposal.resolution().is_none());
    }

    #[test]
    fn test_proposal_status_changes() {
        let mut proposal = create_test_proposal();
        assert!(proposal.is_open());
        
        proposal.approve().unwrap();
        assert!(proposal.is_closed());
        assert!(proposal.is_approved());
        
        // Reset for the next test
        proposal = create_test_proposal();
        proposal.reject().unwrap();
        assert!(proposal.is_closed());
        assert!(proposal.is_rejected());
    }

    #[test]
    fn test_proposal_resolution() {
        let mut proposal = create_test_proposal();
        proposal.set_resolution(Some(Resolution::Approved));
        assert_eq!(proposal.resolution(), Some(Resolution::Approved));
        
        proposal.set_resolution(Some(Resolution::Rejected));
        assert_eq!(proposal.resolution(), Some(Resolution::Rejected));
    }

    #[test]
    fn test_budget_request_details() {
        let mut proposal = create_test_proposal();
        let budget_details = BudgetRequestDetails::new(
            Some(Uuid::new_v4()),
            [("ETH".to_string(), 100.0)].iter().cloned().collect(),
            Some(NaiveDate::from_ymd_opt(2023, 2, 1).unwrap()),
            Some(NaiveDate::from_ymd_opt(2023, 2, 28).unwrap()),
            None,
        ).unwrap();
        
        proposal.set_budget_request_details(Some(budget_details));
        assert!(proposal.is_budget_request());
        
        let details = proposal.budget_request_details().unwrap();
        assert_eq!(details.request_amounts().get("ETH"), Some(&100.0));
    }

    #[test]
    fn test_proposal_dates() {
        let mut proposal = create_test_proposal();
        let new_announced = NaiveDate::from_ymd_opt(2023, 3, 1).unwrap();
        let new_published = NaiveDate::from_ymd_opt(2023, 3, 5).unwrap();
        let new_resolved = NaiveDate::from_ymd_opt(2023, 3, 10).unwrap();
        
        proposal.set_dates(Some(new_announced), Some(new_published), Some(new_resolved)).unwrap();
        assert_eq!(proposal.announced_at(), Some(new_announced));
        assert_eq!(proposal.published_at(), Some(new_published));
        assert_eq!(proposal.resolved_at(), Some(new_resolved));
    }

    
    #[test]
    fn test_proposal_update() {
        let mut proposal = create_test_proposal();
        
        proposal.set_dates(
            Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
            Some(NaiveDate::from_ymd_opt(2023, 1, 5).unwrap()),
            Some(NaiveDate::from_ymd_opt(2023, 1, 10).unwrap())
        ).unwrap();

        let updates = UpdateProposalDetails {
            title: Some("Updated Title".to_string()),
            url: Some("http://updated.com".to_string()),
            budget_request_details: Some(BudgetRequestDetailsScript {
                team: Some("New Team".to_string()),
                request_amounts: Some([("ETH".to_string(), 200.0)].iter().cloned().collect()),
                start_date: Some(NaiveDate::from_ymd_opt(2023, 4, 1).unwrap()),
                end_date: Some(NaiveDate::from_ymd_opt(2023, 4, 30).unwrap()),
                payment_status: None,
            }),
            announced_at: Some(NaiveDate::from_ymd_opt(2023, 3, 15).unwrap()),
            published_at: Some(NaiveDate::from_ymd_opt(2023, 3, 20).unwrap()),
            resolved_at: Some(NaiveDate::from_ymd_opt(2023, 3, 25).unwrap()),
        };
        
        proposal.update(updates, Some(Uuid::new_v4())).unwrap();
        
        assert_eq!(proposal.title(), "Updated Title");
        assert_eq!(proposal.url(), Some("http://updated.com"));
        assert_eq!(proposal.announced_at(), Some(NaiveDate::from_ymd_opt(2023, 3, 15).unwrap()));
        assert_eq!(proposal.published_at(), Some(NaiveDate::from_ymd_opt(2023, 3, 20).unwrap()));
        assert_eq!(proposal.resolved_at(), Some(NaiveDate::from_ymd_opt(2023, 3, 25).unwrap()));

        let budget_details = proposal.budget_request_details().unwrap();
        assert_eq!(budget_details.request_amounts().get("ETH"), Some(&200.0));
        assert_eq!(budget_details.start_date(), Some(NaiveDate::from_ymd_opt(2023, 4, 1).unwrap()));
        assert_eq!(budget_details.end_date(), Some(NaiveDate::from_ymd_opt(2023, 4, 30).unwrap()));
    }

    #[test]
    fn test_proposal_duration() {
        let mut proposal = create_test_proposal();
        proposal.set_dates(
            Some(NaiveDate::from_ymd_opt(2023, 1, 1).unwrap()),
            Some(NaiveDate::from_ymd_opt(2023, 1, 5).unwrap()),
            Some(NaiveDate::from_ymd_opt(2023, 1, 10).unwrap()),
        ).unwrap();
        
        assert_eq!(proposal.duration().unwrap().num_days(), 9);
    }

    #[test]
    #[should_panic(expected = "Announced date cannot be after published date")]
    fn test_invalid_dates() {
        let mut proposal = create_test_proposal();
        proposal.set_dates(
            Some(NaiveDate::from_ymd_opt(2023, 1, 10).unwrap()),
            Some(NaiveDate::from_ymd_opt(2023, 1, 5).unwrap()),
            None,
        ).unwrap();
    }

    #[test]
    fn test_budget_request_validation() {
        let result = BudgetRequestDetails::new(
            None,
            [("ETH".to_string(), -100.0)].iter().cloned().collect(),
            None,
            None,
            None,
        );
        assert!(result.is_err());
    }

    #[test]
    fn test_proposal_actionable_status() {
        let mut proposal = create_test_proposal();
        assert!(proposal.is_actionable());
        
        proposal.set_status(ProposalStatus::Closed);
        assert!(!proposal.is_actionable());
        
        proposal.set_status(ProposalStatus::Reopened);
        assert!(proposal.is_actionable());
    }
}