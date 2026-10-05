//! Reed-Solomon 纠删码
//!
//! 基于 `reed-solomon-erasure`（GF(2^8)）的**真实** RS 编码：
//! 数据分为 `data_shards` 个等长数据片，并计算 `parity_shards` 个校验片。
//! 只要**任意 `data_shards` 个分片（数据/校验均可）存活**即可重建全部数据，
//! 即最多容忍 `parity_shards` 个分片丢失。

use reed_solomon_erasure::galois_8::ReedSolomon;

pub struct ErasureCoder {
    data_shards: usize,
    parity_shards: usize,
    rs: ReedSolomon,
}

#[derive(Debug, Clone)]
pub struct DecodedShard {
    pub index: u8,
    pub data: Vec<u8>,
    pub is_parity: bool,
}

impl ErasureCoder {
    pub fn new(data_shards: u8, parity_shards: u8) -> Result<Self, String> {
        if data_shards == 0 || parity_shards == 0 {
            return Err("data_shards 和 parity_shards 必须 > 0".into());
        }
        let (d, p) = (data_shards as usize, parity_shards as usize);
        if d + p > 256 {
            return Err("GF(2^8) 最多 256 个分片".into());
        }
        let rs = ReedSolomon::new(d, p).map_err(|e| format!("reed-solomon 参数: {e}"))?;
        Ok(Self {
            data_shards: d,
            parity_shards: p,
            rs,
        })
    }

    pub fn total_shards(&self) -> u8 {
        (self.data_shards + self.parity_shards) as u8
    }
    pub fn data_shards(&self) -> u8 {
        self.data_shards as u8
    }
    pub fn parity_shards(&self) -> u8 {
        self.parity_shards as u8
    }

    /// 编码：data_shards 个等长数据片（不足补 0）+ parity_shards 个真实 RS 校验片。
    pub fn encode(&self, data: &[u8]) -> Result<Vec<DecodedShard>, String> {
        let total = self.data_shards + self.parity_shards;
        let shard_size = data.len().div_ceil(self.data_shards).max(1);

        // 所有分片预分配等长缓冲（RS 在定长缓冲上运算）
        let mut shards: Vec<Vec<u8>> = (0..total).map(|_| vec![0u8; shard_size]).collect();

        // 按顺序铺入数据，剩余字节保持 0（padding）
        for (i, byte) in data.iter().enumerate() {
            shards[i / shard_size][i % shard_size] = *byte;
        }

        self.rs
            .encode(&mut shards)
            .map_err(|e| format!("reed-solomon encode: {e}"))?;

        Ok(shards
            .into_iter()
            .enumerate()
            .map(|(i, s)| DecodedShard {
                index: i as u8,
                data: s,
                is_parity: i >= self.data_shards,
            })
            .collect())
    }

    /// 解码重建：按 index 放置存活分片、缺失置 None，由 RS 重建；
    /// 丢失分片数 ≤ parity_shards 时成功（含数据片丢失、靠校验片重建）。
    pub fn decode(&self, shards: &[DecodedShard], original_size: usize) -> Result<Vec<u8>, String> {
        let total = self.data_shards + self.parity_shards;
        let mut present: Vec<Option<Vec<u8>>> = vec![None; total];
        for s in shards {
            let i = s.index as usize;
            if i < total {
                present[i] = Some(s.data.clone());
            }
        }

        self.rs
            .reconstruct(&mut present)
            .map_err(|e| format!("reed-solomon reconstruct: {e}"))?;

        let mut result = Vec::with_capacity(original_size);
        for shard in present.iter().take(self.data_shards) {
            let bytes = shard.as_ref().ok_or("reconstruct 后数据片仍缺失")?;
            result.extend_from_slice(bytes);
        }
        result.truncate(original_size);
        Ok(result)
    }

    /// 用 RS verify 校验校验片是否与数据片一致；任一分片缺失即 false。
    pub fn verify_parity(&self, shards: &[DecodedShard]) -> bool {
        let total = self.data_shards + self.parity_shards;
        let mut ordered: Vec<Vec<u8>> = vec![Vec::new(); total];
        for s in shards {
            let i = s.index as usize;
            if i < total {
                ordered[i] = s.data.clone();
            }
        }
        // 真实分片长度恒 ≥ 1；空 Vec 表示该 index 缺失
        if ordered.iter().any(|s| s.is_empty()) {
            return false;
        }
        self.rs.verify(&ordered).unwrap_or(false)
    }
}

pub mod distributed;
pub mod wire;
