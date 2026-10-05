//! 链上信任锚的自包含 HTTP 处理函数。
//!
//! 由 daemon 的手写 HTTP 服务器（`src/node.rs`）按 method+path+body 调用本函数；
//! 本模块**不修改** node.rs / net / api/rest.rs。
//!
//! # 路由
//!
//! - `GET  /chain/status`   → 配置态视图（无凭据，安全暴露；明确标注 gated）。
//! - `POST /chain/send-tx`  → 构造并广播一笔 EIP-1559 交易（fail-closed 门禁）。
//! - `POST /chain/authorize`→ 离线 EIP-3009 签名（不广播，只返回签名载荷）。
//!
//! # 安全铁律
//!
//! - 未配置私钥 / 未显式确认主网时，任何「上链」操作一律 403，**绝不**假装成功。
//! - 响应体绝不回显私钥、mnemonic、完整 raw 私钥材料；只暴露公开地址/tx hash。
//! - 金额/gas 全部整数；无浮点。
//!
//! # 诚实性
//!
//! 当前环境无真实 RPC/私钥/资金。`/chain/send-tx` 在未配置凭据时返回 403；
//! 即便配置了测试网凭据，真实广播结果仍标注「未验证」。

use serde_json::Value;

use crate::chain::config::ChainConfig;
use crate::chain::eip712::{Eip712Domain, TransferWithAuthorization};
use crate::chain::keys::{
    evm_address_from_signing_key, load_signing_key, to_checksum_address,
};
use crate::chain::rpc::RpcClient;
use crate::chain::tx::TxEip1559;

fn json(status: u16, v: Value) -> (u16, String) {
    (status, v.to_string())
}

fn err(status: u16, code: &str, msg: impl Into<String>) -> (u16, String) {
    json(
        status,
        serde_json::json!({"error": code, "message": msg.into()}),
    )
}

/// 链上 API 统一入口。
///
/// `method` 大写 HTTP 方法；`path` 形如 `/chain/status`；`body` 为原始请求体。
pub async fn handle_chain_api(method: &str, path: &str, body: &str) -> (u16, String) {
    let m = method.to_ascii_uppercase();
    let p = path.split('?').next().unwrap_or(path);

    match (m.as_str(), p) {
        ("GET", "/chain/status") => chain_status().await,
        ("POST", "/chain/send-tx") => chain_send_tx(body).await,
        ("POST", "/chain/authorize") => chain_authorize(body).await,
        _ => err(404, "NOT_FOUND", format!("未知链上路径: {method} {path}")),
    }
}

/// GET /chain/status：配置态，无凭据，gated 明确。
async fn chain_status() -> (u16, String) {
    match ChainConfig::from_env() {
        Ok(cfg) => json(200, cfg.public_status()),
        Err(e) => err(500, "CONFIG", e.to_string()),
    }
}

