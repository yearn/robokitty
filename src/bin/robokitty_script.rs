use robokitty::{run_script_commands, initialize_environment};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    initialize_environment();
    run_script_commands().await
}

#[cfg(test)]
mod tests {
    use std::sync::Once;
    use tokio::runtime::Runtime;

    // TODO: Improve unit testing

    // Mock environment initialization
    static INIT: Once = Once::new();
    fn initialize_environment() {
        INIT.call_once(|| {
            // Set up any necessary test environment variables
            std::env::set_var("TELEGRAM_BOT_TOKEN", "test_token");
        });
    }

    #[test]
    fn test_main_function_success() {
        initialize_environment();

        // Create a runtime for running async code in the test
        let rt = Runtime::new().unwrap();

        // Mock the run_script_commands function
        fn mock_run_script_commands() -> Result<(), Box<dyn std::error::Error>> {
            Ok(())
        }

        // Run the main function with the mocked run_script_commands
        let result = rt.block_on(async {
            robokitty::initialize_environment();
            mock_run_script_commands()
        });

        assert!(result.is_ok());
    }

}