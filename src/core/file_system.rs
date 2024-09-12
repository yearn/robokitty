// src/core/file_system.rs
use crate::core::budget_system::BudgetSystem;
use crate::core::models::Proposal;
use crate::core::state::BudgetSystemState;
use crate::app_config::AppConfig;
use crate::services::ethereum::EthereumServiceTrait;
use crate::ScriptCommand;

use serde_json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::error::Error;
use log::{debug, info, error};
use uuid::Uuid;

pub struct FileSystem;

impl FileSystem {
    pub fn save_state(state: &BudgetSystemState, state_file: &str) -> Result<(), Box<dyn Error>> {
        let json = serde_json::to_string_pretty(state)?;
        
        if let Some(parent) = Path::new(state_file).parent() {
            fs::create_dir_all(parent)?;
        }
        
        let temp_file = format!("{}.temp", state_file);
        fs::write(&temp_file, &json)?;
        fs::rename(&temp_file, state_file)?;
        
        Ok(())
    }

    pub fn load_state(path: &str) -> Result<BudgetSystemState, Box<dyn Error>> {
        let json = fs::read_to_string(path)?;
        let state: BudgetSystemState = serde_json::from_str(&json)?;
        Ok(state)
    }

    pub fn try_load_state(path: &str) -> Option<BudgetSystemState> {
        match Self::load_state(path) {
            Ok(state) => Some(state),
            Err(e) => {
                eprintln!("Failed to load state from {}: {}. Starting with a new state.", path, e);
                None
            }
        }
    }

    pub async fn initialize_budget_system(
        config: &AppConfig,
        ethereum_service: Arc<dyn EthereumServiceTrait>
    ) -> Result<BudgetSystem, Box<dyn Error>> {
        let state = Self::try_load_state(&config.state_file);
        BudgetSystem::new(config.clone(), ethereum_service, state).await
    }

    pub fn generate_report_file_path(
        proposal: &Proposal,
        epoch_name: &str,
        state_file: &Path
    ) -> PathBuf {
        let state_file_dir = state_file.parent().unwrap_or_else(|| Path::new("."));
        let reports_dir = state_file_dir.join("reports").join(Self::sanitize_filename(epoch_name));

        let date = proposal.published_at()
            .or(proposal.announced_at())
            .map(|date| date.format("%Y%m%d").to_string())
            .unwrap_or_else(|| "00000000".to_string());

        let team_part = proposal.budget_request_details()
            .as_ref()
            .and_then(|details| details.team())
            .map(|team_id| format!("-{}", Self::clean_file_name(&team_id.to_string())))
            .unwrap_or_default();

        let truncated_title = Self::clean_file_name(proposal.title())
            .chars()
            .take(30)
            .collect::<String>()
            .replace(" ", "_");

        let file_name = format!("{}{}-{}.md", date, team_part, truncated_title);
        reports_dir.join(file_name)
    }

    pub fn generate_and_save_proposal_report(
        proposal: &Proposal,
        report_content: &str,
        epoch_name: &str,
        state_file: &Path
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let file_path = Self::generate_report_file_path(proposal, epoch_name, state_file);

        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&file_path, report_content)?;

        Ok(file_path)
    }

    pub fn load_script(script_file: &str) -> Result<Vec<ScriptCommand>, Box<dyn Error>> {
        let script_content = fs::read_to_string(script_file)?;
        let script: Vec<ScriptCommand> = serde_json::from_str(&script_content)?;
        Ok(script)
    }

    pub fn clean_file_name(name: &str) -> String {
        name.chars()
            .map(|c| match c {
                '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                _ => c
            })
            .collect()
    }
    
    pub fn sanitize_filename(name: &str) -> String {
        name.chars()
            .map(|c| match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '_' => c,
                _ => '_'
            })
            .collect()
    }
}