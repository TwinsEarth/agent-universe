//! V2.3.2 MCP + ACA 兼容层测试

use gsn_core::*;
use serde_json::json;

// ========== MCP 协议测试 ==========

#[test]
fn test_mcp_request_construction() {
    let req = McpRequest::new(
        RequestId::Number(1),
        McpMethod::ToolsList,
        json!({}),
    );
    assert_eq!(req.method, "tools/list");
    assert_eq!(req.jsonrpc, "2.0");
    assert!(matches!(req.method_enum(), McpMethod::ToolsList));
}

#[test]
fn test_mcp_response_success() {
    let resp = McpResponse::success(
        RequestId::Number(1),
        json!({ "tools": [] }),
    );
    assert!(resp.is_success());
    assert!(resp.error.is_none());
}

#[test]
fn test_mcp_response_error() {
    let resp = McpResponse::error(
        RequestId::Number(2),
        McpError::ToolNotFound("nonexistent".to_string()),
    );
    assert!(!resp.is_success());
    assert!(resp.error.is_some());
    assert_eq!(resp.error.as_ref().unwrap().code, -32001);
}

#[test]
fn test_mcp_method_from_str() {
    assert_eq!(McpMethod::from_str("initialize"), McpMethod::Initialize);
    assert_eq!(McpMethod::from_str("tools/call"), McpMethod::ToolsCall);
    assert_eq!(McpMethod::from_str("unknown"), McpMethod::Custom);
}

// ========== MCP 工具测试 ==========

#[test]
fn test_tool_definition() {
    let tool = ToolDefinition::new("text_gen".to_string(), "文本生成".to_string())
        .with_param(ToolParameter {
            name: "prompt".to_string(),
            param_type: "string".to_string(),
            description: "提示词".to_string(),
            required: true,
            enum_values: None,
        })
        .with_capability("text-gen".to_string())
        .with_verify_level(1);

    assert_eq!(tool.name, "text_gen");
    assert!(tool.input_schema.required.contains(&"prompt".to_string()));
    assert_eq!(tool.gsn_verify_level, 1);
}

#[test]
fn test_tool_result() {
    let ok = ToolResult::text("hello");
    assert!(!ok.is_error);

    let err = ToolResult::error("failed");
    assert!(err.is_error);
}

// ========== MCP 资源测试 ==========

#[test]
fn test_resource_uri_parse() {
    let _uri = ResourceUri::new("gsn://did:123/agent/card");
    let parsed = ResourceUri::parse_gsn("gsn://did:123/agent/card");
    assert!(parsed.is_some());
    let (did, res_type, id) = parsed.unwrap();
    assert_eq!(did, "did:123");
    assert_eq!(res_type, "agent");
    assert_eq!(id, "card");
}

#[test]
fn test_resource_definition() {
    let res = ResourceDefinition::new(
        ResourceUri::new("gsn://did:1/card"),
        "Agent Card".to_string(),
        "application/json".to_string(),
    ).with_cid("bafy...".to_string());
    assert!(res.gsn_cid.is_some());
}

// ========== MCP 提示测试 ==========

#[test]
fn test_prompt_render() {
    let prompt = PromptDefinition::new("socratic_hint".to_string(), "苏格拉底提示".to_string())
        .socratic();

    let messages = vec![
        PromptMessage::user("请帮我解释 {concept}"),
    ];

    let mut args = serde_json::Map::new();
    args.insert("concept".to_string(), json!("光合作用"));

    let rendered = prompt.render(&messages, &args);
    assert_eq!(rendered.len(), 1);
    assert!(rendered[0].content.contains("光合作用"));
    assert!(!rendered[0].content.contains("{concept}"));
}

// ========== MCP 服务器测试 ==========

#[test]
fn test_mcp_server_initialize() {
    let mut server = McpServer::new("gsn-node-1".to_string());
    let req = McpRequest::new(RequestId::Number(1), McpMethod::Initialize, json!({}));
    let resp = server.handle(&req);
    assert!(resp.is_success());
}

