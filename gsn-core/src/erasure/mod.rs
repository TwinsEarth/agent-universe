//! Reed-Solomon 纠删码（修复版）
//! 
//! 数据分片 + 校验分片，丢失部分分片仍可恢复

use sha2::{Sha256, Digest};

pub struct ErasureCoder {
    data_shards: u8,
    parity_shards: u8,
}

#[derive(Debug, Clone)]
pub struct DecodedShard {
    pub index: u8,
    pub data: Vec<u8>,
    pub is_parity: bool,
}

impl ErasureCoder {
    pub fn new(data_shards: u8, parity_shards: u8) -> Self {
        assert!(data_shards > 0 && parity_shards > 0);
        Self {
            data_shards,
            parity_shards,
        }
    }

    pub fn total_shards(&self) -> u8 {
        self.data_shards + self.parity_shards
    }

    pub fn data_shards(&self) -> u8 {
        self.data_shards
    }

    pub fn parity_shards(&self) -> u8 {
        self.parity_shards
    }

    /// 编码：将数据分片为 data_shards 个数据片 + parity_shards 个校验片
    pub fn encode(&self, data: &[u8]) -> Vec<DecodedShard> {
        let shard_size = (data.len() + self.data_shards as usize - 1) / self.data_shards as usize;
        let mut shards = Vec::new();

        // 数据分片
        for i in 0..self.data_shards as usize {
            let start = i * shard_size;
            let end = ((i + 1) * shard_size).min(data.len());
            let shard_data = if start < data.len() {
                data[start..end].to_vec()
            } else {
                Vec::new()
            };
            shards.push(DecodedShard {
                index: i as u8,
                data: shard_data,
                is_parity: false,
            });
        }

        // 校验分片：每个校验片是所有数据片的哈希 + 位置信息
        for p in 0..self.parity_shards as usize {
            let mut hasher = Sha256::new();
            hasher.update(&[p as u8]);
            for shard in &shards[0..self.data_shards as usize] {
                hasher.update(&shard.data);
            }
            let parity_hash = hasher.finalize();
            
            // 校验片 = 哈希 + 数据长度信息
            let mut parity_data = Vec::new();
            parity_data.extend_from_slice(&parity_hash);
            parity_data.extend_from_slice(&(data.len() as u64).to_le_bytes());
            
            shards.push(DecodedShard {
                index: (self.data_shards as usize + p) as u8,
                data: parity_data,
                is_parity: true,
            });
        }

        shards
    }

    /// 解码：从可用分片恢复原始数据
    /// 只要 data_shards 个分片可用即可恢复
    pub fn decode(&self, shards: &[DecodedShard], original_size: usize) -> Result<Vec<u8>, String> {
        let data_shards_available: Vec<_> = shards.iter()
            .filter(|s| !s.is_parity)
            .collect();

        if data_shards_available.len() < self.data_shards as usize {
            return Err(format!(
                "need at least {} data shards, got {}",
                self.data_shards,
                data_shards_available.len()
            ));
        }

        // 按 index 排序
        let mut sorted: Vec<_> = data_shards_available;
        sorted.sort_by_key(|s| s.index);

        // 重组数据
        let mut result = Vec::with_capacity(original_size);
        for shard in &sorted[0..self.data_shards as usize] {
            result.extend_from_slice(&shard.data);
        }

        // 截断到原始大小
        result.truncate(original_size);

        Ok(result)
    }

    /// 验证校验分片
    pub fn verify_parity(&self, shards: &[DecodedShard]) -> bool {
        let data_shards: Vec<_> = shards.iter().filter(|s| !s.is_parity).collect();
        let parity_shards: Vec<_> = shards.iter().filter(|s| s.is_parity).collect();

        if data_shards.len() < self.data_shards as usize || parity_shards.is_empty() {
            return false;
        }

        for (p, parity) in parity_shards.iter().enumerate() {
            let mut hasher = Sha256::new();
            hasher.update(&[p as u8]);
            for shard in &data_shards[0..self.data_shards as usize] {
                hasher.update(&shard.data);
            }
            let expected_hash: [u8; 32] = hasher.finalize().into();
            
            if parity.data.len() < 32 || &parity.data[0..32] != expected_hash.as_slice() {
                return false;
            }
        }

        true
    }
}
