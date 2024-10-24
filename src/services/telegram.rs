use crate::core::budget_system::BudgetSystem;
use crate::commands::telegram::{TelegramCommand, execute_command};
use teloxide::{
    prelude::*,
    utils::command::BotCommands,
    types::{LinkPreviewOptions, ParseMode},
    dispatching::{
        UpdateFilterExt,
        dialogue::{InMemStorage, Storage},
    },
};
use tokio::sync::{mpsc, oneshot};
use std::error::Error;

pub struct TelegramBot {
    bot: Bot,
    command_sender: mpsc::Sender<(TelegramCommand, oneshot::Sender<String>)>,
}

impl TelegramBot {
    pub fn new(bot: Bot, command_sender: mpsc::Sender<(TelegramCommand, oneshot::Sender<String>)>) -> Self {
        Self { bot, command_sender }
    }

    pub async fn run(self) {
        let handler = Update::filter_message()
            .filter_command::<TelegramCommand>()
            .chain(dptree::endpoint(
                move |bot: Bot, msg: Message, cmd: TelegramCommand| {
                    let command_sender = self.command_sender.clone();
                    async move {
                        let (response_sender, response_receiver) = oneshot::channel();
                        
                        if let Err(e) = command_sender.send((cmd, response_sender)).await {
                            bot.send_message(
                                msg.chat.id,
                                format!("Error sending command: {}", e)
                            ).await?;
                            return Ok(()) as Result<(), Box<dyn Error + Send + Sync>>;
                        }
    
                        match response_receiver.await {
                            Ok(response) => {
                                bot.send_message(msg.chat.id, response)
                                    .parse_mode(ParseMode::MarkdownV2)
                                    .link_preview_options(LinkPreviewOptions { 
                                        is_disabled: true, 
                                        url: None, 
                                        prefer_small_media: false, 
                                        prefer_large_media: false, 
                                        show_above_text: false 
                                    })
                                    .await?;
                            },
                            Err(e) => {
                                bot.send_message(
                                    msg.chat.id,
                                    format!("Error processing command: {}", e)
                                ).await?;
                            }
                        }
    
                        Ok(()) as Result<(), Box<dyn Error + Send + Sync>>
                    }
                }
            ));
    
        Dispatcher::builder(self.bot, handler)
            .dependencies(dptree::deps![InMemStorage::<()>::new()])
            .enable_ctrlc_handler()
            .build()
            .dispatch()
            .await;
    }

    pub async fn register_commands(&self) -> Result<(), Box<dyn Error>> {
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
                Ok(output) => crate::escape_markdown(&output),
                Err(e) => format!("Error: {}", crate::escape_markdown(&e.to_string())),
            };

            if let Err(e) = response_sender.send(response) {
                log::error!("Error sending response: {}", e);
            }

            if let Err(e) = budget_system.save_state() {
                log::error!("Error saving state: {}", e);
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
            TelegramCommand::PrintTeamParticipation {
                team_name: "NonExistentTeam".to_string(),
                epoch_name: "NonExistentEpoch".to_string()
            },
            response_tx
        )).await.unwrap();

        let response = response_rx.await.unwrap();
        assert!(response.contains("Error"));
    }
}