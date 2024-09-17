//src/commands/telegram.rs
use crate::core::budget_system::BudgetSystem;

#[derive(Debug, PartialEq, Clone)]
pub enum TelegramCommand {
    PrintEpochState,
    MarkdownTest,
    // Add other commands here as needed
}

pub fn parse_command(text: &str) -> Option<TelegramCommand> {
    match text {
        "/print_epoch_state" => Some(TelegramCommand::PrintEpochState),
        "/markdown_test" => Some(TelegramCommand::MarkdownTest),
        // Add other command mappings here
        _ => None,
    }
}

pub fn handle_command(command: TelegramCommand, budget_system: &mut BudgetSystem) -> Result<String, Box<dyn std::error::Error>> {
    match command {
        TelegramCommand::PrintEpochState => budget_system.print_epoch_state(),
        TelegramCommand::MarkdownTest => Ok(budget_system.generate_markdown_test()),
        // Add other command handlers here
    }
}
