use crate::core::budget_system::BudgetSystem;
use crate::services::ethereum::EthereumService;
use crate::app_config::AppConfig;
use crate::commands::common::Command;
use std::sync::Arc;
use dotenvy::dotenv;

pub mod core;
pub mod services;
pub mod commands;
pub mod app_config;
pub mod lock;

pub fn initialize_environment() {
    pretty_env_logger::init();
    dotenv().expect(".env file not found");
}

pub async fn initialize_system() -> Result<(BudgetSystem, AppConfig), Box<dyn std::error::Error>> {
    let config = AppConfig::new()?;
    let ethereum_service = Arc::new(EthereumService::new(&config.ipc_path, config.future_block_offset).await?);
    let state = crate::core::file_system::FileSystem::try_load_state(&config.state_file);
    let budget_system = BudgetSystem::new(config.clone(), ethereum_service, state).await?;
    Ok((budget_system, config))
}

pub async fn run_script_commands(command: Command) -> Result<(), Box<dyn std::error::Error>> {
    let (mut budget_system, config) = initialize_system().await?;
    lock::create_lock_file()?;
    
    let mut stdout = std::io::stdout();
    let result = commands::cli::execute_command(&mut budget_system, command, &config, &mut stdout).await;
    
    budget_system.save_state()?;
    lock::remove_lock_file()?;
    
    result
}

pub async fn run_telegram_bot() -> Result<(), Box<dyn std::error::Error>> {
    let (budget_system, config) = initialize_system().await?;
    let (command_sender, command_receiver) = tokio::sync::mpsc::channel(100);
    
    crate::services::telegram::spawn_command_executor(budget_system, command_receiver);
    
    let bot = teloxide::Bot::new(&config.telegram.token);
    let telegram_bot = crate::services::telegram::TelegramBot::new(bot, command_sender);
    
    telegram_bot.run().await;
    Ok(())
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::TempDir;

    // TODO: Improve unit testing


    fn setup_test_environment() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        env::set_var("TELEGRAM_BOT_TOKEN", "test_token");
        env::set_var("APP_STATE_FILE", temp_dir.path().join("test_state.json").to_str().unwrap());
        temp_dir
    }

    #[tokio::test]
    async fn test_initialize_system_success() {
        let _guard = setup_test_environment();
        let result = initialize_system().await;
        assert!(result.is_ok());
        
        let (_, config) = result.unwrap();
        assert_eq!(config.telegram.token, "test_token");
        // Add more assertions here to check other properties of config
    }

    #[test]
    fn test_escape_markdown_with_special_characters() {
        let input = "Hello_World! This is a *test* [link](https://example.com)";
        let expected = "Hello\\_World\\! This is a \\*test\\* \\[link\\]\\(https://example\\.com\\)";
        assert_eq!(escape_markdown(input), expected);
    }

    #[test]
    fn test_escape_markdown_without_special_characters() {
        let input = "Hello World This is a test";
        let expected = "Hello World This is a test";
        assert_eq!(escape_markdown(input), expected);
    }

    #[test]
    fn test_escape_markdown_with_mixed_content() {
        let input = "Normal text _italic_ **bold** `code` > quote";
        let expected = "Normal text \\_italic\\_ \\*\\*bold\\*\\* \\`code\\` \\> quote";
        assert_eq!(escape_markdown(input), expected);
    }
}