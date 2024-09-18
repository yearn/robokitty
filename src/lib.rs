use crate::core::budget_system::BudgetSystem;
use crate::services::ethereum::EthereumService;
use crate::app_config::AppConfig;
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

pub async fn run_script_commands() -> Result<(), Box<dyn std::error::Error>> {
    let (mut budget_system, config) = initialize_system().await?;
    lock::create_lock_file()?;
    
    // Execute script commands
    if std::path::Path::new(&config.script_file).exists() {
        let script = crate::core::file_system::FileSystem::load_script(&config.script_file)?;
        for command in script {
            if let Err(e) = crate::commands::cli::execute_command(&mut budget_system, command, &config).await {
                log::error!("Error executing command: {}", e);
            }
        }
    }
    
    budget_system.save_state()?;
    lock::remove_lock_file()?;
    Ok(())
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