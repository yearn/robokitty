use uuid::Uuid;
use chrono::NaiveDate;
use std::collections::HashMap;
use serde::{Serialize, Deserialize};
use crate::UpdateProposalDetails;
use crate::BudgetRequestDetailsScript;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Proposal {
    pub id: Uuid,
    pub epoch_id: Uuid,
    pub title: String,
    pub url: Option<String>,
    pub status: ProposalStatus,
    pub resolution: Option<Resolution>,
    pub budget_request_details: Option<BudgetRequestDetails>,
    pub announced_at: Option<NaiveDate>,
    pub published_at: Option<NaiveDate>,
    pub resolved_at: Option<NaiveDate>,
    pub is_historical: bool,
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
    pub team: Option<Uuid>,
    pub request_amounts: HashMap<String, f64>,
    pub start_date: Option<NaiveDate>,
    pub end_date: Option<NaiveDate>,
    pub payment_status: Option<PaymentStatus>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PaymentStatus {
    Unpaid,
    Paid
}

impl Proposal {
    pub fn new(epoch_id: Uuid, title: String, url: Option<String>, budget_request_details: Option<BudgetRequestDetails>, announced_at: Option<NaiveDate>, published_at: Option<NaiveDate>, is_historical: Option<bool>) -> Self {
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

    pub fn set_announced_at(&mut self, date: NaiveDate) {
        self.announced_at = Some(date);
    }

    pub fn set_published_at(&mut self, date: NaiveDate) {
        self.published_at = Some(date);
    }

    pub fn set_resolved_at(&mut self, date: NaiveDate) {
        self.resolved_at = Some(date);
    }

    pub fn set_historical(&mut self, is_historical: bool) {
        self.is_historical = is_historical;
    }

    pub fn set_dates(&mut self, announced_at: Option<NaiveDate>, published_at: Option<NaiveDate>, resolved_at: Option<NaiveDate>) {
        if let Some(date) = announced_at {
            self.set_announced_at(date);
        }
        if let Some(date) = published_at {
            self.set_published_at(date);
        }
        if let Some(date) = resolved_at {
            self.set_resolved_at(date);
        }
    }

    pub fn update_status(&mut self, new_status: ProposalStatus) {
        self.status = new_status;
    }

    pub fn set_resolution(&mut self, resolution: Resolution) {
        self.resolution = Some(resolution);
    }

    pub fn remove_resolution(&mut self) {
        self.resolution = None;
    }

    pub fn mark_as_paid(&mut self) -> Result<(), &'static str> {
        match (&self.status, &self.resolution, &mut self.budget_request_details) {
            (_, Some(Resolution::Approved), Some(details)) => {
                details.payment_status = Some(PaymentStatus::Paid);
                Ok(())
            }
            (_, Some(Resolution::Approved), None) => Err("Cannot mark as paid: Not a budget request"),
            _ => Err("Cannot mark as paid: Proposal is not approved")
        }
    }

    pub fn is_budget_request(&self) -> bool {
        self.budget_request_details.is_some()
    }

    pub fn is_actionable(&self) -> bool {
        matches!(self.status, ProposalStatus::Open | ProposalStatus::Reopened)
    }

    pub fn approve(&mut self) -> Result<(), &'static str> {
        println!("Attempting to approve proposal. Current status: {:?}", self.status);
        if self.status != ProposalStatus::Open && self.status != ProposalStatus::Reopened {
            return Err("Proposal is not in a state that can be approved");
        }
        self.status = ProposalStatus::Closed;
        self.resolution = Some(Resolution::Approved);
        Ok(())
    }
    
    pub fn reject(&mut self) -> Result<(), &'static str> {
        println!("Attempting to reject proposal. Current status: {:?}", self.status);
        if self.status != ProposalStatus::Open && self.status != ProposalStatus::Reopened {
            return Err("Proposal is not in a state that can be rejected");
        }
        self.status = ProposalStatus::Closed;
        self.resolution = Some(Resolution::Rejected);
        Ok(())
    }


    pub fn update(&mut self, updates: UpdateProposalDetails, team_id: Option<Uuid>) -> Result<(), &'static str> {
        if let Some(title) = updates.title {
            self.title = title;
        }
        if let Some(url) = updates.url {
            self.url = Some(url);
        }
        if let Some(announced_at) = updates.announced_at {
            self.announced_at = Some(announced_at);
        }
        if let Some(published_at) = updates.published_at {
            self.published_at = Some(published_at);
        }
        if let Some(resolved_at) = updates.resolved_at {
            self.resolved_at = Some(resolved_at);
        }
        if let Some(budget_details) = updates.budget_request_details {
            self.update_budget_request_details(&budget_details, team_id)?;
        }

        // Validate dates
        if let (Some(start), Some(end)) = (self.budget_request_details.as_ref().and_then(|d| d.start_date), self.budget_request_details.as_ref().and_then(|d| d.end_date)) {
            if start > end {
                return Err("Start date cannot be after end date");
            }
        }

        Ok(())
    }

    pub fn update_budget_request_details(&mut self, updates: &BudgetRequestDetailsScript, team_id: Option<Uuid>) -> Result<(), &'static str> {
        let details = self.budget_request_details.get_or_insert(BudgetRequestDetails {
            team: None,
            request_amounts: HashMap::new(),
            start_date: None,
            end_date: None,
            payment_status: None,
        });

        // Update team ID if provided
        if updates.team.is_some() {
            details.team = team_id;
            if details.team.is_none() {
                return Err("Specified team not found");
            }
        }
        
        if let Some(new_request_amounts) = &updates.request_amounts {
            println!("Significant change: Replacing entire request_amounts for proposal {}", self.title);
            println!("Old request_amounts: {:?}", details.request_amounts);
            println!("New request_amounts: {:?}", new_request_amounts);
            details.request_amounts = new_request_amounts.clone();
        }
        
        if let Some(start_date) = updates.start_date {
            details.start_date = Some(start_date);
        }
        if let Some(end_date) = updates.end_date {
            details.end_date = Some(end_date);
        }
        if let Some(payment_status) = &updates.payment_status {
            details.payment_status = Some(payment_status.clone());
        }
        
        // Validate budget amounts
        for &amount in details.request_amounts.values() {
            if amount < 0.0 {
                return Err("Budget amounts must be non-negative");
            }
        }

        Ok(())
    }
    
}




