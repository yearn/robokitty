use crate::core::budget_system::BudgetSystem;
use crate::commands::telegram::{TelegramCommand, parse_command, handle_command};
use teloxide::prelude::*;
use teloxide::types::ParseMode;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

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
                        
                        // Clone the command for determining parse mode
                        let parse_mode = match command.clone() {
                            TelegramCommand::MarkdownTest | TelegramCommand::PrintEpochState => Some(ParseMode::MarkdownV2),
                            _ => None,
                        };
                        
                        if let Err(e) = command_sender.send((command, response_sender)).await {
                            bot.send_message(msg.chat.id, format!("Error sending command: {}", e)).await?;
                            return Ok(());
                        }
                        
                        match response_receiver.await {
                            Ok(response) => {
                                let mut message = bot.send_message(msg.chat.id, response);
                                if let Some(mode) = parse_mode {
                                    message = message.parse_mode(mode);
                                }
                                message.disable_web_page_preview(true).await?;
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

pub fn spawn_command_executor(
    mut budget_system: BudgetSystem,
    mut command_receiver: mpsc::Receiver<(TelegramCommand, oneshot::Sender<String>)>,
) {
    std::thread::spawn(move || {
        while let Some((command, response_sender)) = command_receiver.blocking_recv() {
            let result = match command {
                TelegramCommand::PrintEpochState => budget_system.print_epoch_state(),
                TelegramCommand::MarkdownTest => Ok(budget_system.generate_markdown_test()),
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_telegram_command_parsing() {
        let (tx, _rx) = mpsc::channel(100);
        let bot = TelegramBot::new(Bot::new("dummy_token"), tx);

        assert_eq!(parse_command("/print_epoch_state"), Some(TelegramCommand::PrintEpochState));
        assert_eq!(parse_command("/markdown_test"), Some(TelegramCommand::MarkdownTest));
        assert_eq!(parse_command("/unknown_command"), None);
    }

    // Add more tests for TelegramBot functionality as needed
}