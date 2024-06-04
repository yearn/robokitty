use ethers::prelude::*;
use eyre::{Result};
use std::{
    sync::Arc,
};
use tokio::{
    self,
    time::{sleep, Duration},
};

#[tokio::main]
async fn main() -> eyre::Result<()> {

    // Connect to reth via ipc
    let provider = Provider::connect_ipc("/tmp/reth.ipc").await?;
    let client = Arc::new(provider);

    // Get current latest block number from chain
    let latest_block = client.get_block_number().await?.as_u64();
    println!("Current block height: {}", latest_block);

    // Get randomness from latest block
    match client.get_block(latest_block).await {
        Ok(Some(block)) => {
            match block.mix_hash {
                Some(mix_hash) => {
                    println!("Randomness: {:x}", mix_hash);
                }
                None => {
                    println!("Randomness not found for block {}", latest_block);
                }
            }
        }
        Ok(None) => {
            println!("Block number {} not found", latest_block);
        }
        Err(e) => {
            eprintln!("Error fetching block {}:{:?}", latest_block, e);
        }
    }
    Ok(())
}