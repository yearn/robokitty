use uuid::Uuid;
use std::{collections::HashMap, str::FromStr};
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};
use ethers::types::{Address, H256};

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
    pub url: Option<String>,
    pub team_name: String,
    pub amounts: HashMap<String, f64>,
    pub payment_address: Option<String>,
    pub approved_date: String,
    pub is_loan: bool,
    pub start_date: Option<String>,
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
        url: Option<String>,
        start_date: Option<chrono::NaiveDate>,
    ) -> Self {
        Self {
            proposal_id: proposal_id.to_string(),
            title,
            url,
            team_name,
            amounts,
            payment_address,
            approved_date: approved_date.format("%Y-%m-%d").to_string(),
            is_loan,
            start_date: start_date.map(|d| d.format("%Y-%m-%d").to_string()),
            epoch_name,
        }
    }
}

// Custom serialization for Ethereum address
pub mod address_serde {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(address: &Option<Address>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match address {
            Some(addr) => serializer.serialize_str(&format!("{:?}", addr)),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<Address>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Option<String> = Option::deserialize(deserializer)?;
        match s {
            Some(s) => {
                Address::from_str(&s)
                    .map(Some)
                    .map_err(serde::de::Error::custom)
            }
            None => Ok(None),
        }
    }
}

// Custom serialization for transaction hash
pub mod tx_hash_serde {
    use super::*;
    use serde::{Deserializer, Serializer};

    pub fn serialize<S>(hash: &Option<H256>, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match hash {
            Some(hash) => serializer.serialize_str(&format!("{:?}", hash)),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<H256>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s: Option<String> = Option::deserialize(deserializer)?;
        match s {
            Some(s) => {
                H256::from_str(&s)
                    .map(Some)
                    .map_err(serde::de::Error::custom)
            }
            None => Ok(None),
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
            Some("https://example.com".to_string()),
            Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
        );
        
        let json = serde_json::to_string_pretty(&request).unwrap();
        println!("Serialized JSON:\n{}", json);
        
        let deserialized: UnpaidRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.title, "Test Proposal");
        assert_eq!(deserialized.team_name, "Test Team");
        assert_eq!(deserialized.url, Some("https://example.com".to_string()));
        assert_eq!(deserialized.start_date, Some("2024-01-01".to_string()));
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
            Some("https://example.com".to_string()),
            Some(NaiveDate::from_ymd_opt(2024, 1, 1).unwrap()),
        );
        
        let report = UnpaidRequestsReport::new(vec![request]);
        let json = serde_json::to_string_pretty(&report).unwrap();
        println!("Serialized Report JSON:\n{}", json);
        
        let deserialized: UnpaidRequestsReport = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.unpaid_requests.len(), 1);
    }

    #[cfg(test)]
mod tests {
    use super::*;
    use serde::{Serialize, Deserialize};

    #[derive(Serialize, Deserialize)]
    struct TestStruct {
        #[serde(with = "address_serde")]
        address: Option<Address>,
        #[serde(with = "tx_hash_serde")]
        hash: Option<H256>,
    }

    #[test]
    fn test_address_serialization() {
        let addr_str = "0x742d35Cc6634C0532925a3b844Bc454e4438f44e";
        let addr = Address::from_str(addr_str).unwrap();
        let test_struct = TestStruct {
            address: Some(addr),
            hash: None,
        };

        let serialized = serde_json::to_string(&test_struct).unwrap();
        let deserialized: TestStruct = serde_json::from_str(&serialized).unwrap();
        
        // Address is case-insensitive for validation but always serializes to lowercase
        let expected_str = "0x742d35cc6634c0532925a3b844bc454e4438f44e";
        assert_eq!(format!("{:?}", test_struct.address.unwrap()), expected_str);
        assert_eq!(format!("{:?}", deserialized.address.unwrap()), expected_str);
    }

    #[test]
    fn test_hash_serialization() {
        let hash_str = "0x0000000000000000000000000000000000000000000000000000000000000000";
        let hash = H256::from_str(hash_str).unwrap();
        let test_struct = TestStruct {
            address: None,
            hash: Some(hash),
        };

        let serialized = serde_json::to_string(&test_struct).unwrap();
        let deserialized: TestStruct = serde_json::from_str(&serialized).unwrap();
        
        assert_eq!(format!("{:?}", test_struct.hash.unwrap()), hash_str);
        assert_eq!(format!("{:?}", deserialized.hash.unwrap()), hash_str);
    }
}
}