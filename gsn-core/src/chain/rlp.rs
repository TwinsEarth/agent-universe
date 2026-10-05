//! 手写 RLP（Recursive Length Prefix）编解码——Ethereum 的线格式。
//!
//! 只实现本仓库需要的最小子集（足够编码 EIP-1559 交易与 EIP-712/3009 辅助载荷），
//! 不引入 ethereum-rlp 依赖。
//!
//! # 规则（以太坊黄皮书 Appendix B）
//!
//! - 单字节 `[0x00, 0x7f]`：编码即自身；
//! - 短字符串（0–55 字节）：`0x80 + len` 后接内容；
//! - 长字符串（>55 字节）：`0xb7 + len_of_len` 后接大端长度，再接内容；
//! - 短列表（载荷 0–55 字节）：`0xc0 + payload_len` 后接子项串联；
//! - 长列表（载荷 >55 字节）：`0xf7 + len_of_len` 后接大端长度，再接子项串联。
//!
//! 本模块无 unsafe、无 panic；越界输入一律返回 [`ChainError::Rlp`]。

use crate::chain::config::ChainError;

/// RLP 编码树节点。字节项持有自有的 `Vec<u8>`（方便 `Rlp::uint` 构造临时编码）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rlp {
    /// 单字节或字符串内容。
    Bytes(Vec<u8>),
    /// 嵌套列表。
    List(Vec<Rlp>),
}

impl Rlp {
    /// 把一个无符号整数编码为「最小大端、无前导零」字节串（0 → 空串），再包成 RLP 字节项。
    ///
    /// EVM 里 nonce/chainId/gas/value 等字段都用这种最小表示编码。
    pub fn uint(v: u128) -> Rlp {
        Rlp::Bytes(minimal_be_u128(v))
    }
}

/// 计算 `v` 的最小大端字节表示（无前导零；0 → 空 Vec）。
pub fn minimal_be_u128(v: u128) -> Vec<u8> {
    if v == 0 {
        Vec::new()
    } else {
        let bytes = v.to_be_bytes();
        let first_nonzero = bytes.iter().position(|b| *b != 0).unwrap_or(bytes.len());
        bytes[first_nonzero..].to_vec()
    }
}

/// 计算 `v` 的最小大端字节表示（u64 版本）。
pub fn minimal_be_u64(v: u64) -> Vec<u8> {
    minimal_be_u128(v as u128)
}

fn encode_bytes(content: &[u8], out: &mut Vec<u8>) {
    let len = content.len();
    if len == 1 && content[0] < 0x80 {
        // 单字节：自身即编码（不写长度前缀）。
        out.push(content[0]);
    } else if len <= 55 {
        out.push(0x80 + len as u8);
        out.extend_from_slice(content);
    } else {
        let len_be = minimal_be_u128(len as u128);
        out.push(0xb7 + len_be.len() as u8);
        out.extend_from_slice(&len_be);
        out.extend_from_slice(content);
    }
}

/// 把一棵 RLP 树编码成字节串。
pub fn encode(item: &Rlp) -> Vec<u8> {
    let mut out = Vec::new();
    encode_into(item, &mut out);
    out
}

fn encode_into(item: &Rlp, out: &mut Vec<u8>) {
    match item {
        Rlp::Bytes(b) => encode_bytes(b, out),
        Rlp::List(children) => {
            let mut payload = Vec::new();
            for c in children {
                encode_into(c, &mut payload);
            }
            let plen = payload.len();
            if plen <= 55 {
                out.push(0xc0 + plen as u8);
            } else {
                let len_be = minimal_be_u128(plen as u128);
                out.push(0xf7 + len_be.len() as u8);
                out.extend_from_slice(&len_be);
            }
            out.extend_from_slice(&payload);
        }
    }
}

// ── 解码 ────────────────────────────────────────────────────────────────────

/// 解码后的 RLP 树（拥有字节）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RlpOwned {
    Bytes(Vec<u8>),
    List(Vec<RlpOwned>),
}

impl RlpOwned {
    /// 若为列表，返回子项数；否则报错。
    pub fn as_list_len(&self) -> Result<usize, ChainError> {
        match self {
            RlpOwned::List(l) => Ok(l.len()),
            RlpOwned::Bytes(_) => Err(ChainError::Rlp("期望列表，得到字节串".into())),
        }
    }

