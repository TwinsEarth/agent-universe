//! v2.6.1 集成测试：
//!  - MCP 参数校验（GAP §8.1）：缺必填 / 类型错误 → -32602，由调用方转换
//!  - 账本从只追加流水恢复（GAP §6.1）：restored 引擎余额 / 守恒 / 审计连续

use gsn_core::mcp::tool::validate_arguments;
use gsn_core::mcp::ToolSchema;
use gsn_core::marketplace::{Money, SettlementEngine, SettlementReason};
use serde_json::{json, Map, Value};

fn sample_schema() -> ToolSchema {
    let mut properties = Map::new();
    properties.insert("account".to_string(), json!({ "type": "string" }));
    properties.insert("amount".to_string(), json!({ "type": "integer" }));
    properties.insert("memo".to_string(), json!({ "type": "string" }));
    ToolSchema {
        schema_type: "object".to_string(),
        properties,
        required: vec!["account".to_string(), "amount".to_string()],
    }
}

#[test]
fn validate_accepts_full_and_optional_absent() {
    let schema = sample_schema();
    assert!(validate_arguments(&schema, &json!({ "account": "a1", "amount": 100 })).is_ok());
    assert!(
        validate_arguments(&schema, &json!({ "account": "a1", "amount": 100, "memo": "hi" }))
            .is_ok()
    );
}

#[test]
fn validate_rejects_missing_or_null_required() {
    let schema = sample_schema();
    let err = validate_arguments(&schema, &json!({ "account": "a1" })).unwrap_err();
    assert!(err.contains("amount"), "{err}");
    let err =
        validate_arguments(&schema, &json!({ "account": "a1", "amount": null })).unwrap_err();
    assert!(err.contains("amount"), "{err}");
    let err = validate_arguments(&schema, &json!({ "amount": 1 })).unwrap_err();
    assert!(err.contains("account"), "{err}");
}

#[test]
fn validate_rejects_wrong_types() {
    let schema = sample_schema();
    // integer 传字符串
    let err =
        validate_arguments(&schema, &json!({ "account": "a1", "amount": "100" })).unwrap_err();
    assert!(err.contains("amount"), "{err}");
    // integer 传非整浮点
    let err =
        validate_arguments(&schema, &json!({ "account": "a1", "amount": 1.5 })).unwrap_err();
    assert!(err.contains("amount"), "{err}");
    // string 传数字
    let err = validate_arguments(&schema, &json!({ "account": 7, "amount": 1 })).unwrap_err();
    assert!(err.contains("account"), "{err}");
}

#[test]
fn validate_rejects_non_object_arguments() {
    let schema = sample_schema();
    let err = validate_arguments(&schema, &Value::Array(vec![])).unwrap_err();
    assert!(err.contains("对象"), "{err}");
}

#[test]
fn restored_engine_preserves_balances_conservation_and_audit() {
    let mut e = SettlementEngine::new();
    e.deposit("requester", Money::new(1000)).unwrap();
    e.deposit("agent", Money::new(500)).unwrap();
    // 质押锁定
    e.lock(
        "stake:agent",
        "agent",
        "__stake__:agent",
        Money::new(100),
        SettlementReason::Staked,
    )
    .unwrap();
    // 任务托管锁定
    e.lock(
        "t1",
        "requester",
        "__escrow__:t1",
        Money::new(200),
        SettlementReason::Escrowed,
    )
    .unwrap();
    // 结算（托管 → agent，中标价 150，余 50 留托管待退）
    e.settle(
        "t1",
        "__escrow__:t1",
        "agent",
        Money::new(150),
        SettlementReason::Completed,
    )
    .unwrap();
    // 罚没部分质押
    e.slash("__stake__:agent", Money::new(50)).unwrap();

    let before = e.conservation_check();
    let audit_before = e.independent_audit();
    assert!(before.conserved);
    assert!(audit_before.passed);

    // 从流水恢复
    let records = e.records().to_vec();
    let r = SettlementEngine::restore(records).unwrap();
    let after = r.conservation_check();
    let audit_after = r.independent_audit();

    // 守恒连续
    assert!(after.conserved);
    assert_eq!(before.balance_sum, after.balance_sum);
    assert_eq!(before.total_deposits, after.total_deposits);
    assert_eq!(before.total_paid, after.total_paid);
    assert_eq!(before.total_slashed, after.total_slashed);

    // 逐账户余额连续
    // requester 800 / agent 550 / __stake__:agent 50 / __escrow__:t1 50
    assert_eq!(r.balance("requester"), Money::new(800));
    assert_eq!(r.balance("agent"), Money::new(550));
    assert_eq!(r.balance("__stake__:agent"), Money::new(50));
    assert_eq!(r.balance("__escrow__:t1"), Money::new(50));
    for acct in ["requester", "agent", "__stake__:agent", "__escrow__:t1"] {
        assert_eq!(e.balance(acct), r.balance(acct), "账户 {acct} 恢复不一致");
    }

    // 恢复后独立审计仍通过
    assert!(audit_after.passed, "恢复后独立审计应通过");
}
