// src/core/file_system.rs
use crate::core::budget_system::BudgetSystem;
use crate::core::models::Proposal;
use crate::core::state::BudgetSystemState;
use crate::app_config::AppConfig;
use crate::services::ethereum::EthereumServiceTrait;
use crate::commands::cli::ScriptCommand;

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
            .map(|team_id| format!("-{}", Self::sanitize_filename(&team_id.to_string())))
            .unwrap_or_default();
    
        let sanitized_title = Self::sanitize_filename(proposal.title());
    
        // Calculate the maximum length for the title
        let max_title_length = 255 
            - reports_dir.as_os_str().len() 
            - date.len() 
            - team_part.len() 
            - 5; // 5 for the dash, file extension (.md), and some buffer
    
        let truncated_title = if sanitized_title.len() > max_title_length {
            sanitized_title[..max_title_length].to_string()
        } else {
            sanitized_title
        };
    
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
        let sanitized: String = name.chars()
            .map(|c| match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' => c,
                _ => '_'
            })
            .collect();
        if sanitized.len() > 255 {
            sanitized[..255].to_string()
        } else {
            sanitized
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use chrono::{Utc, NaiveDate};
    use crate::core::models::{Proposal, Team, TeamStatus};
    use crate::app_config::AppConfig;
    use std::collections::HashMap;
    use std::path::Path;
    use std::fs::File;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    fn setup_temp_dir() -> TempDir {
        TempDir::new().expect("Failed to create temp dir")
    }

    fn create_mock_state() -> BudgetSystemState {
        let mut state = BudgetSystemState::new();
        let team = Team::new(
            "Test Team".to_string(),
            "John Doe".to_string(),
            Some(vec![1000, 2000, 3000])
        ).unwrap();
        state.add_team(team);
        state
    }

    fn create_mock_proposal() -> Proposal {
        Proposal::new(
            Uuid::new_v4(),
            "Test Proposal".to_string(),
            Some("http://example.com".to_string()),
            None,
            Some(Utc::now().date_naive()),
            Some(Utc::now().date_naive()),
            None
        )
    }

    mod state_management_tests {
        use super::*;

        #[test]
        fn test_save_state_to_file() {
            let temp_dir = setup_temp_dir();
            let state_file = temp_dir.path().join("test_state.json");
            let state = create_mock_state();

            FileSystem::save_state(&state, state_file.to_str().unwrap()).unwrap();

            assert!(state_file.exists());
            assert!(state_file.metadata().unwrap().len() > 0);
        }

        #[test]
        fn test_load_state_from_file() {
            let temp_dir = setup_temp_dir();
            let state_file = temp_dir.path().join("test_state.json");
            let original_state = create_mock_state();

            FileSystem::save_state(&original_state, state_file.to_str().unwrap()).unwrap();

            let loaded_state = FileSystem::load_state(state_file.to_str().unwrap()).unwrap();

            assert_eq!(
                original_state.current_state().teams().len(),
                loaded_state.current_state().teams().len()
            );
        }

        #[test]
        fn test_try_load_state_non_existent_file() {
            let temp_dir = setup_temp_dir();
            let non_existent_file = temp_dir.path().join("non_existent.json");

            let result = FileSystem::try_load_state(non_existent_file.to_str().unwrap());

            assert!(result.is_none());
        }

        #[test]
        fn test_save_and_load_state_various_sizes() {
            let temp_dir = setup_temp_dir();
            let state_file = temp_dir.path().join("test_state.json");

            // Test with empty state
            let empty_state = BudgetSystemState::new();
            FileSystem::save_state(&empty_state, state_file.to_str().unwrap()).unwrap();
            let loaded_empty_state = FileSystem::load_state(state_file.to_str().unwrap()).unwrap();
            assert_eq!(empty_state.current_state().teams().len(), loaded_empty_state.current_state().teams().len());

            // Test with populated state
            let populated_state = create_mock_state();
            FileSystem::save_state(&populated_state, state_file.to_str().unwrap()).unwrap();
            let loaded_populated_state = FileSystem::load_state(state_file.to_str().unwrap()).unwrap();
            assert_eq!(populated_state.current_state().teams().len(), loaded_populated_state.current_state().teams().len());
        }

        #[test]
        fn test_overwrite_existing_state_file() {
            let temp_dir = setup_temp_dir();
            let state_file = temp_dir.path().join("test_state.json");

            let initial_state = create_mock_state();
            FileSystem::save_state(&initial_state, state_file.to_str().unwrap()).unwrap();

            let mut new_state = BudgetSystemState::new();
            let new_team = Team::new(
                "New Team".to_string(),
                "Jane Doe".to_string(),
                None
            ).unwrap();
            new_state.add_team(new_team);

            FileSystem::save_state(&new_state, state_file.to_str().unwrap()).unwrap();

            let loaded_state = FileSystem::load_state(state_file.to_str().unwrap()).unwrap();
            assert_eq!(new_state.current_state().teams().len(), loaded_state.current_state().teams().len());
            assert!(loaded_state.current_state().teams().values().any(|team| team.name() == "New Team"));
        }
    }

    mod file_path_generation_tests {
        use super::*;

        #[test]
        fn test_generate_report_file_paths() {
            let temp_dir = setup_temp_dir();
            let state_file = temp_dir.path().join("state.json");
            let proposal = create_mock_proposal();
            let epoch_name = "Test Epoch";

            let path = FileSystem::generate_report_file_path(&proposal, epoch_name, &state_file);

            assert!(path.to_str().unwrap().contains("Test_Epoch"));
            assert!(path.to_str().unwrap().contains("Test_Proposal"));
            assert!(path.extension().unwrap() == "md");
        }

        #[test]
        fn test_handle_special_characters_in_file_names() {
            let temp_dir = setup_temp_dir();
            let state_file = temp_dir.path().join("state.json");
            let mut proposal = create_mock_proposal();
            proposal.set_title("Test: Proposal with * special / characters?".to_string());
            let epoch_name = "Test & Epoch";

            let path = FileSystem::generate_report_file_path(&proposal, epoch_name, &state_file);

            let file_name = path.file_name().unwrap().to_str().unwrap();
            println!("Generated file name: {}", file_name);
            println!("File name contains 'Test': {}", file_name.contains("Test"));
            println!("File name contains 'Proposal': {}", file_name.contains("Proposal"));
            println!("File name contains 'with': {}", file_name.contains("with"));
            println!("File name contains 'special': {}", file_name.contains("special"));
            println!("File name contains 'characters': {}", file_name.contains("characters"));

            assert!(!file_name.contains("*"));
            assert!(!file_name.contains("?"));
            assert!(!file_name.contains("/"));
            assert!(!file_name.contains("&"));
            assert!(file_name.contains("Test__Proposal_with___special___characters_"));
        }

        #[test]
        fn test_path_generation_different_epoch_names() {
            let temp_dir = setup_temp_dir();
            let state_file = temp_dir.path().join("state.json");
            let proposal = create_mock_proposal();

            let path1 = FileSystem::generate_report_file_path(&proposal, "Epoch 1", &state_file);
            let path2 = FileSystem::generate_report_file_path(&proposal, "Epoch 2", &state_file);

            assert!(path1.to_str().unwrap().contains("Epoch_1"));
            assert!(path2.to_str().unwrap().contains("Epoch_2"));
            assert_ne!(path1, path2);
        }

        #[test]
        fn test_path_generation_long_names() {
            let temp_dir = setup_temp_dir();
            let state_file = temp_dir.path().join("state.json");
            let mut proposal = create_mock_proposal();
            proposal.set_title("This is a very long proposal title that exceeds the normal length of a title and should be truncated in the file name".to_string());
            let epoch_name = "This is also a very long epoch name that should be handled properly in the file path generation process";

            let path = FileSystem::generate_report_file_path(&proposal, epoch_name, &state_file);

            println!("Generated path: {:?}", path);
            println!("Path length: {}", path.to_str().unwrap().len());

            assert!(path.to_str().unwrap().len() <= 255, "Path length exceeds 255 characters");
        }
    }

    mod report_generation_and_saving_tests {
        use super::*;

        #[test]
        fn test_generate_and_save_proposal_report() {
            let temp_dir = setup_temp_dir();
            let state_file = temp_dir.path().join("state.json");
            let proposal = create_mock_proposal();
            let epoch_name = "Test Epoch";
            let report_content = "This is a test report content.";

            let file_path = FileSystem::generate_and_save_proposal_report(
                &proposal,
                report_content,
                epoch_name,
                &state_file
            ).unwrap();

            assert!(file_path.exists());
            let saved_content = std::fs::read_to_string(file_path).unwrap();
            assert_eq!(saved_content, report_content);
        }

        #[test]
        fn test_overwrite_existing_report() {
            let temp_dir = setup_temp_dir();
            let state_file = temp_dir.path().join("state.json");
            let proposal = create_mock_proposal();
            let epoch_name = "Test Epoch";
            let initial_content = "Initial content";
            let new_content = "New content";

            let file_path = FileSystem::generate_and_save_proposal_report(
                &proposal,
                initial_content,
                epoch_name,
                &state_file
            ).unwrap();

            let new_file_path = FileSystem::generate_and_save_proposal_report(
                &proposal,
                new_content,
                epoch_name,
                &state_file
            ).unwrap();

            assert_eq!(file_path, new_file_path);
            let saved_content = std::fs::read_to_string(file_path).unwrap();
            assert_eq!(saved_content, new_content);
        }

        #[test]
        fn test_report_content_integrity() {
            let temp_dir = setup_temp_dir();
            let state_file = temp_dir.path().join("state.json");
            let proposal = create_mock_proposal();
            let epoch_name = "Test Epoch";
            let report_content = "This is a test report with some special characters: !@#$%^&*()";

            let file_path = FileSystem::generate_and_save_proposal_report(
                &proposal,
                report_content,
                epoch_name,
                &state_file
            ).unwrap();

            let saved_content = std::fs::read_to_string(file_path).unwrap();
            assert_eq!(saved_content, report_content);
        }
    }

    mod script_loading_tests {
        use super::*;

        #[test]
        fn test_load_valid_script() {
            let temp_dir = setup_temp_dir();
            let script_file = temp_dir.path().join("valid_script.json");
            let script_content = r#"
            [
                {"type": "CreateEpoch", "params": {"name": "Test Epoch", "start_date": "2023-01-01T00:00:00Z", "end_date": "2023-12-31T23:59:59Z"}},
                {"type": "AddTeam", "params": {"name": "Test Team", "representative": "John Doe", "trailing_monthly_revenue": [1000, 2000, 3000]}}
            ]
            "#;
            std::fs::write(&script_file, script_content).unwrap();

            let loaded_script = FileSystem::load_script(script_file.to_str().unwrap()).unwrap();

            assert_eq!(loaded_script.len(), 2);
            match &loaded_script[0] {
                ScriptCommand::CreateEpoch { name, .. } => assert_eq!(name, "Test Epoch"),
                _ => panic!("Unexpected command type"),
            }
        }

        #[test]
        fn test_load_invalid_json_script() {
            let temp_dir = setup_temp_dir();
            let script_file = temp_dir.path().join("invalid_script.json");
            let script_content = r#"
            [
                {"type": "CreateEpoch", "params": {"name": "Test Epoch", "start_date": "2023-01-01T00:00:00Z", "end_date": "2023-12-31T23:59:59Z"}},
                {"type": "InvalidCommand", "params": {}}
            ]
            "#;
            std::fs::write(&script_file, script_content).unwrap();

            let result = FileSystem::load_script(script_file.to_str().unwrap());

            assert!(result.is_err());
        }

        #[test]
        fn test_load_empty_script() {
            let temp_dir = setup_temp_dir();
            let script_file = temp_dir.path().join("empty_script.json");
            let script_content = "[]";
            std::fs::write(&script_file, script_content).unwrap();

            let loaded_script = FileSystem::load_script(script_file.to_str().unwrap()).unwrap();

            assert!(loaded_script.is_empty());
        }

        #[test]
        fn test_load_script_with_unknown_commands() {
            let temp_dir = setup_temp_dir();
            let script_file = temp_dir.path().join("mixed_script.json");
            let script_content = r#"
            [
                {"type": "CreateEpoch", "params": {"name": "Test Epoch", "start_date": "2023-01-01T00:00:00Z", "end_date": "2023-12-31T23:59:59Z"}},
                {"type": "UnknownCommand", "params": {}},
                {"type": "AddTeam", "params": {"name": "Test Team", "representative": "John Doe", "trailing_monthly_revenue": [1000, 2000, 3000]}}
            ]
            "#;
            std::fs::write(&script_file, script_content).unwrap();

            let result = FileSystem::load_script(script_file.to_str().unwrap());

            assert!(result.is_err());
            // The error should mention the unknown command
            assert!(result.unwrap_err().to_string().contains("UnknownCommand"));
        }
    }

    mod file_name_sanitization_tests {
        use super::*;

        #[test]
        fn test_clean_file_name_with_special_characters() {
            assert_eq!(FileSystem::clean_file_name("file:name?.txt"), "file_name_.txt");
            assert_eq!(FileSystem::clean_file_name("file/name\\with|invalid*chars"), "file_name_with_invalid_chars");
            assert_eq!(FileSystem::clean_file_name("file<with>quotes\""), "file_with_quotes_");
        }

        #[test]
        fn test_clean_file_name_with_normal_name() {
            assert_eq!(FileSystem::clean_file_name("normal_file_name.txt"), "normal_file_name.txt");
        }

        #[test]
        fn test_sanitize_filename_with_special_characters() {
            assert_eq!(FileSystem::sanitize_filename("file name with spaces"), "file_name_with_spaces");
            assert_eq!(FileSystem::sanitize_filename("file@name#with$special%chars"), "file_name_with_special_chars");
        }

        #[test]
        fn test_sanitize_filename_with_unicode() {
            assert_eq!(FileSystem::sanitize_filename("файл с юникодом"), "_______________");
        }

        #[test]
        fn test_sanitize_filename_empty_string() {
            assert_eq!(FileSystem::sanitize_filename(""), "");
        }

        #[test]
        fn test_sanitize_filename_very_long_name() {
            let long_name = "a".repeat(300);
            let sanitized = FileSystem::sanitize_filename(&long_name);
            assert!(sanitized.len() <= 255);
        }
    }

    mod error_handling_and_edge_case_tests {
        use super::*;

        #[test]
        fn test_save_state_permission_error() {
            let temp_dir = setup_temp_dir();
            let state_file = temp_dir.path().join("readonly_state.json");
            
            // Create a directory instead of a file
            std::fs::create_dir(&state_file).unwrap();

            let state = create_mock_state();
            let result = FileSystem::save_state(&state, state_file.to_str().unwrap());

            assert!(result.is_err());
            if let Err(e) = result {
                assert!(e.to_string().contains("Is a directory") || e.to_string().contains("Access is denied"));
            }
        }

        #[test]
        fn test_load_state_invalid_json() {
            let temp_dir = setup_temp_dir();
            let state_file = temp_dir.path().join("invalid_state.json");
            std::fs::write(&state_file, "invalid json content").unwrap();

            let result = FileSystem::load_state(state_file.to_str().unwrap());

            assert!(result.is_err());
        }

        #[test]
        fn test_generate_report_file_path_invalid_characters() {
            let temp_dir = setup_temp_dir();
            let state_file = temp_dir.path().join("state.json");
            let mut proposal = create_mock_proposal();
            proposal.set_title("Invalid/File:Name?".to_string());
            let epoch_name = "Test*Epoch";

            let path = FileSystem::generate_report_file_path(&proposal, epoch_name, &state_file);

            let file_name = path.file_name().unwrap().to_str().unwrap();
            assert!(!file_name.contains("/"));
            assert!(!file_name.contains(":"));
            assert!(!file_name.contains("?"));
            assert!(!file_name.contains("*"));
            assert!(file_name.contains("Invalid_File_Name_"));
        }


        #[test]
        fn test_load_script_file_not_found() {
            let temp_dir = setup_temp_dir();
            let non_existent_file = temp_dir.path().join("non_existent_script.json");

            let result = FileSystem::load_script(non_existent_file.to_str().unwrap());

            assert!(result.is_err());
            assert!(result.unwrap_err().to_string().contains("No such file or directory"));
        }
    }

    mod integration_tests {
        use super::*;
        use crate::services::ethereum::MockEthereumService;
        use std::sync::Arc;

        async fn create_mock_budget_system(temp_dir: &TempDir) -> BudgetSystem {
            let config = AppConfig {
                state_file: temp_dir.path().join("state.json").to_str().unwrap().to_string(),
                ipc_path: "/tmp/test_reth.ipc".to_string(),
                future_block_offset: 10,
                script_file: "test_script.json".to_string(),
                default_total_counted_seats: 7,
                default_max_earner_seats: 5,
                default_qualified_majority_threshold: 0.7,
                counted_vote_points: 5,
                uncounted_vote_points: 2,
                telegram: crate::app_config::TelegramConfig {
                    chat_id: "test_chat_id".to_string(),
                    token: "test_token".to_string(),
                },
            };
            let ethereum_service = Arc::new(MockEthereumService);
            FileSystem::initialize_budget_system(&config, ethereum_service).await.unwrap()
        }

        #[tokio::test]
        async fn test_full_cycle_save_modify_load() {
            let temp_dir = setup_temp_dir();
            let mut budget_system = create_mock_budget_system(&temp_dir).await;

            // Modify the state
            budget_system.create_team("Test Team".to_string(), "John Doe".to_string(), Some(vec![1000, 2000, 3000])).unwrap();

            // Save the state
            budget_system.save_state().unwrap();

            // Create a new budget system (simulating a restart)
            let loaded_budget_system = create_mock_budget_system(&temp_dir).await;

            // Verify the loaded state
            assert_eq!(loaded_budget_system.state().current_state().teams().len(), 1);
            assert!(loaded_budget_system.state().current_state().teams().values().any(|team| team.name() == "Test Team"));
        }

        #[tokio::test]
        async fn test_initialize_budget_system_with_existing_state() {
            let temp_dir = setup_temp_dir();
            let mut initial_budget_system = create_mock_budget_system(&temp_dir).await;

            // Modify and save the initial state
            initial_budget_system.create_team("Existing Team".to_string(), "Jane Doe".to_string(), None).unwrap();
            initial_budget_system.save_state().unwrap();

            // Initialize a new budget system with the existing state
            let loaded_budget_system = create_mock_budget_system(&temp_dir).await;

            // Verify the loaded state
            assert_eq!(loaded_budget_system.state().current_state().teams().len(), 1);
            assert!(loaded_budget_system.state().current_state().teams().values().any(|team| team.name() == "Existing Team"));
        }

        #[tokio::test]
        async fn test_initialize_budget_system_without_existing_state() {
            let temp_dir = setup_temp_dir();
            let budget_system = create_mock_budget_system(&temp_dir).await;

            // Verify that a new, empty state was created
            assert_eq!(budget_system.state().current_state().teams().len(), 0);
        }
    }
}