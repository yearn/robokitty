// src/telegram_bot.rs

use crate::BudgetSystem;
use teloxide::prelude::*;
use teloxide::types::ParseMode::MarkdownV2;
use teloxide::utils::markdown::escape;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use std::error::Error;

pub enum TelegramCommand {
    PrintEpochState,
    // Add other commands here as needed
}
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
                        if let Err(e) = command_sender.send((command, response_sender)).await {
                            bot.send_message(msg.chat.id, format!("Error sending command: {}", e)).await?;
                            return Ok(());
                        }
                        
                        match response_receiver.await {
                            Ok(response) => {
                                let escaped_response = escape(&response);
                                bot.send_message(msg.chat.id, escaped_response)
                                .parse_mode(MarkdownV2)
                                .disable_web_page_preview(true)
                                .await?;
                            }
                            Err(e) => {
                                bot.send_message(msg.chat.id, format!("Error receiving response: {}", e)).await?;
                            }
                        }
                    } else {
                        bot.send_message(msg.chat.id, "Unknown command").await?;
                    }
                }
                Ok(())
            }
        })
        .await;
    }
}

fn parse_command(text: &str) -> Option<TelegramCommand> {
    match text {
        "/print_epoch_state" => Some(TelegramCommand::PrintEpochState),
        // Add other command mappings here
        _ => None,
    }
}

pub fn spawn_command_executor(
    mut budget_system: BudgetSystem,
    mut command_receiver: mpsc::Receiver<(TelegramCommand, oneshot::Sender<String>)>,
) {
    std::thread::spawn(move || {
        while let Some((command, response_sender)) = command_receiver.blocking_recv() {
            let result = match command {
                TelegramCommand::PrintEpochState => budget_system.print_epoch_state(),
                // Add other command executions here
            };

            let response = match result {
                Ok(output) => output,
                Err(e) => format!("Error executing command: {}", e),
            };

            if let Err(e) = response_sender.send(response) {
                eprintln!("Error sending response: {}", e);
            }
        }
    });
}