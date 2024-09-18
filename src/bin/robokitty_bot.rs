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