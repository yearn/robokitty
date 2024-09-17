//src/commands/telegram.rs
use crate::core::budget_system::BudgetSystem;

#[derive(Debug, PartialEq, Clone)]
pub enum TelegramCommand {
    PrintEpochState,
    MarkdownTest,
    // Add other commands here as needed
}

pub fn parse_command(text: &str) -> Option<TelegramCommand> {
    match text.trim() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::budget_system::BudgetSystem;
    use crate::app_config::AppConfig;
    use crate::services::ethereum::MockEthereumService;
    use std::sync::Arc;
    use chrono::Utc;

    // Helper function to create a test BudgetSystem
    async fn create_test_budget_system() -> BudgetSystem {
        let config = AppConfig::default();
        let ethereum_service = Arc::new(MockEthereumService);
        BudgetSystem::new(config, ethereum_service, None).await.unwrap()
    }

    #[test]
    fn test_parse_command_valid() {
        assert_eq!(parse_command("/print_epoch_state"), Some(TelegramCommand::PrintEpochState));
        assert_eq!(parse_command("/markdown_test"), Some(TelegramCommand::MarkdownTest));
    }

    #[test]
    fn test_parse_command_unknown() {
        assert_eq!(parse_command("/unknown_command"), None);
    }

    #[test]
    fn test_parse_command_with_whitespace() {
        assert_eq!(parse_command("  /print_epoch_state  "), Some(TelegramCommand::PrintEpochState));
        assert_eq!(parse_command("  /markdown_test  "), Some(TelegramCommand::MarkdownTest));
    }

    #[test]
    fn test_parse_command_case_sensitive() {
        assert_eq!(parse_command("/PRINT_EPOCH_STATE"), None);
        assert_eq!(parse_command("/Markdown_Test"), None);
    }

    #[tokio::test]
    async fn test_handle_command_print_epoch_state() {
        let mut budget_system = create_test_budget_system().await;
        
        // Create and activate an epoch
        let start_date = Utc::now();
        let end_date = start_date + chrono::Duration::days(30);
        let epoch_id = budget_system.create_epoch("Test Epoch", start_date, end_date).unwrap();
        budget_system.activate_epoch(epoch_id).unwrap();

        let result = handle_command(TelegramCommand::PrintEpochState, &mut budget_system);
        assert!(result.is_ok());
        // We can't easily predict the exact output, but we can check that it's not empty
        assert!(!result.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_handle_command_markdown_test() {
        let mut budget_system = create_test_budget_system().await;
        let result = handle_command(TelegramCommand::MarkdownTest, &mut budget_system);
        assert!(result.is_ok());
        let output = result.unwrap();
        // Check that the output contains some expected Markdown elements
        assert!(output.contains("*Bold text*"));
        assert!(output.contains("_Italic text_"));
        assert!(output.contains("`inline fixed-width code`"));
    }

    #[tokio::test]
    async fn test_handle_command_error() {
        let mut budget_system = create_test_budget_system().await;
        // We don't create an epoch here, so this should result in an error
        let result = handle_command(TelegramCommand::PrintEpochState, &mut budget_system);
        assert!(result.is_err());
    }
}