    /// 若为字节串，返回引用。
    pub fn as_bytes(&self) -> Result<&[u8], ChainError> {
        match self {
            RlpOwned::Bytes(b) => Ok(b),
            RlpOwned::List(_) => Err(ChainError::Rlp("期望字节串，得到列表".into())),
        }
    }
}

/// 从 `buf` 的起始位置解码一个 RLP 项，返回 (值, 消费字节数)。
pub fn decode(buf: &[u8]) -> Result<(RlpOwned, usize), ChainError> {
    if buf.is_empty() {
        return Err(ChainError::Rlp("空 buffer 无法解码 RLP".into()));
    }
    let b0 = buf[0];
    if b0 < 0x80 {
        // 单字节
        Ok((RlpOwned::Bytes(vec![b0]), 1))
    } else if b0 <= 0xb7 {
        // 短字符串
        let len = (b0 - 0x80) as usize;
        let end = 1usize.saturating_add(len);
        if buf.len() < end {
            return Err(ChainError::Rlp(format!(
                "短字符串声明 {len} 字节，buffer 只剩 {} 字节",
                buf.len().saturating_sub(1)
            )));
        }
        Ok((RlpOwned::Bytes(buf[1..end].to_vec()), end))
    } else if b0 <= 0xbf {
        // 长字符串
        let len_of_len = (b0 - 0xb7) as usize;
        if len_of_len == 0 || len_of_len > 8 {
            return Err(ChainError::Rlp(format!(
                "非法长字符串长度前缀长度 {len_of_len}"
            )));
        }
        let head = 1usize.saturating_add(len_of_len);
        if buf.len() < head {
            return Err(ChainError::Rlp("长字符串长度声明超出 buffer".into()));
        }
        let mut len: u128 = 0;
        for &lb in &buf[1..head] {
            len = len.saturating_mul(256).saturating_add(lb as u128);
        }
        let len = usize::try_from(len).map_err(|_| ChainError::Rlp("长字符串长度过大".into()))?;
        let end = head.saturating_add(len);
        if buf.len() < end {
            return Err(ChainError::Rlp(format!(
                "长字符串声明 {len} 字节，buffer 不足"
            )));
        }
        Ok((RlpOwned::Bytes(buf[head..end].to_vec()), end))
    } else if b0 <= 0xf7 {
        // 短列表
        let plen = (b0 - 0xc0) as usize;
        let end = 1usize.saturating_add(plen);
        if buf.len() < end {
            return Err(ChainError::Rlp("短列表载荷超出 buffer".into()));
        }
        let children = decode_all(&buf[1..end])?;
        Ok((RlpOwned::List(children), end))
    } else {
        // 长列表
        let len_of_len = (b0 - 0xf7) as usize;
        if len_of_len == 0 || len_of_len > 8 {
            return Err(ChainError::Rlp(format!(
                "非法长列表长度前缀长度 {len_of_len}"
            )));
        }
        let head = 1usize.saturating_add(len_of_len);
        if buf.len() < head {
            return Err(ChainError::Rlp("长列表长度声明超出 buffer".into()));
        }
        let mut plen: u128 = 0;
        for &lb in &buf[1..head] {
            plen = plen.saturating_mul(256).saturating_add(lb as u128);
        }
        let plen = usize::try_from(plen).map_err(|_| ChainError::Rlp("长列表长度过大".into()))?;
        let end = head.saturating_add(plen);
        if buf.len() < end {
            return Err(ChainError::Rlp("长列表载荷超出 buffer".into()));
        }
        let children = decode_all(&buf[head..end])?;
        Ok((RlpOwned::List(children), end))
    }
}

/// 把一段连续 RLP 子项全部解出来（必须刚好消费完整段）。
fn decode_all(segment: &[u8]) -> Result<Vec<RlpOwned>, ChainError> {
    let mut out = Vec::new();
    let mut pos = 0usize;
    while pos < segment.len() {
        let (v, n) = decode(&segment[pos..])?;
        pos = pos.saturating_add(n);
        out.push(v);
    }
    Ok(out)
}