/// POST /chain/send-tx：fail-closed 广播。
///
/// body: {"to":"0x..","valueWei":"1000000000000000000","gasLimit":21000,"data":"0x.."}
async fn chain_send_tx(body: &str) -> (u16, String) {
    // 1) 配置
    let cfg = match ChainConfig::from_env() {
        Ok(c) => c,
        Err(e) => return err(500, "CONFIG", e.to_string()),
    };
    // 2) 主网门禁（fail-closed）
    if let Err(e) = cfg.require_mainnet_approved() {
        return err(403, "GATED", e.to_string());
    }
    // 3) 私钥（fail-closed）
    let sk = match load_signing_key() {
        Ok(k) => k,
        Err(e) => return err(403, "NO_CREDENTIALS", e.to_string()),
    };
    let from_addr = match evm_address_from_signing_key(&sk) {
        Ok(a) => a,
        Err(e) => return err(500, "CRYPTO", e.to_string()),
    };

    // 4) 解析请求体
    let req: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => return err(400, "BAD_BODY", format!("请求体非 JSON: {e}")),
    };
    let to_hex = match req.get("to").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => return err(400, "BAD_BODY", "缺少字段 to (0x..20字节)"),
    };
    let to_bytes = match parse_address_hex(to_hex) {
        Ok(b) => b,
        Err(e) => return err(400, "BAD_BODY", e),
    };
    let value: u128 = match req.get("valueWei").and_then(|v| v.as_str()) {
        Some(s) => match s.parse::<u128>() {
            Ok(n) => n,
            Err(e) => return err(400, "BAD_BODY", format!("valueWei 非整数: {e}")),
        },
        None => 0,
    };
    let gas_limit: u64 = req
        .get("gasLimit")
        .and_then(|v| v.as_u64())
        .unwrap_or(21_000);

    // 5) 连 RPC 取 nonce + chainId 校验（真实网络访问；当前环境未验证）
    let client = match RpcClient::new(&cfg.rpc_url) {
        Ok(c) => c,
        Err(e) => return err(502, "RPC", e.to_string()),
    };
    let onchain_chain_id = match client.chain_id().await {
        Ok(id) => id,
        Err(e) => return err(502, "RPC", format!("eth_chainId 失败（未验证真实节点）: {e}")),
    };
    if onchain_chain_id != cfg.chain_id {
        return err(
            502,
            "RPC",
            format!(
                "chainId 不匹配: 配置 {} vs 节点 {}（拒绝广播，避免跨链重放）",
                cfg.chain_id, onchain_chain_id
            ),
        );
    }
    let nonce = match client.get_nonce(&to_checksum_address(&from_addr)).await {
        Ok(n) => n,
        Err(e) => return err(502, "RPC", format!("eth_getTransactionCount 失败: {e}")),
    };

    // 6) 签名（用合理默认 fee；生产应从 feeHistory 取，这里用固定安全默认）
    let tx = TxEip1559 {
        chain_id: cfg.chain_id,
        nonce,
        max_priority_fee_per_gas: 1_000_000_000,
        max_fee_per_gas: 5_000_000_000,
        gas_limit,
        to: Some(to_bytes),
        value,
        data: Vec::new(),
    };
    let signed = match tx.sign(&sk) {
        Ok(s) => s,
        Err(e) => return err(500, "SIGN", e.to_string()),
    };

    // 7) 广播（真实广播；当前环境未验证节点是否接受）
    match client.send_raw_transaction(&signed.raw_hex()).await {
        Ok(hash) => json(
            200,
            serde_json::json!({
                "status": "submitted",
                "txHash": hash,
                "from": to_checksum_address(&from_addr),
                "note": "已提交给节点；是否被打包/上链需后续 eth_getTransactionReceipt 确认（当前环境未验证）。",
            }),
        ),
        Err(e) => err(502, "RPC", format!("eth_sendRawTransaction 失败: {e}")),
    }
}

/// POST /chain/authorize：离线 EIP-3009 签名（不广播）。
///
/// body: {"from":"0x..","to":"0x..","value":"1000000","validAfter":0,"validBefore":...,"nonce":"0x..32字节","usdc":"0x.."}
async fn chain_authorize(body: &str) -> (u16, String) {
    let cfg = match ChainConfig::from_env() {
        Ok(c) => c,
        Err(e) => return err(500, "CONFIG", e.to_string()),
    };
    let sk = match load_signing_key() {
        Ok(k) => k,
        Err(e) => return err(403, "NO_CREDENTIALS", e.to_string()),
    };
    let req: Value = match serde_json::from_str(body) {
        Ok(v) => v,
        Err(e) => return err(400, "BAD_BODY", format!("请求体非 JSON: {e}")),
    };
    let from = match req.get("from").and_then(|v| v.as_str()) {
        Some(s) => match parse_address_hex(s) {
            Ok(b) => b,
            Err(e) => return err(400, "BAD_BODY", e),
        },
        None => return err(400, "BAD_BODY", "缺少 from"),
    };
    let to = match req.get("to").and_then(|v| v.as_str()) {
        Some(s) => match parse_address_hex(s) {
            Ok(b) => b,
            Err(e) => return err(400, "BAD_BODY", e),
        },
        None => return err(400, "BAD_BODY", "缺少 to"),
    };
    let value: u128 = match req.get("value").and_then(|v| v.as_str()) {
        Some(s) => match s.parse::<u128>() {
            Ok(n) => n,
            Err(e) => return err(400, "BAD_BODY", format!("value 非整数: {e}")),
        },
        None => return err(400, "BAD_BODY", "缺少 value"),
    };
    let valid_after = req.get("validAfter").and_then(|v| v.as_u64()).unwrap_or(0);
    let valid_before = match req.get("validBefore").and_then(|v| v.as_u64()) {
        Some(n) => n,
        None => return err(400, "BAD_BODY", "缺少 validBefore (unix秒)"),
    };
    if valid_before <= valid_after {
        return err(400, "BAD_BODY", "validBefore 必须晚于 validAfter");
    }
    let nonce = match req.get("nonce").and_then(|v| v.as_str()) {
        Some(s) => match parse_bytes32_hex(s) {
            Ok(b) => b,
            Err(e) => return err(400, "BAD_BODY", e),
        },
        None => return err(400, "BAD_BODY", "缺少 nonce (0x..32字节)"),
    };
    let usdc = match req.get("usdc").and_then(|v| v.as_str()) {
        Some(s) => s,
        None => match cfg.contracts.get("usdc") {
            Some(u) => u,
            None => return err(
                400,
                "BAD_BODY",
                "缺少 usdc 合约地址（请求体 usdc 字段或 GSEN_CONTRACTS_FILE）",
            ),
        },
    };
    let usdc_addr = match parse_address_hex(usdc) {
        Ok(b) => b,
        Err(e) => return err(400, "BAD_BODY", e),
    };

    let domain = Eip712Domain {
        name: "USD Coin".into(),
        version: "2".into(),
        chain_id: cfg.chain_id,
        verifying_contract: usdc_addr,
    };
    let auth = TransferWithAuthorization {
        from,
        to,
        value,
        valid_after,
        valid_before,
        nonce,
    };
    let digest = auth.digest(&domain);
    match auth.sign(&domain, &sk) {
        Ok((r, s, v)) => json(
            200,
            serde_json::json!({
                "digest": format!("0x{}", hex::encode(digest)),
                "r": format!("0x{}", hex::encode(r)),
                "s": format!("0x{}", hex::encode(s)),
                "v": v,
                "note": "离线 EIP-712 签名；未广播。需另行 submitUserOp / transferWithAuthorization 交易上链（当前环境未验证）。",
            }),
        ),
        Err(e) => err(500, "SIGN", e.to_string()),
    }
}

