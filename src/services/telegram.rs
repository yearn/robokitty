use crate::core::budget_system::BudgetSystem;
use crate::commands::telegram::{TelegramCommand, execute_command};
use teloxide::prelude::*;
use teloxide::utils::command::BotCommands;
use teloxide::types::ParseMode;
use core::marker::PhantomData;
use tokio::sync::{mpsc, oneshot};

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
        
        teloxide::commands_repl(
            self.bot,
            move |bot: Bot, msg: Message, cmd: TelegramCommand| {
                let command_sender = command_sender.clone();
                async move {
                    let (response_sender, response_receiver) = oneshot::channel();
                    
                    if let Err(e) = command_sender.send((cmd, response_sender)).await {
                        bot.send_message(
                            msg.chat.id,
                            format!("Error sending command: {}", e)
                        ).await?;
                        return Ok(());
                    }

                    match response_receiver.await {
                        Ok(response) => {
                            bot.send_message(msg.chat.id, response)
                                .parse_mode(ParseMode::MarkdownV2)
                                .disable_web_page_preview(true)
                                .await?;
                        },
                        Err(e) => {
                            bot.send_message(
                                msg.chat.id,
                                format!("Error processing command: {}", e)
                            ).await?;
                        }
                    }

                    Ok(())
                }
            },
            PhantomData::<TelegramCommand>,
        ).await;
    }

    pub async fn register_commands(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.bot.set_my_commands(TelegramCommand::bot_commands()).await?;
        Ok(())
    }
}

pub fn spawn_command_executor(
    mut budget_system: BudgetSystem,
    mut command_receiver: mpsc::Receiver<(TelegramCommand, oneshot::Sender<String>)>,
) {
    tokio::spawn(async move {
        while let Some((telegram_command, response_sender)) = command_receiver.recv().await {
            let result = execute_command(telegram_command, &mut budget_system).await;
            
            let response = match result {
                Ok(output) => {
                    // Escape markdown special characters in the output
                    crate::escape_markdown(&output)
                },
                Err(e) => format!("Error: {}", crate::escape_markdown(&e.to_string())),
            };

            if let Err(e) = response_sender.send(response) {
                eprintln!("Error sending response: {}", e);
            }

            // Save state after each command
            if let Err(e) = budget_system.save_state() {
                eprintln!("Error saving state: {}", e);
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_config::AppConfig;
    use crate::services::ethereum::MockEthereumService;
    use std::sync::Arc;

    async fn create_test_budget_system() -> BudgetSystem {
        let config = AppConfig::default();
        let ethereum_service = Arc::new(MockEthereumService);
        BudgetSystem::new(config, ethereum_service, None).await.unwrap()
    }

    #[tokio::test]
    async fn test_command_execution() {
        let (tx, rx) = mpsc::channel(100);
        let budget_system = create_test_budget_system().await;
        
        spawn_command_executor(budget_system, rx);

        // Test help command
        let (response_tx, response_rx) = oneshot::channel();
        tx.send((TelegramCommand::Help, response_tx)).await.unwrap();
        let response = response_rx.await.unwrap();
        assert!(response.contains("show available commands"));

        // Test print team report
        let (response_tx, response_rx) = oneshot::channel();
        tx.send((TelegramCommand::PrintTeamReport, response_tx)).await.unwrap();
        let response = response_rx.await.unwrap();
        assert!(response.contains("Team Report"));
    }

    #[tokio::test]
    async fn test_error_handling() {
        let (tx, rx) = mpsc::channel(100);
        let budget_system = create_test_budget_system().await;
        
        spawn_command_executor(budget_system, rx);

        // Test command with non-existent team
        let (response_tx, response_rx) = oneshot::channel();
        tx.send((
            TelegramCommand::PrintTeamParticipation(
                "NonExistentTeam".to_string(),
                "NonExistentEpoch".to_string()
            ),
            response_tx
        )).await.unwrap();

        let response = response_rx.await.unwrap();
        assert!(response.contains("Error"));
    }
}