/// 把 RLP 字节串值还原为 u64（最小大端；空串 = 0）。
pub fn rlp_bytes_to_u64(b: &[u8]) -> Result<u64, ChainError> {
    if b.len() > 8 {
        return Err(ChainError::Rlp(format!(
            "u64 字段编码 {} 字节过长",
            b.len()
        )));
    }
    let mut acc: u64 = 0;
    for &byte in b {
        acc = acc.saturating_mul(256).saturating_add(byte as u64);
    }
    Ok(acc)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Vec<u8> {
        hex::decode(s).unwrap()
    }

    #[test]
    fn yellow_paper_byte_vectors() {
        // 空串 = 0x80
        assert_eq!(encode(&Rlp::Bytes(b"".to_vec())), hex("80"));
        // "dog" = 0x83 64 6f 67
        assert_eq!(encode(&Rlp::Bytes(b"dog".to_vec())), hex("83646f67"));
        // 空列表 = 0xc0
        assert_eq!(encode(&Rlp::List(vec![])), hex("c0"));
        // ["cat","dog"] = c8 83 636174 83 646f67
        let v = Rlp::List(vec![
            Rlp::Bytes(b"cat".to_vec()),
            Rlp::Bytes(b"dog".to_vec()),
        ]);
        assert_eq!(encode(&v), hex("c88363617483646f67"));
        // [[], [[]], [[], [[]]]] = c7 c0 c1 c0 c3 c0 c1 c0
        let nested = Rlp::List(vec![
            Rlp::List(vec![]),
            Rlp::List(vec![Rlp::List(vec![])]),
            Rlp::List(vec![Rlp::List(vec![]), Rlp::List(vec![Rlp::List(vec![])])]),
        ]);
        assert_eq!(encode(&nested), hex("c7c0c1c0c3c0c1c0"));
    }

    #[test]
    fn uint_vectors() {
        // 15 = 0x0f（单字节 <0x80 自身编码）
        assert_eq!(encode(&Rlp::uint(15)), hex("0f"));
        // 1024 = 0x82 0400
        assert_eq!(encode(&Rlp::uint(1024)), hex("820400"));
        // 0 = 0x80
        assert_eq!(encode(&Rlp::uint(0)), hex("80"));
        // 127 = 0x7f
        assert_eq!(encode(&Rlp::uint(127)), hex("7f"));
        // 128 = 0x81 80（跨越单字节边界）
        assert_eq!(encode(&Rlp::uint(128)), hex("8180"));
    }

    #[test]
    fn long_string_vector() {
        // 56 字节的 'a'：长度 56 > 55 → 0xb8 0x38 + 56 个 0x61。
        let content = vec![0x61u8; 56];
        let enc = encode(&Rlp::Bytes(content));
        assert_eq!(&enc[..2], &[0xb8, 0x38]);
        assert_eq!(enc.len(), 2 + 56);
    }

    #[test]
    fn decode_roundtrip() {
        let v = Rlp::List(vec![
            Rlp::uint(1u64 as u128),
            Rlp::Bytes(b"hello".to_vec()),
            Rlp::List(vec![Rlp::Bytes(b"inner".to_vec())]),
        ]);
        let enc = encode(&v);
        let (dec, consumed) = decode(&enc).unwrap();
        assert_eq!(consumed, enc.len());
        let list = match dec {
            RlpOwned::List(l) => l,
            _ => panic!("expected list"),
        };
        assert_eq!(list.len(), 3);
        assert_eq!(rlp_bytes_to_u64(list[0].as_bytes().unwrap()).unwrap(), 1);
        assert_eq!(list[1].as_bytes().unwrap(), b"hello");
        assert_eq!(list[2].as_list_len().unwrap(), 1);
    }

    #[test]
    fn decode_rejects_truncated_and_garbage() {
        assert!(decode(&[]).is_err());
        // 声称 5 字节字符串但只给 1 字节
        assert!(decode(&[0x85, 0x01, 0x02]).is_err());
        // 非法长度前缀长度（0xb8 但声明 8 字节长度位后无内容）
        assert!(decode(&[0xb9, 0x01]).is_err());
    }

    #[test]
    fn minimal_be_no_leading_zeros() {
        assert_eq!(minimal_be_u64(0), Vec::<u8>::new());
        assert_eq!(minimal_be_u64(1), vec![1]);
        assert_eq!(minimal_be_u64(0x1024), vec![0x10, 0x24]);
        assert_eq!(minimal_be_u64(0xff), vec![0xff]);
    }
}
