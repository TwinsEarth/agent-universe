//! 内容寻址镜像块清单（v3.7.1）。
//!
//! AUSec「镜像按需加载」的第一块拼图：把一个镜像描述成**严格连续布局**的定长块
//! 序列，每块带自己的 SHA-256 内容地址（偏移 / 长度 / sha256 / 顺序）。
//!
//! 本模块只做三件可独立验证的事，且全部是确定性的纯逻辑，不做任何网络/磁盘 I/O：
//!
//! 1. [`build_manifest`]：从镜像字节按定长切块，计算每块 sha256，产出清单；
//! 2. [`ChunkManifest::validate`]：解析后的结构完整性基线（顺序、连续偏移、长度、
//!    摘要形状、总长度）；
//! 3. [`ChunkManifest::verify_chunk`] / [`ChunkManifest::verify_image`]：按需取块后
//!    对**实际取到的字节**重算 sha256，坏块/错长/篡改一律具名拒绝。
//!
//! 诚实边界：本模块不联网拉块、不实现 BlockStore（v3.7.2）、不对块做 Ed25519 签名
//! 锚定（v3.7.3）。它只保证「清单自身自洽」与「字节与清单声明一致」，不保证块来自
//! 可信发布者——那是 v3.7.3 的签名职责。
//!
//! 见 `docs/ausec/AUSEC-DESIGN.md` §3.1（3.7.1）。

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// 单块名义大小上限（64 MiB）。防止畸形清单声明超大块导致按需读取方一次性分配巨量内存。
pub const MAX_CHUNK_SIZE: u64 = 64 * 1024 * 1024;

/// sha256 小写十六进制长度。
pub const DIGEST_HEX_LEN: usize = 64;

/// 计算一段字节的 sha256 小写十六进制（块的内容地址）。
pub fn digest_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// 判断字符串是否为合法的 sha256 小写十六进制摘要。
fn is_sha256_hex(s: &str) -> bool {
    s.len() == DIGEST_HEX_LEN
        && s.bytes().all(|b| b.is_ascii_hexdigit())
        && !s.bytes().any(|b| b.is_ascii_uppercase())
}

/// 单个镜像块的清单条目：内容地址 + 在镜像内的严格布局。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkEntry {
    /// 块序号，必须从 0 开始、严格等于其在 `chunks` 中的位置（顺序不可乱）。
    pub index: u32,
    /// 块在镜像内的字节偏移，必须从 0 开始、严格连续（无空洞/无重叠）。
    pub offset: u64,
    /// 块字节数；除最后一块外必须等于清单的 `chunk_size`，任何块都必须 > 0。
    pub length: u64,
    /// 块字节的 sha256 小写十六进制（内容寻址 id）。
    pub sha256: String,
}

/// 内容寻址镜像块清单。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChunkManifest {
    /// 镜像逻辑标识（非空），与块内容地址解耦。
    pub image: String,
    /// 名义满块大小（> 0 且 ≤ [`MAX_CHUNK_SIZE`]）。
    pub chunk_size: u64,
    /// 镜像总字节数；空镜像为 0 且 `chunks` 为空。
    pub total_length: u64,
    /// 按顺序排列的块条目。
    pub chunks: Vec<ChunkEntry>,
}

/// 清单构建/校验/完整性错误，全部具名，不含 panic 路径。
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ManifestError {
    #[error("镜像 id 为空")]
    EmptyImageId,
    #[error("非法 chunk_size：必须在 1..={max} 字节之间", max = MAX_CHUNK_SIZE)]
    InvalidChunkSize,
    #[error("非空镜像至少需要一个块")]
    NoChunks,
    #[error("第 {index} 块序号错误：应为 {expected}，实际 {got}")]
    BadIndex {
        index: usize,
        expected: u32,
        got: u32,
    },
    #[error("第 {index} 块偏移不连续：应为 {expected}，实际 {got}")]
    BadOffset {
        index: usize,
        expected: u64,
        got: u64,
    },
    #[error("第 {index} 块长度非法：{length}（满块应为 {chunk_size}）")]
    BadLength {
        index: usize,
        length: u64,
        chunk_size: u64,
    },
    #[error("第 {index} 块 sha256 不是 64 位小写十六进制")]
    BadDigestHex { index: usize },
    #[error("total_length={declared} 与按布局计算的 {computed} 不一致")]
    TotalLengthMismatch { declared: u64, computed: u64 },
    #[error("清单 JSON 不合法：{0}")]
    MalformedJson(String),
    #[error("块序号 {index} 越界（共 {count} 块）")]
    ChunkIndexOutOfRange { index: usize, count: usize },
    #[error("第 {index} 块字节数不符：清单 {declared}，实得 {actual}")]
    ChunkLengthMismatch {
        index: usize,
        declared: u64,
        actual: u64,
    },
    #[error("第 {index} 块 sha256 不匹配（内容损坏或被篡改）")]
    ChunkDigestMismatch { index: usize },
    #[error("镜像总字节数不符：清单 total_length={declared}，实得 {actual}")]
    ImageLengthMismatch { declared: u64, actual: u64 },
}

