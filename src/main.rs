use ethers::prelude::*;
use eyre::{Result};
use sha2::{Sha256, Digest};
use std::{
    str,
    sync::Arc,
};
use tokio::{
    self,
    time::{sleep, Duration},
};

fn draw_with(block_randomness: &str, ballot_index: u64) -> f64 {
    let combined_seed = format!("{}_{}", block_randomness, ballot_index);
    let mut hasher = Sha256::new();

    hasher.update(combined_seed.as_bytes());
    let result = hasher.finalize();

    // Convert first 8 bytes of the hash to a u64
    let hash_num = u64::from_be_bytes(result[..8].try_into().unwrap());
    let max_num = u64::MAX as f64;
    hash_num as f64 / max_num
}

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

    let test_randomness = "0xd0cb380f49b60f392631607e78ba2cd1094fa8069918edcfc97455b7ad029db4";
    let test_index: u64 = 0;
    println!("Test draw output:{}", draw_with(test_randomness, test_index));

    Ok(())
}