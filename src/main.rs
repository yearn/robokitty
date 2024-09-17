//src/main.rs

use crate::core::budget_system::BudgetSystem;
use crate::services::ethereum::{EthereumService, EthereumServiceTrait};
use crate::services::telegram::{TelegramBot, spawn_command_executor};
use crate::commands::telegram::TelegramCommand;
use crate::commands::cli::{ScriptCommand, execute_command};
use crate::core::file_system::FileSystem;
use dotenvy::dotenv;
use log::{info, debug, error};
use std::{
    error::Error,
    fs,
    path::Path,
    str,
    sync::Arc,
};
use teloxide::prelude::*;
use tokio::{
    self,
    sync::{mpsc, oneshot},
};

use crate::app_config::AppConfig;


pub mod core;
pub mod services;
pub mod commands;
pub mod app_config;


// Helper function to escape special characters for MarkdownV2
pub fn escape_markdown(text: &str) -> String {
    let special_chars = ['_', '*', '[', ']', '(', ')', '~', '`', '>', '#', '+', '-', '=', '|', '{', '}', '.', '!'];
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        if special_chars.contains(&c) {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    pretty_env_logger::init();
    // Load .env file
    dotenv().expect(".env file not found");
    let config = AppConfig::new()?;

    // Ensure the directory exists
    if let Some(parent) = Path::new(&config.state_file).parent() {
        fs::create_dir_all(parent)?;
    }

    // Create the EthereumService
    let ethereum_service = Arc::new(EthereumService::new(&config.ipc_path, config.future_block_offset).await?);

    let state = FileSystem::try_load_state(&config.state_file);
    let mut budget_system = BudgetSystem::new(config.clone(), ethereum_service.clone(), state).await?;

    // Read and execute the script
    if Path::new(&config.script_file).exists() {
        let script = FileSystem::load_script(&config.script_file)?;
        
        for command in script {
            if let Err(e) = execute_command(&mut budget_system, command, &config).await {
                error!("Error executing command: {}", e);
            }
        }
        println!("Script execution completed.");
    } else {
        println!("No script file found at {}. Skipping script execution.", &config.script_file);
    }

    // Save the current state
    match budget_system.save_state() {
        Ok(_) => info!("Saved current state to {}", &config.state_file),
        Err(e) => error!("Failed to save state to {}: {}", &config.state_file, e),
    }

    let (command_sender, command_receiver) = mpsc::channel::<(TelegramCommand, oneshot::Sender<String>)>(100);
    
    spawn_command_executor(budget_system, command_receiver);

    let bot = Bot::new(&config.telegram.token);
    let telegram_bot = TelegramBot::new(bot, command_sender);
    
    println!("Bot is running...");
    telegram_bot.run().await;

    Ok(())
    
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tempfile::TempDir;
    use crate::app_config::{AppConfig, TelegramConfig};
    use uuid::Uuid;
    use chrono::Utc;
    use crate::core::state::BudgetSystemState;
    use crate::services::ethereum::MockEthereumService;
    use crate::core::models::TeamStatus;


    // Helper function to create a test BudgetSystem

    async fn create_test_budget_system(state_file: &str, initial_state: Option<BudgetSystemState>) -> BudgetSystem {
        let config = AppConfig {
            state_file: state_file.to_string(),
            ipc_path: "/tmp/test_reth.ipc".to_string(),
            future_block_offset: 10,
            script_file: "test_script.json".to_string(),
            default_total_counted_seats: 7,
            default_max_earner_seats: 5,
            default_qualified_majority_threshold: 0.7,
            counted_vote_points: 5,
            uncounted_vote_points: 2,
            telegram: TelegramConfig {
                chat_id: "test_chat_id".to_string(),
                token: "test_token".to_string(),
            },
        };
        let ethereum_service = Arc::new(MockEthereumService);
        BudgetSystem::new(config, ethereum_service, initial_state).await.unwrap()
    }

    // Helper function to create and activate an epoch
    async fn create_active_epoch(budget_system: &mut BudgetSystem, name: &str, duration_days: i64) -> Uuid {
        let start_date = Utc::now();
        let end_date = start_date + chrono::Duration::days(duration_days);
        let epoch_id = budget_system.create_epoch(name, start_date, end_date).unwrap();
        budget_system.activate_epoch(epoch_id).unwrap();
        epoch_id
    }

    #[tokio::test]
    async fn test_save_and_load_state() {
        // Create a temporary directory for this test
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("test_state.json").to_str().unwrap().to_string();

        // Create a BudgetSystem and modify its state
        let mut budget_system = create_test_budget_system(&state_file, None).await;

        // Create an epoch
        let start_date = Utc::now();
        let end_date = start_date + chrono::Duration::days(30);
        let epoch_id = budget_system.create_epoch("Test Epoch", start_date, end_date).unwrap();

        // Add a team
        let team_id = budget_system.create_team("Test Team".to_string(), "Representative".to_string(), Some(vec![1000, 2000, 3000])).unwrap();

        // Save the state
        budget_system.save_state().unwrap();

        // Load the saved state
        let loaded_state = FileSystem::try_load_state(&state_file).expect("Failed to load state");

        // Create a new BudgetSystem with the loaded state
        let loaded_system = create_test_budget_system(&state_file, Some(loaded_state)).await;

        // Verify the loaded state
        assert_eq!(loaded_system.state().epochs().len(), 1);
        assert!(loaded_system.state().epochs().contains_key(&epoch_id));
        assert_eq!(loaded_system.state().current_state().teams().len(), 1);
        assert!(loaded_system.state().current_state().teams().contains_key(&team_id));

        // Verify epoch details
        let loaded_epoch = loaded_system.get_epoch(&epoch_id).unwrap();
        assert_eq!(loaded_epoch.name(), "Test Epoch");

        // Verify team details
        let loaded_team = loaded_system.get_team(&team_id).unwrap();
        assert_eq!(loaded_team.name(), "Test Team");
        assert_eq!(loaded_team.representative(), "Representative");
        if let TeamStatus::Earner { trailing_monthly_revenue } = loaded_team.status() {
            assert_eq!(trailing_monthly_revenue, &vec![1000, 2000, 3000]);
        } else {
            panic!("Expected Earner status");
        }
    }

    #[tokio::test]
    async fn test_create_epoch() {
        // Create a temporary directory for this test
        let temp_dir = TempDir::new().unwrap();
        let state_file = temp_dir.path().join("test_state.json").to_str().unwrap().to_string();
       
        let mut budget_system = create_test_budget_system(&state_file, None).await;
        let _epoch_id = create_active_epoch(&mut budget_system, "Test Epoch", 30).await;
        
        let epoch = budget_system.get_current_epoch().unwrap();
        assert_eq!(epoch.name(), "Test Epoch");
        assert!(epoch.is_active());
    }
}