/// 从镜像字节按定长切块构建内容寻址清单。
///
/// - `image` 必须非空；`chunk_size` 必须在 `1..=MAX_CHUNK_SIZE`；
/// - 空镜像（`data` 为空）产出零块、`total_length = 0` 的合法空清单；
/// - 除最后一块外每块恰为 `chunk_size`，偏移严格连续。
pub fn build_manifest(
    image: &str,
    data: &[u8],
    chunk_size: u64,
) -> Result<ChunkManifest, ManifestError> {
    if image.trim().is_empty() {
        return Err(ManifestError::EmptyImageId);
    }
    if chunk_size == 0 || chunk_size > MAX_CHUNK_SIZE {
        return Err(ManifestError::InvalidChunkSize);
    }
    let size = chunk_size as usize;
    let mut chunks = Vec::new();
    let mut offset: u64 = 0;
    for (i, part) in data.chunks(size).enumerate() {
        let length = part.len() as u64;
        chunks.push(ChunkEntry {
            index: i as u32,
            offset,
            length,
            sha256: digest_hex(part),
        });
        offset += length;
    }
    let total_length = offset;
    let manifest = ChunkManifest {
        image: image.to_string(),
        chunk_size,
        total_length,
        chunks,
    };
    // 自建即自洽：立即用同一结构基线复核，避免构建逻辑与校验逻辑漂移。
    manifest.validate()?;
    Ok(manifest)
}

impl ChunkManifest {
    /// 从 JSON 字节解析清单并立即做结构完整性校验。
    pub fn parse_and_validate(json_bytes: &[u8]) -> Result<ChunkManifest, ManifestError> {
        let manifest: ChunkManifest = serde_json::from_slice(json_bytes)
            .map_err(|e| ManifestError::MalformedJson(e.to_string()))?;
        manifest.validate()?;
        Ok(manifest)
    }

    /// 结构完整性基线：顺序、连续偏移、长度、摘要形状、总长度。
    ///
    /// 这是「按需取块」之前唯一的入场闸：任何畸形清单都不得进入 BlockStore。
    pub fn validate(&self) -> Result<(), ManifestError> {
        if self.image.trim().is_empty() {
            return Err(ManifestError::EmptyImageId);
        }
        if self.chunk_size == 0 || self.chunk_size > MAX_CHUNK_SIZE {
            return Err(ManifestError::InvalidChunkSize);
        }

        // 空镜像：必须恰好零块、总长 0。
        if self.chunks.is_empty() {
            return if self.total_length == 0 {
                Ok(())
            } else {
                Err(ManifestError::TotalLengthMismatch {
                    declared: self.total_length,
                    computed: 0,
                })
            };
        }

        let mut expected_offset: u64 = 0;
        let last = self.chunks.len() - 1;
        for (i, entry) in self.chunks.iter().enumerate() {
            if entry.index as usize != i {
                return Err(ManifestError::BadIndex {
                    index: i,
                    expected: i as u32,
                    got: entry.index,
                });
            }
            if entry.offset != expected_offset {
                return Err(ManifestError::BadOffset {
                    index: i,
                    expected: expected_offset,
                    got: entry.offset,
                });
            }
            if entry.length == 0 {
                return Err(ManifestError::BadLength {
                    index: i,
                    length: 0,
                    chunk_size: self.chunk_size,
                });
            }
            if i != last {
                // 非末块必须恰为满块，保证「连续 + 满块」⇒ 无空洞无重叠。
                if entry.length != self.chunk_size {
                    return Err(ManifestError::BadLength {
                        index: i,
                        length: entry.length,
                        chunk_size: self.chunk_size,
                    });
                }
            } else if entry.length > self.chunk_size {
                // 末块也不得超过名义块大小。
                return Err(ManifestError::BadLength {
                    index: i,
                    length: entry.length,
                    chunk_size: self.chunk_size,
                });
            }
            if !is_sha256_hex(&entry.sha256) {
                return Err(ManifestError::BadDigestHex { index: i });
            }
            expected_offset += entry.length;
        }

        if expected_offset != self.total_length {
            return Err(ManifestError::TotalLengthMismatch {
                declared: self.total_length,
                computed: expected_offset,
            });
        }
        Ok(())
    }

