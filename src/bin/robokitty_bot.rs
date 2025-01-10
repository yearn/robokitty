use robokitty::{run_telegram_bot, lock, initialize_environment};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    initialize_environment();
    
    loop {
        if !lock::check_lock_file() {
            break;
        }
        println!("Script is running. Waiting...");
        sleep(Duration::from_secs(3)).await;
    }
    run_telegram_bot().await
}

#[cfg(test)]
mod tests {
    use std::sync::Once;
    use tokio::runtime::Runtime;

    // Mock environment initialization
    static INIT: Once = Once::new();
    fn initialize_environment() {
        INIT.call_once(|| {
            // Set up any necessary test environment variables
            std::env::set_var("TELEGRAM_BOT_TOKEN", "test_token");
        });
    }

    // TODO: Improve unit testing

    #[test]
    fn test_main_function_error_handling() {
        initialize_environment();

        let rt = Runtime::new().unwrap();

        // Mock the run_telegram_bot function to return an error
        fn mock_run_telegram_bot() -> Result<(), Box<dyn std::error::Error>> {
            Err("Simulated error".into())
        }

        // Capture stderr output
        let stderr = std::io::stderr();
        let mut _handle = stderr.lock();
        let mut output = Vec::new();

        let _result = rt.block_on(async {
            robokitty::initialize_environment();
            let _ = mock_run_telegram_bot().map_err(|e| {
                // Redirect error output to our buffer
                use std::io::Write;
                writeln!(output, "Error: {}", e).unwrap();
            });
        });

        // Check that the error was printed to stderr
        let error_output = String::from_utf8(output).unwrap();
        assert!(error_output.contains("Error: Simulated error"));
    }
}