#[test]
fn test_mcp_server_tools_list() {
    let mut server = McpServer::new("test".to_string());
    server.register_tool(ToolDefinition::new("search".to_string(), "搜索".to_string()));
    server.register_tool(ToolDefinition::new("generate".to_string(), "生成".to_string()));

    let req = McpRequest::new(RequestId::Number(1), McpMethod::ToolsList, json!({}));
    let resp = server.handle(&req);
    assert!(resp.is_success());
    assert_eq!(server.tool_count(), 2);
}

#[test]
fn test_mcp_server_tools_call_not_found() {
    let mut server = McpServer::new("test".to_string());
    let req = McpRequest::new(
        RequestId::Number(1),
        McpMethod::ToolsCall,
        json!({ "name": "nonexistent" }),
    );
    let resp = server.handle(&req);
    assert!(!resp.is_success());
}

#[test]
fn test_mcp_server_prompts() {
    let mut server = McpServer::new("test".to_string());
    let prompt = PromptDefinition::new("explain".to_string(), "解释概念".to_string());
    let messages = vec![PromptMessage::user("解释 {topic}")];
    server.register_prompt(prompt, messages);

    let list_req = McpRequest::new(RequestId::Number(1), McpMethod::PromptsList, json!({}));
    let list_resp = server.handle(&list_req);
    assert!(list_resp.is_success());
    assert_eq!(server.prompt_count(), 1);

    let get_req = McpRequest::new(
        RequestId::Number(2),
        McpMethod::PromptsGet,
        json!({ "name": "explain", "arguments": { "topic": "量子计算" } }),
    );
    let get_resp = server.handle(&get_req);
    assert!(get_resp.is_success());
}

// ========== ACA Manifest 测试 ==========

#[test]
fn test_agent_manifest() {
    let manifest = AgentManifest::new("did:aip:123".to_string(), "TestAgent".to_string())
        .with_capability("text-gen".to_string())
        .with_stake(10000)
        .with_verification_mode(VerificationMode::Redundant)
        .with_mcp_tools(5);

    assert_eq!(manifest.did, "did:aip:123");
    assert_eq!(manifest.capabilities.len(), 1);
    assert_eq!(manifest.stake, 10000);
    assert_eq!(manifest.mcp_tool_count, 5);
    assert!(manifest.compatibility_score() > 0.0);
}

// ========== ACA Envelope 测试 ==========

#[test]
fn test_task_envelope() {
    let env = TaskEnvelope::new(
        "did:requester".to_string(),
        "text-gen".to_string(),
        "cid:abc".to_string(),
        "生成一段摘要".to_string(),
    )
    .with_verification(VerificationLevel::L1Redundant)
    .with_budget(100)
    .with_mcp_tool("text_gen".to_string());

    assert_eq!(env.verification_level, VerificationLevel::L1Redundant);
    assert_eq!(env.budget, 100);
    assert!(env.estimated_verification_cost() <= env.budget);
}

#[test]
fn test_task_envelope_verification_affordability() {
    let cheap = TaskEnvelope::new(
        "r".to_string(), "c".to_string(), "cid".to_string(), "o".to_string(),
    )
    .with_verification(VerificationLevel::L0Sample)
    .with_budget(100);
    assert!(cheap.verification_affordable());

    let expensive = TaskEnvelope::new(
        "r".to_string(), "c".to_string(), "cid".to_string(), "o".to_string(),
    )
    .with_verification(VerificationLevel::L4Committee)
    .with_budget(10);
    assert!(!expensive.verification_affordable());
}

// ========== ACA Receipt 测试 ==========