    /// 校验按需取回的单个块：序号存在、字节数与清单一致、sha256 内容地址匹配。
    pub fn verify_chunk(&self, index: usize, data: &[u8]) -> Result<(), ManifestError> {
        let entry = self
            .chunks
            .get(index)
            .ok_or(ManifestError::ChunkIndexOutOfRange {
                index,
                count: self.chunks.len(),
            })?;
        if data.len() as u64 != entry.length {
            return Err(ManifestError::ChunkLengthMismatch {
                index,
                declared: entry.length,
                actual: data.len() as u64,
            });
        }
        if digest_hex(data) != entry.sha256 {
            return Err(ManifestError::ChunkDigestMismatch { index });
        }
        Ok(())
    }

    /// 校验完整镜像字节：总长一致，且按每块偏移/长度切片后逐块内容地址匹配。
    pub fn verify_image(&self, data: &[u8]) -> Result<(), ManifestError> {
        self.validate()?;
        if data.len() as u64 != self.total_length {
            return Err(ManifestError::ImageLengthMismatch {
                declared: self.total_length,
                actual: data.len() as u64,
            });
        }
        for (i, entry) in self.chunks.iter().enumerate() {
            let start = entry.offset as usize;
            let end = start + entry.length as usize;
            // 上面已保证 end <= total_length == data.len()，切片不会越界。
            self.verify_chunk(i, &data[start..end])?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 造一段确定性、跨块内容不同的镜像字节。
    fn sample_data(full_chunks: usize, chunk_size: u64, tail: usize) -> Vec<u8> {
        let total = full_chunks as u64 * chunk_size + tail as u64;
        (0..total).map(|i| (i % 251) as u8).collect()
    }

    #[test]
    fn build_and_verify_roundtrip() {
        let data = sample_data(3, 4, 3); // 4,4,4,3 → 15 字节
        let m = build_manifest("img/demo", &data, 4).unwrap();
        assert_eq!(m.chunks.len(), 4);
        assert_eq!(m.total_length, 15);
        assert_eq!(m.chunks[0].offset, 0);
        assert_eq!(m.chunks[3].offset, 12);
        assert_eq!(m.chunks[3].length, 3);
        // 完整镜像逐块校验通过。
        m.verify_image(&data).unwrap();
        // 单块校验通过。
        m.verify_chunk(3, &data[12..15]).unwrap();
    }

    #[test]
    fn empty_image_is_valid_zero_chunk_manifest() {
        let m = build_manifest("img/empty", &[], 1024).unwrap();
        assert_eq!(m.chunks.len(), 0);
        assert_eq!(m.total_length, 0);
        m.validate().unwrap();
        m.verify_image(&[]).unwrap();
        // 空清单不接受非零 total_length。
        let mut bad = m.clone();
        bad.total_length = 1;
        assert!(matches!(
            bad.validate(),
            Err(ManifestError::TotalLengthMismatch { .. })
        ));
    }

    #[test]
    fn rejects_invalid_chunk_size_and_image_id() {
        assert!(matches!(
            build_manifest("img", &[1, 2, 3], 0),
            Err(ManifestError::InvalidChunkSize)
        ));
        assert!(matches!(
            build_manifest("img", &[1, 2, 3], MAX_CHUNK_SIZE + 1),
            Err(ManifestError::InvalidChunkSize)
        ));
        assert!(matches!(
            build_manifest("   ", &[1], 4),
            Err(ManifestError::EmptyImageId)
        ));
    }

    #[test]
    fn corrupted_chunk_byte_is_detected() {
        let data = sample_data(2, 8, 5);
        let mut m = build_manifest("img/x", &data, 8).unwrap();
        let mut tampered = data.clone();
        tampered[0] ^= 0xFF;
        // 整镜像校验必须在第 0 块抓到篡改。
        assert!(matches!(
            m.verify_image(&tampered),
            Err(ManifestError::ChunkDigestMismatch { index: 0 })
        ));
        // 直接伪造清单里的摘要也过不了（用假摘要对真数据）。
        m.chunks[1].sha256 = "0".repeat(64);
        assert!(matches!(
            m.verify_image(&data),
            Err(ManifestError::ChunkDigestMismatch { index: 1 })
        ));
    }

    #[test]
    fn rejects_gap_overlap_and_bad_order() {
        let data = sample_data(3, 4, 0);
        let mut m = build_manifest("img/y", &data, 4).unwrap();

        // 空洞：第 1 块偏移跳了一格。
        m.chunks[1].offset = 8;
        assert!(matches!(
            m.validate(),
            Err(ManifestError::BadOffset {
                index: 1,
                expected: 4,
                got: 8
            })
        ));

        // 重叠：第 2 块偏移回退。
        let mut m2 = build_manifest("img/y", &data, 4).unwrap();
        m2.chunks[2].offset = 4;
        assert!(matches!(
            m2.validate(),
            Err(ManifestError::BadOffset {
                index: 2,
                expected: 8,
                ..
            })
        ));

        // 乱序：index 与位置不符。
        let mut m3 = build_manifest("img/y", &data, 4).unwrap();
        m3.chunks.swap(0, 1);
        assert!(matches!(
            m3.validate(),
            Err(ManifestError::BadIndex { index: 0, .. })
        ));
    }

    #[test]
    fn rejects_bad_lengths_and_total() {
        let data = sample_data(3, 4, 0);
        let mut m = build_manifest("img/z", &data, 4).unwrap();

        // 非末块长度不等于 chunk_size。
        m.chunks[0].length = 3;
        assert!(matches!(
            m.validate(),
            Err(ManifestError::BadLength { index: 0, .. })
        ));

        // 零长度块。
        let mut m2 = build_manifest("img/z", &data, 4).unwrap();
        m2.chunks[2].length = 0;
        assert!(matches!(
            m2.validate(),
            Err(ManifestError::BadLength {
                index: 2,
                length: 0,
                ..
            })
        ));

        // total_length 与布局不符。
        let mut m3 = build_manifest("img/z", &data, 4).unwrap();
        m3.total_length = 99;
        assert!(matches!(
            m3.validate(),
            Err(ManifestError::TotalLengthMismatch { declared: 99, .. })
        ));
    }

    #[test]
    fn rejects_bad_digest_shape() {
        let data = sample_data(1, 4, 1);
        let mut m = build_manifest("img/h", &data, 4).unwrap();
        m.chunks[0].sha256 = "ABC".to_string(); // 非 64 位
        assert!(matches!(
            m.validate(),
            Err(ManifestError::BadDigestHex { index: 0 })
        ));
        // 大写十六进制也不接受（强制规范化小写）。
        m.chunks[0].sha256 = "A".repeat(64);
        assert!(matches!(
            m.validate(),
            Err(ManifestError::BadDigestHex { index: 0 })
        ));
    }

    #[test]
    fn verify_chunk_catches_length_and_range() {
        let data = sample_data(2, 4, 2);
        let m = build_manifest("img/l", &data, 4).unwrap();
        // 多给一个字节。
        assert!(matches!(
            m.verify_chunk(0, &data[0..5]),
            Err(ManifestError::ChunkLengthMismatch {
                index: 0,
                declared: 4,
                actual: 5
            })
        ));
        // 越界序号。
        assert!(matches!(
            m.verify_chunk(9, &[]),
            Err(ManifestError::ChunkIndexOutOfRange { index: 9, count: 3 })
        ));
        // 整镜像长度不符。
        assert!(matches!(
            m.verify_image(&data[..9]),
            Err(ManifestError::ImageLengthMismatch {
                declared: 10,
                actual: 9
            })
        ));
    }

    #[test]
    fn json_parse_and_validate_roundtrip_and_malformed() {
        let data = sample_data(2, 16, 7);
        let m = build_manifest("img/j", &data, 16).unwrap();
        let json = serde_json::to_vec(&m).unwrap();
        let parsed = ChunkManifest::parse_and_validate(&json).unwrap();
        assert_eq!(parsed, m);
        parsed.verify_image(&data).unwrap();

        // 坏 JSON。
        assert!(matches!(
            ChunkManifest::parse_and_validate(b"{not json"),
            Err(ManifestError::MalformedJson(_))
        ));
        // JSON 合法但结构非法（offset 有空洞）。
        let mut bad = m.clone();
        bad.chunks[1].offset += 1;
        let bad_json = serde_json::to_vec(&bad).unwrap();
        assert!(matches!(
            ChunkManifest::parse_and_validate(&bad_json),
            Err(ManifestError::BadOffset { index: 1, .. })
        ));
    }
}
