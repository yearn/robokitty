use serde::{Serialize, Deserialize};
use uuid::Uuid;
use super::common::{NameMatches, address_serde};
use ethers::types::Address;
use std::str::FromStr;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TeamStatus {
    Earner { trailing_monthly_revenue: Vec<u64>},
    Supporter,
    Inactive,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Team {
    id: Uuid,
    name: String,
    representative: String,
    status: TeamStatus,
    #[serde(with = "address_serde", default)]
    payment_address: Option<Address>,
}

impl Team {
    // Constructor
    pub fn new(name: String, representative: String, trailing_monthly_revenue: Option<Vec<u64>>, address: Option<String>) -> Result<Self, &'static str> {
        if name.trim().is_empty() {
            return Err("Team name cannot be empty");
        }
        if representative.trim().is_empty() {
            return Err("Representative name cannot be empty");
        }

        let payment_address = match address {
            Some(addr) => Some(
                Address::from_str(&addr)
                    .map_err(|_| "Invalid Ethereum address")?
            ),
            None => None,
        };

        let status = match trailing_monthly_revenue {
            Some(revenue) => {
                if revenue.is_empty() {
                    return Err("Revenue data cannot be empty");
                } else if revenue.len() > 3 {
                    return Err("Revenue data cannot exceed 3 entries");  
                } 

                TeamStatus::Earner { trailing_monthly_revenue: revenue }
            },
            None => TeamStatus::Supporter,
        };

        Ok(Team {
            id: Uuid::new_v4(),
            name,
            representative,
            status,
            payment_address,
        })
    }

    // Getter methods
    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn representative(&self) -> &str {
        &self.representative
    }

    pub fn status(&self) -> &TeamStatus {
        &self.status
    }

    pub fn payment_address(&self) -> Option<&Address> {
        self.payment_address.as_ref()
    }

    // Setter methods
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }

    pub fn set_representative(&mut self, representative: String) {
        self.representative = representative;
    }

    pub fn set_status(&mut self, new_status: TeamStatus) -> Result<(), &'static str> {
        match new_status {
            TeamStatus::Earner { ref trailing_monthly_revenue } if trailing_monthly_revenue.is_empty() => {
                Err("Trailing revenue data must be provided when changing to Earner status")
            },
            TeamStatus::Earner { trailing_monthly_revenue } if trailing_monthly_revenue.len() > 3 => {
                Err("Revenue data cannot exceed 3 entries")
            },
            _ => {
                self.status = new_status;
                Ok(())
            }
        }
    }

    pub fn set_payment_address(&mut self, address: Option<String>) -> Result<(), &'static str> {
        self.payment_address = match address {
            Some(addr) => Some(Address::from_str(&addr).map_err(|_| "Invalid Ethereum address")?),
            None => None,
        };
        Ok(())
    }

    // Helper methods
    pub fn is_active(&self) -> bool {
        !matches!(self.status, TeamStatus::Inactive)
    }

    pub fn is_earner(&self) -> bool {
        matches!(self.status, TeamStatus::Earner { .. })
    }

    pub fn is_supporter(&self) -> bool {
        matches!(self.status, TeamStatus::Supporter)
    }

    pub fn is_inactive(&self) -> bool {
        matches!(self.status, TeamStatus::Inactive)
    }

}