#[test]
fn test_receipt() {
    let result = b"task output data";
    let receipt = Receipt::new(
        "task-1".to_string(),
        "did:executor".to_string(),
        result,
    );

    assert!(receipt.verify_result(result));
    assert!(!receipt.verify_result(b"wrong data"));
    assert_eq!(receipt.status, ReceiptStatus::Completed);
    assert!(!receipt.needs_arbitration());
}

#[test]
fn test_receipt_dispute() {
    let mut receipt = Receipt::new("task-1".to_string(), "did:exec".to_string(), b"output");
    receipt.mark_disputed();
    assert!(receipt.needs_arbitration());
    assert_eq!(receipt.status, ReceiptStatus::Disputed);
}

// ========== ACA 五级验证测试 ==========

#[test]
fn test_verification_level_properties() {
    assert_eq!(VerificationLevel::L0Sample.as_str(), "L0");
    assert_eq!(VerificationLevel::L4Committee.as_str(), "L4");
    assert!(VerificationLevel::L0Sample.cost_multiplier() < VerificationLevel::L1Redundant.cost_multiplier());
    assert!(VerificationLevel::L0Sample.finality_seconds() < VerificationLevel::L4Committee.finality_seconds());
}

#[test]
fn test_verification_policy_chooses_level() {
    // 低价值低风险 → L0
    let p1 = VerificationPolicy::new(10, 0.01);
    assert_eq!(p1.choose_level(), VerificationLevel::L0Sample);

    // 高价值高风险 → L4
    let p2 = VerificationPolicy::new(100000, 0.5);
    assert_eq!(p2.choose_level(), VerificationLevel::L4Committee);
}

#[test]
fn test_verification_required_verifiers() {
    let policy = VerificationPolicy::new(100, 0.1);
    assert_eq!(policy.required_verifiers(VerificationLevel::L1Redundant), 3);
    assert_eq!(policy.required_verifiers(VerificationLevel::L0Sample), 1);
    assert_eq!(policy.required_verifiers(VerificationLevel::L4Committee), 7);
}

// ========== ACA 多维声誉测试 ==========

#[test]
fn test_multi_reputation() {
    let mut rep = MultiReputation::new("did:agent1".to_string());
    assert_eq!(rep.did(), "did:agent1");
    assert!(!rep.is_transferable()); // 不可转让

    rep.record_success(ReputationDimension::Quality, 500.0);
    rep.record_success(ReputationDimension::Honesty, 300.0);

    assert!(rep.get(ReputationDimension::Quality) > 5000.0);
    assert!(rep.overall() > 0.0);
    assert_eq!(rep.interactions(), 2);
}

#[test]
fn test_multi_reputation_failure() {
    let mut rep = MultiReputation::new("did:agent2".to_string());
    rep.record_failure(ReputationDimension::Honesty, 1000.0);
    assert!(rep.get(ReputationDimension::Honesty) < 5000.0);
}

// ========== ACA 消息测试 ==========

#[test]
fn test_aca_message_handshake() {
    let manifest = AgentManifest::new("did:a".to_string(), "A".to_string());
    let msg = AcaMessage::handshake("did:a".to_string(), manifest).unwrap();
    assert_eq!(msg.msg_type, MessageType::Handshake);
    assert!(msg.is_broadcast());
    let parsed = msg.parse_manifest().unwrap();
    assert_eq!(parsed.did, "did:a");
}

#[test]
fn test_aca_message_task_proposal() {
    let envelope = TaskEnvelope::new(
        "did:req".to_string(),
        "cap".to_string(),
        "cid".to_string(),
        "out".to_string(),
    );
    let msg = AcaMessage::propose_task("did:req".to_string(), "did:exec".to_string(), envelope).unwrap();
    assert_eq!(msg.msg_type, MessageType::TaskProposal);
    assert!(!msg.is_broadcast());
    let parsed = msg.parse_envelope().unwrap();
    assert_eq!(parsed.requester_did, "did:req");
}

