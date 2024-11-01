use uuid::Uuid;
use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

pub trait NameMatches {
    fn name_matches(&self, name: &str) -> bool;
}

pub fn get_id_by_name<T: NameMatches>(map: &HashMap<Uuid, T>, name: &str) -> Option<Uuid> {
    map.iter()
        .find(|(_, item)| item.name_matches(name))
        .map(|(id, _)| *id)
}


#[derive(Debug, Serialize, Deserialize)]
pub struct UnpaidRequestsReport {
    pub generated_at: DateTime<Utc>,
    pub unpaid_requests: Vec<UnpaidRequest>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UnpaidRequest {
    pub proposal_id: String,
    pub title: String,
    pub team_name: String,
    pub amounts: HashMap<String, f64>,
    pub payment_address: Option<String>,
    pub approved_date: String,
    pub is_loan: bool,
    pub epoch_name: String,
}

impl UnpaidRequestsReport {
    pub fn new(unpaid_requests: Vec<UnpaidRequest>) -> Self {
        Self {
            generated_at: Utc::now(),
            unpaid_requests,
        }
    }
}

impl UnpaidRequest {
    pub fn new(
        proposal_id: uuid::Uuid,
        title: String,
        team_name: String,
        amounts: HashMap<String, f64>,
        payment_address: Option<String>,
        approved_date: chrono::NaiveDate,
        is_loan: bool,
        epoch_name: String,
    ) -> Self {
        Self {
            proposal_id: proposal_id.to_string(),
            title,
            team_name,
            amounts,
            payment_address,
            approved_date: approved_date.format("%Y-%m-%d").to_string(),
            is_loan,
            epoch_name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_unpaid_request_serialization() {
        let mut amounts = HashMap::new();
        amounts.insert("ETH".to_string(), 100.0);
        
        let request = UnpaidRequest::new(
            uuid::Uuid::new_v4(),
            "Test Proposal".to_string(),
            "Test Team".to_string(),
            amounts,
            Some("0x123...".to_string()),
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            false,
            "Q1 2024".to_string(),
        );
        
        let json = serde_json::to_string_pretty(&request).unwrap();
        println!("Serialized JSON:\n{}", json);
        
        let deserialized: UnpaidRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.title, "Test Proposal");
        assert_eq!(deserialized.team_name, "Test Team");
    }

    #[test]
    fn test_report_serialization() {
        let mut amounts = HashMap::new();
        amounts.insert("ETH".to_string(), 100.0);
        
        let request = UnpaidRequest::new(
            uuid::Uuid::new_v4(),
            "Test Proposal".to_string(),
            "Test Team".to_string(),
            amounts,
            Some("0x123...".to_string()),
            NaiveDate::from_ymd_opt(2024, 1, 1).unwrap(),
            false,
            "Q1 2024".to_string(),
        );
        
        let report = UnpaidRequestsReport::new(vec![request]);
        let json = serde_json::to_string_pretty(&report).unwrap();
        println!("Serialized Report JSON:\n{}", json);
        
        let deserialized: UnpaidRequestsReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.unpaid_requests.len(), 1);
    }
}