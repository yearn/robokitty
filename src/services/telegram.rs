use crate::core::budget_system::BudgetSystem;
use crate::commands::telegram::{TelegramCommand, parse_command};
use crate::commands::common::Command;
use teloxide::prelude::*;
use teloxide::types::ParseMode;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use std::io::Cursor;

pub struct TelegramBot {
    bot: Bot,
    command_sender: mpsc::Sender<(TelegramCommand, oneshot::Sender<String>)>,
}

impl TelegramBot {
    pub fn new(bot: Bot, command_sender: mpsc::Sender<(TelegramCommand, oneshot::Sender<String>)>) -> Self {
        Self { bot, command_sender }
    }

    pub async fn run(self) {
        let command_sender = self.command_sender;
        teloxide::repl(self.bot, move |bot: Bot, msg: Message| {
            let command_sender = command_sender.clone();
            async move {
                if let Some(text) = msg.text() {
                    if let Some(command) = parse_command(text) {
                        let (response_sender, response_receiver) = oneshot::channel();
                        
                        // Determine parse mode based on command type
                        let parse_mode = match &command {
                            TelegramCommand::PrintEpochState | TelegramCommand::MarkdownTest => {
                                Some(ParseMode::MarkdownV2)
                            },
                            _ => None,
                        };
                        
                        match command_sender.send((command, response_sender)).await {
                            Ok(_) => {
                                match response_receiver.await {
                                    Ok(response) => {
                                        let mut message = bot.send_message(msg.chat.id, response);
                                        if let Some(mode) = parse_mode {
                                            message = message.parse_mode(mode);
                                        }
                                        message.disable_web_page_preview(true).await?;
                                    }
                                    Err(e) => {
                                        bot.send_message(
                                            msg.chat.id, 
                                            format!("Error processing command: {}", e)
                                        ).await?;
                                    }
                                }
                            }
                            Err(e) => {
                                bot.send_message(
                                    msg.chat.id,
                                    format!("Error sending command: {}", e)
                                ).await?;
                            }
                        }
                    } else {
                        bot.send_message(msg.chat.id, "Unknown command. Type /help for available commands.").await?;
                    }
                }
                Ok(())
            }
        }).await;
    }
}

pub fn spawn_command_executor(
    mut budget_system: BudgetSystem,
    mut command_receiver: mpsc::Receiver<(TelegramCommand, oneshot::Sender<String>)>,
) {
    std::thread::spawn(move || {
        while let Some((telegram_command, response_sender)) = command_receiver.blocking_recv() {
            let rt = tokio::runtime::Runtime::new().unwrap();
            
            let result = rt.block_on(async {
                // Execute command
                match crate::commands::telegram::handle_command(telegram_command, &mut budget_system).await {
                    Ok(output) => Ok(output),
                    Err(e) => Err(format!("Error executing command: {}", e)),
                }
            });

            let response = match result {
                Ok(output) => output,
                Err(e) => format!("Command execution failed: {}", e),
            };

            if let Err(e) = response_sender.send(response) {
                eprintln!("Error sending response: {}", e);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::telegram::TelegramCommand;
    use crate::core::budget_system::BudgetSystem;
    use crate::app_config::AppConfig;
    use crate::services::ethereum::MockEthereumService;
    use std::sync::Arc;
    use tokio::sync::mpsc;

    async fn create_test_budget_system() -> BudgetSystem {
        let config = AppConfig::default();
        let ethereum_service = Arc::new(MockEthereumService);
        BudgetSystem::new(config, ethereum_service, None).await.unwrap()
    }

    #[tokio::test]
    async fn test_command_processing() {
        // Create channels
        let (tx, rx) = mpsc::channel(100);
        
        // Create and spawn budget system
        let budget_system = create_test_budget_system().await;
        spawn_command_executor(budget_system, rx);

        // Test command execution
        let (response_tx, response_rx) = oneshot::channel();
        tx.send((TelegramCommand::PrintEpochState, response_tx)).await.unwrap();
        
        // Wait for response
        match response_rx.await {
            Ok(response) => {
                assert!(!response.is_empty());
                // We could add more specific assertions about the response content
            },
            Err(e) => panic!("Failed to receive response: {}", e),
        }
    }

    #[tokio::test]
    async fn test_multiple_commands() {
        let (tx, rx) = mpsc::channel(100);
        let budget_system = create_test_budget_system().await;
        spawn_command_executor(budget_system, rx);

        // Test multiple commands in sequence
        for command in vec![
            TelegramCommand::PrintEpochState,
            TelegramCommand::PrintTeamReport,
            TelegramCommand::MarkdownTest,
        ] {
            let (response_tx, response_rx) = oneshot::channel();
            tx.send((command.clone(), response_tx)).await.unwrap();
            
            match response_rx.await {
                Ok(response) => {
                    assert!(!response.is_empty());
                    // Command-specific assertions could be added here
                },
                Err(e) => panic!("Failed to receive response for {:?}: {}", command, e),
            }
        }
    }

    #[tokio::test]
    async fn test_error_handling() {
        let (tx, rx) = mpsc::channel(100);
        let budget_system = create_test_budget_system().await;
        spawn_command_executor(budget_system, rx);

        // Test command with arguments
        let (response_tx, response_rx) = oneshot::channel();
        tx.send((
            TelegramCommand::PrintPointReport { 
                epoch_name: Some("NonExistentEpoch".to_string()) 
            },
            response_tx
        )).await.unwrap();
        
        // Should receive error response rather than panicking
        match response_rx.await {
            Ok(response) => {
                assert!(response.contains("error") || response.contains("Error"));
            },
            Err(e) => panic!("Failed to receive response: {}", e),
        }
    }
}