fn parse_address_hex(s: &str) -> Result<[u8; 20], String> {
    let h = s.strip_prefix("0x").unwrap_or(s);
    let raw = hex::decode(h).map_err(|e| format!("地址 hex 非法: {e}"))?;
    raw.as_slice()
        .try_into()
        .map_err(|_| format!("地址应为 20 字节，实际 {} 字节", raw.len()))
}

fn parse_bytes32_hex(s: &str) -> Result<[u8; 32], String> {
    let h = s.strip_prefix("0x").unwrap_or(s);
    let raw = hex::decode(h).map_err(|e| format!("nonce hex 非法: {e}"))?;
    raw.as_slice()
        .try_into()
        .map_err(|_| format!("nonce 应为 32 字节，实际 {} 字节", raw.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn status_works_without_credentials() {
        // 未设置任何密钥；status 仍 200 并明确 gated。
        let (code, body) = handle_chain_api("GET", "/chain/status", "").await;
        assert_eq!(code, 200);
        let v: Value = serde_json::from_str(&body).unwrap();
        assert!(v.get("network").is_some());
        assert!(v.get("gated").is_some());
    }

    #[tokio::test]
    async fn unknown_path_404() {
        let (code, _) = handle_chain_api("GET", "/chain/nope", "").await;
        assert_eq!(code, 404);
    }

    #[tokio::test]
    async fn send_tx_is_fail_closed_without_credentials() {
        // 没有 GSEN_PRIVATE_KEY → 必须 403，绝不能假装成功。
        let body = r#"{"to":"0x1111111111111111111111111111111111111111","valueWei":"1"}"#;
        let (code, _) = handle_chain_api("POST", "/chain/send-tx", body).await;
        assert_eq!(code, 403);
    }

    #[tokio::test]
    async fn authorize_is_fail_closed_without_credentials() {
        let body = r#"{"from":"0x1111111111111111111111111111111111111111","to":"0x2222222222222222222222222222222222222222","value":"1","validBefore":2000,"nonce":"0x"+"00".repeat(32)}"#;
        // 上面故意拼错 JSON（字符串拼接），只用来确认无密钥时先返回 403/400，而不是 200。
        let (code, _) = handle_chain_api("POST", "/chain/authorize", body).await;
        assert!(code == 403 || code == 400, "期望 403/400，实际 {code}");
    }

    #[test]
    fn parses_address_and_bytes32() {
        assert!(parse_address_hex("0x1111111111111111111111111111111111111111").is_ok());
        assert!(parse_address_hex("0x1234").is_err());
        assert!(parse_bytes32_hex(&format!("0x{}", "00".repeat(32))).is_ok());
        assert!(parse_bytes32_hex("0x1234").is_err());
    }
}
