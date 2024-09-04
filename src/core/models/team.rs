use serde::{Serialize, Deserialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TeamStatus {
    Earner { trailing_monthly_revenue: Vec<u64>},
    Supporter,
    Inactive,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Team {
    id: Uuid,
    name: String,
    representative: String,
    status: TeamStatus,
}

impl Team {
    pub fn new(name: String, representative: String, trailing_monthly_revenue: Option<Vec<u64>>) -> Result<Self, &'static str> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_update_team_status() {
        let mut team = Team::new(
            "Test Team".to_string(),
            "Representative".to_string(),
            Some(vec![1000, 2000, 3000])
        ).unwrap();

        // Test changing to Supporter status
        team.set_status(TeamStatus::Supporter).unwrap();
        assert!(team.is_supporter());

        // Test changing back to Earner status
        team.set_status(TeamStatus::Earner { trailing_monthly_revenue: vec![4000, 5000, 6000] }).unwrap();
        assert!(team.is_earner());
        if let TeamStatus::Earner { trailing_monthly_revenue } = team.status() {
            assert_eq!(trailing_monthly_revenue, &vec![4000, 5000, 6000]);
        } else {
            panic!("Expected Earner status");
        }

        // Test changing to Inactive status
        team.set_status(TeamStatus::Inactive).unwrap();
        assert!(team.is_inactive());

        // Test error case: changing to Earner without revenue data
        let result = team.set_status(TeamStatus::Earner { trailing_monthly_revenue: vec![] });
        assert!(result.is_err());

        // Test error case: changing to Earner with too much revenue data
        let result = team.set_status(TeamStatus::Earner { trailing_monthly_revenue: vec![1000, 2000, 3000, 4000] });
        assert!(result.is_err());
    }
}