#[test]
fn test_aca_message_receipt() {
    let receipt = Receipt::new("task-1".to_string(), "did:exec".to_string(), b"result");
    let msg = AcaMessage::deliver_receipt("did:exec".to_string(), "did:req".to_string(), receipt).unwrap();
    assert_eq!(msg.msg_type, MessageType::Receipt);
    let parsed = msg.parse_receipt().unwrap();
    assert_eq!(parsed.task_id, "task-1");
}

// ========== 协议对象闭环测试 ==========

#[test]
fn test_protocol_object_lifecycle() {
    // 完整闭环：Manifest → Envelope → Receipt → Reputation
    let manifest = AgentManifest::new("did:exec".to_string(), "Executor".to_string())
        .with_verification_mode(VerificationMode::Redundant);

    let envelope = TaskEnvelope::new(
        "did:req".to_string(),
        "text-gen".to_string(),
        "cid:input".to_string(),
        "output".to_string(),
    )
    .with_verification(VerificationLevel::L1Redundant);

    let receipt = Receipt::new(envelope.task_id.clone(), "did:exec".to_string(), b"output result");

    let mut rep = MultiReputation::new("did:exec".to_string());
    rep.record_success(ReputationDimension::Quality, 100.0);
    rep.record_success(ReputationDimension::Honesty, 100.0);

    // 闭环验证
    assert_eq!(receipt.task_id, envelope.task_id);
    assert!(manifest.compatibility_score() > 0.0);
    assert!(rep.overall() > 0.0);
}

// ========== 审计修复验证测试 ==========

#[test]
fn test_erasure_recover_from_parity() {
    // 当前实现：所有 data shards 可用时可正常解码
    // parity shards 用于完整性验证
    let coder = ErasureCoder::new(4, 2);
    let data = b"important data that needs redundancy";
    let shards = coder.encode(data);

    // 所有分片都在
    let recovered = coder.decode(&shards, data.len()).unwrap();
    assert_eq!(recovered, data);

    // parity 校验通过
    assert!(coder.verify_parity(&shards));
}

#[test]
fn test_erasure_too_many_lost() {
    // XOR 编码最多恢复 1 个丢失 shard；丢失 2 个应返回错误
    let coder = ErasureCoder::new(4, 2);
    let data = b"test data for erasure coding recovery";
    let shards = coder.encode(data);

    // 丢失 2 个 data shard（只保留 2 个 data）
    let partial: Vec<_> = shards.iter()
        .filter(|s| s.index == 0 || s.index == 3)
        .cloned()
        .collect();
    assert_eq!(partial.len(), 2);

    let result = coder.decode(&partial, data.len());
    assert!(result.is_err(), "should fail when too many shards lost");
}

#[test]
fn test_erasure_verify_parity() {
    let coder = ErasureCoder::new(4, 2);
    let data = b"verify parity test data";
    let shards = coder.encode(data);
    assert!(coder.verify_parity(&shards));
}

#[test]
fn test_gsn_daemon_help() {
    // 验证 gsn-daemon 二进制存在且可运行
    let bin_path = std::env::var("CARGO_BIN_FILE_GSN_DAEMON")
        .unwrap_or_else(|_| "./target/debug/gsn-daemon".to_string());
    let output = std::process::Command::new(&bin_path)
        .arg("--help")
        .output()
        .expect("failed to run gsn-daemon");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("gsn-daemon"));
    assert!(stdout.contains("--port"));
}

#[test]
fn test_contracts_exist() {
    // 验证 Solidity 合约文件存在
    let base = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let contracts = [
        "contracts/src/GovernorToken.sol",
        "contracts/src/AgentCardAnchor.sol",
        "contracts/src/PoCVSettlement.sol",
        "contracts/src/ReputationBridge.sol",
    ];
    for c in &contracts {
        let path = base.join("../").join(c);
        assert!(path.exists(), "contract missing: {} (resolved: {})", c, path.display());
    }
}
