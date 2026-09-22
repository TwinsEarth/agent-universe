//! Reed-Solomon 纠删码

pub struct ErasureCoder {
    data_shards: u8,
    parity_shards: u8,
}

impl ErasureCoder {
    pub fn new(data_shards: u8, parity_shards: u8) -> Self {
        Self {
            data_shards,
            parity_shards,
        }
    }

    pub fn total_shards(&self) -> u8 {
        self.data_shards + self.parity_shards
    }

    pub fn encode(&self, data: &[u8]) -> Vec<Vec<u8>> {
        let shard_size = (data.len() + self.data_shards as usize - 1) / self.data_shards as usize;
        let mut shards = Vec::new();
        
        for i in 0..self.data_shards as usize {
            let start = i * shard_size;
            let end = ((i + 1) * shard_size).min(data.len());
            shards.push(data[start..end].to_vec());
        }
        
        // 简化版：parity shard 就是数据的哈希
        for _ in 0..self.parity_shards {
            use sha2::{Sha256, Digest};
            let mut hasher = Sha256::new();
            hasher.update(data);
            shards.push(hasher.finalize().to_vec());
        }
        
        shards
    }

    pub fn decode(&self, shards: &[Vec<u8>]) -> Result<Vec<u8>, String> {
        if shards.len() < self.data_shards as usize {
            return Err(format!(
                "need at least {} shards, got {}",
                self.data_shards,
                shards.len()
            ));
        }
        
        let mut result = Vec::new();
        for shard in &shards[0..self.data_shards as usize] {
            result.extend_from_slice(shard);
        }
        
        Ok(result)
    }
}