impl NameMatches for Team {
    fn name_matches(&self, name: &str) -> bool {
        self.name() == name
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json;

    #[test]
    fn test_create_valid_team() {
        let earner = Team::new("Earner Team".to_string(), "John Doe".to_string(), Some(vec![1000, 2000, 3000]), None).unwrap();
        assert_eq!(earner.name(), "Earner Team");
        assert_eq!(earner.representative(), "John Doe");
        assert!(matches!(earner.status(), TeamStatus::Earner { .. }));

        let supporter = Team::new("Supporter Team".to_string(), "Jane Doe".to_string(), None, None).unwrap();
        assert_eq!(supporter.name(), "Supporter Team");
        assert_eq!(supporter.representative(), "Jane Doe");
        assert!(matches!(supporter.status(), TeamStatus::Supporter));
    }

    #[test]
    fn test_create_invalid_team() {
        assert!(Team::new("".to_string(), "John Doe".to_string(), None, None).is_err());
        assert!(Team::new(" ".to_string(), "John Doe".to_string(), None, None).is_err());
        assert!(Team::new("Valid Name".to_string(), "".to_string(), None, None).is_err());
        assert!(Team::new("Valid Name".to_string(), " ".to_string(), None, None).is_err());
        assert!(Team::new("Earner".to_string(), "John Doe".to_string(), Some(vec![]), None).is_err());
    }

    #[test]
    fn test_getter_methods() {
        let team = Team::new("Test Team".to_string(), "Test Rep".to_string(), Some(vec![1000]), None).unwrap();
        assert_eq!(team.name(), "Test Team");
        assert_eq!(team.representative(), "Test Rep");
        assert!(matches!(team.status(), TeamStatus::Earner { .. }));
    }

    #[test]
    fn test_setter_methods() {
        let mut team = Team::new("Old Name".to_string(), "Old Rep".to_string(), None, None).unwrap();
        
        team.set_name("New Name".to_string());
        assert_eq!(team.name(), "New Name");

        team.set_representative("New Rep".to_string());
        assert_eq!(team.representative(), "New Rep");

        team.set_status(TeamStatus::Earner { trailing_monthly_revenue: vec![1000, 2000] }).unwrap();
        assert!(matches!(team.status(), TeamStatus::Earner { .. }));
    }

    #[test]
    fn test_status_changes() {
        let mut team = Team::new("Test Team".to_string(), "Test Rep".to_string(), None, None).unwrap();
        
        assert!(team.set_status(TeamStatus::Earner { trailing_monthly_revenue: vec![1000] }).is_ok());
        assert!(team.is_earner());

        assert!(team.set_status(TeamStatus::Supporter).is_ok());
        assert!(team.is_supporter());

        assert!(team.set_status(TeamStatus::Inactive).is_ok());
        assert!(team.is_inactive());
    }

    #[test]
    fn test_status_helper_methods() {
        let mut team = Team::new("Test Team".to_string(), "Test Rep".to_string(), None, None).unwrap();
        
        assert!(team.is_active());
        assert!(team.is_supporter());
        assert!(!team.is_earner());
        assert!(!team.is_inactive());

        team.set_status(TeamStatus::Earner { trailing_monthly_revenue: vec![1000] }).unwrap();
        assert!(team.is_active());
        assert!(team.is_earner());
        assert!(!team.is_supporter());
        assert!(!team.is_inactive());

        team.set_status(TeamStatus::Inactive).unwrap();
        assert!(!team.is_active());
        assert!(!team.is_earner());
        assert!(!team.is_supporter());
        assert!(team.is_inactive());
    }

    #[test]
    fn test_team_status_validation() {
        let mut team = Team::new("Test Team".to_string(), "Test Rep".to_string(), None, None).unwrap();
        
        assert!(team.set_status(TeamStatus::Earner { trailing_monthly_revenue: vec![1000] }).is_ok());
        assert!(team.set_status(TeamStatus::Earner { trailing_monthly_revenue: vec![1000, 2000, 3000] }).is_ok());
        
        assert!(team.set_status(TeamStatus::Earner { trailing_monthly_revenue: vec![] }).is_err());
        assert!(team.set_status(TeamStatus::Earner { trailing_monthly_revenue: vec![1000, 2000, 3000, 4000] }).is_err());
    }

    #[test]
    fn test_edge_cases() {
        let long_name = "a".repeat(256);
        let long_rep = "b".repeat(256);
        let team = Team::new(long_name.clone(), long_rep.clone(), None, None).unwrap();
        assert_eq!(team.name(), long_name);
        assert_eq!(team.representative(), long_rep);

        let max_revenue = u64::MAX;
        let team = Team::new("Max Revenue".to_string(), "Test Rep".to_string(), Some(vec![max_revenue]), None).unwrap();
        if let TeamStatus::Earner { trailing_monthly_revenue } = team.status() {
            assert_eq!(trailing_monthly_revenue[0], max_revenue);
        } else {
            panic!("Expected Earner status");
        }
    }

    #[test]
    fn test_serialization_deserialization() {
        let original_team = Team::new(
            "Serialize Team".to_string(),
            "Serialize Rep".to_string(),
            Some(vec![1000, 2000, 3000]),
            None
        ).unwrap();

        let serialized = serde_json::to_string(&original_team).unwrap();
        let deserialized_team: Team = serde_json::from_str(&serialized).unwrap();

        assert_eq!(original_team.id(), deserialized_team.id());
        assert_eq!(original_team.name(), deserialized_team.name());
        assert_eq!(original_team.representative(), deserialized_team.representative());
        
        match (original_team.status(), deserialized_team.status()) {
            (TeamStatus::Earner { trailing_monthly_revenue: original_revenue },
             TeamStatus::Earner { trailing_monthly_revenue: deserialized_revenue }) => {
                assert_eq!(original_revenue, deserialized_revenue);
            },
            _ => panic!("Expected Earner status for both teams"),
        }

        // Test other status variants
        let supporter_team = Team::new("Supporter".to_string(), "Support Rep".to_string(), None, None).unwrap();
        let serialized = serde_json::to_string(&supporter_team).unwrap();
        let deserialized: Team = serde_json::from_str(&serialized).unwrap();
        assert!(matches!(deserialized.status(), TeamStatus::Supporter));

        let mut inactive_team = supporter_team;
        inactive_team.set_status(TeamStatus::Inactive).unwrap();
        let serialized = serde_json::to_string(&inactive_team).unwrap();
        let deserialized: Team = serde_json::from_str(&serialized).unwrap();
        assert!(matches!(deserialized.status(), TeamStatus::Inactive));
    }

    #[test]
    fn test_team_payment_address() {
        let valid_address = "0x742d35Cc6634C0532925a3b844Bc454e4438f44e".to_string();
        
        // Test creation with address
        let team = Team::new(
            "Test Team".to_string(),
            "Representative".to_string(),
            Some(vec![1000]),
            Some(valid_address.clone())
        ).unwrap();
        assert!(team.payment_address().is_some());
        
        // Test creation without address
        let team_no_addr = Team::new(
            "Test Team 2".to_string(),
            "Representative".to_string(),
            Some(vec![1000]),
            None
        ).unwrap();
        assert!(team_no_addr.payment_address().is_none());
        
        // Test invalid address
        let result = Team::new(
            "Test Team 3".to_string(),
            "Representative".to_string(),
            Some(vec![1000]),
            Some("invalid_address".to_string())
        );
        assert!(result.is_err());
    }
}