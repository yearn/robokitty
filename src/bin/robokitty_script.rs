use robokitty::{run_script_commands, initialize_environment};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    initialize_environment();
    run_script_commands().await
}