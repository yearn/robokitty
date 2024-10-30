use ethers::prelude::*;
use std::sync::{Arc, atomic::{AtomicU64, Ordering}};
use async_trait::async_trait;
use tokio::{
    self,
    time::Duration,
};
use downcast_rs::{impl_downcast, DowncastSync};

#[async_trait]
pub trait EthereumServiceTrait: DowncastSync {
    async fn get_current_block(&self) -> Result<u64, Box<dyn std::error::Error>>;
    async fn get_randomness(&self, block_number: u64) -> Result<String, Box<dyn std::error::Error>>;
    async fn get_raffle_randomness(&self) -> Result<(u64, u64, String), Box<dyn std::error::Error>>;
}

impl_downcast!(sync EthereumServiceTrait);

pub struct EthereumService {
    client: Arc<Provider<Ipc>>,
    future_block_offset: u64,
}

pub struct MockEthereumService {
    current_block: Arc<AtomicU64>,
}

impl EthereumService {
    pub async fn new(ipc_path: &str, future_block_offset: u64) -> Result<Self, Box<dyn std::error::Error>> {
        let provider = Provider::connect_ipc(ipc_path).await?;
        Ok(Self {
            client: Arc::new(provider),
            future_block_offset,
        })
    }

    async fn get_current_block(&self) -> Result<u64, Box<dyn std::error::Error>> {
        Ok(self.client.get_block_number().await?.as_u64())
    }

    async fn get_randomness(&self, block_number: u64) -> Result<String, Box<dyn std::error::Error>> {
        let block = self.client.get_block(block_number).await?
            .ok_or("Block not found")?;
        block.mix_hash
            .ok_or_else(|| "Randomness not found".into())
            .map(|hash| format!("0x{:x}", hash))
    }

    async fn get_raffle_randomness(&self) -> Result<(u64, u64, String), Box<dyn std::error::Error>> {
        let initiation_block = self.get_current_block().await?;
        let randomness_block = initiation_block + self.future_block_offset;

        // Wait for the randomness block
        while self.get_current_block().await? < randomness_block {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        let randomness = self.get_randomness(randomness_block).await?;

        Ok((initiation_block, randomness_block, randomness))
    }
}

impl MockEthereumService {
    pub fn new() -> Self {
        Self {
            current_block: Arc::new(AtomicU64::new(12345)),
        }
    }

    pub fn increment_block(&self) {
        self.current_block.fetch_add(1, Ordering::SeqCst);
    }
}

#[async_trait]
impl EthereumServiceTrait for EthereumService {
    async fn get_current_block(&self) -> Result<u64, Box<dyn std::error::Error>> {
        Ok(self.client.get_block_number().await?.as_u64())
    }

    async fn get_randomness(&self, block_number: u64) -> Result<String, Box<dyn std::error::Error>> {
        let block = self.client.get_block(block_number).await?
            .ok_or("Block not found")?;
        block.mix_hash
            .ok_or_else(|| "Randomness not found".into())
            .map(|hash| format!("0x{:x}", hash))
    }

    async fn get_raffle_randomness(&self) -> Result<(u64, u64, String), Box<dyn std::error::Error>> {
        let initiation_block = self.get_current_block().await?;
        let randomness_block = initiation_block + self.future_block_offset;

        while self.get_current_block().await? < randomness_block {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }

        let randomness = self.get_randomness(randomness_block).await?;

        Ok((initiation_block, randomness_block, randomness))
    }
}

#[async_trait::async_trait]
impl EthereumServiceTrait for MockEthereumService {
    async fn get_current_block(&self) -> Result<u64, Box<dyn std::error::Error>> {
        Ok(self.current_block.load(Ordering::SeqCst))
    }

    async fn get_randomness(&self, block_number: u64) -> Result<String, Box<dyn std::error::Error>> {
        Ok(format!("mock_randomness_for_block_{}", block_number))
    }

    async fn get_raffle_randomness(&self) -> Result<(u64, u64, String), Box<dyn std::error::Error>> {
        let current = self.current_block.load(Ordering::SeqCst);
        Ok((current, current + 10, format!("mock_randomness_for_block_{}", current + 10)))
    }
}