//! 市场 MCP 工具桥接
//!
//! v2.3.5: 把 AgentMarket 的全部能力注册为标准 MCP 工具，
//! 让大模型（Claude / Cursor / 豆包 等）能安全、规范地完成
//! 注册 → 发现 → 匹配 → 执行 → 验证 → 结算 → 信誉 全流程。
//!
//! 桥接持有 MarketActorHandle，tools/call 时路由到对应 actor 方法。

use crate::api::market_actor::{MarketActorHandle, MarketResponse};
use crate::mcp::tool::*;
use serde_json::{json, Value};

/// 市场 MCP 桥接
pub struct MarketMcpBridge {
    market: MarketActorHandle,
}

impl MarketMcpBridge {
    pub fn new(market: MarketActorHandle) -> Self {
        Self { market }
    }

    /// 全部市场工具定义
    pub fn tool_definitions() -> Vec<ToolDefinition> {
        vec![
            tool("market_register_agent", "注册一个智能体到市场（需质押）")
                .param("agent_id", "string", "全局唯一 DID", true)
                .param("name", "string", "智能体名称", true)
                .param("skills", "array", "技能标签数组，如 [\"translation\"]", false)
                .param("stake", "number", "质押金额（须 ≥ 最低质押 100）", false)
                .param("price", "number", "单次调用价格", false)
                .param("description", "string", "能力描述", false),
            tool("market_get_agent", "按 agent_id 查询智能体详情")
                .param("agent_id", "string", "智能体 DID", true),
            tool("market_discover_agents", "按技能标签发现智能体")
                .param("skill", "string", "技能标签，如 translation", true),
            tool("market_search_agents", "按关键词搜索智能体")
                .param("query", "string", "搜索关键词", true),
            tool("market_publish_task", "发布一个任务到市场")
                .param("task", "object", "完整 TaskSpec JSON", true),
            tool("market_get_task", "按 task_id 查询任务")
                .param("task_id", "string", "任务 ID", true),
            tool("market_submit_bid", "对任务提交投标")
                .param("bid", "object", "投标 JSON（agent_id/task_id/proposed_price）", true),
            tool("market_match_task", "为任务匹配最优智能体")
                .param("task_id", "string", "任务 ID", true),
            tool("market_submit_result", "提交任务执行结果")
                .param("envelope", "object", "ResultEnvelope JSON", true),
            tool("market_verify_result", "QA 委员会验证结果（BFT-lite）")
                .param("task_id", "string", "任务 ID", true)
                .param("approvals", "number", "投通过票委员数", false)
                .param("committee_size", "number", "委员会总人数", false),
            tool("market_settle_task", "结算已验收任务")
                .param("task_id", "string", "任务 ID", true),
            tool("market_open_dispute", "对任务发起争议")
                .param("dispute", "object", "争议 JSON（task_id/complainant/reason）", true),
            tool("market_arbitrate", "仲裁争议，可罚没质押")
                .param("dispute_id", "string", "争议 ID", true)
                .param("guilty", "boolean", "是否裁定有罪", true)
                .param("slash_amount", "number", "罚没金额", false),
            tool("market_deposit", "向账户充值")
                .param("account", "string", "账户 DID", true)
                .param("amount", "number", "充值金额", true),
            tool("market_balance", "查询账户余额")
                .param("account", "string", "账户 DID", true),
            tool("market_conservation", "检查结算守恒不变量（无参数）"),
            tool("market_leaderboard", "查询信誉排行榜")
                .param("limit", "number", "返回条数", false),
            tool("market_stats", "查询市场概览统计（无参数）"),
        ]
    }

    /// 执行工具调用，返回 MCP ToolResult
    pub async fn call(&self, name: &str, args: &Value) -> ToolResult {
        let get = |key: &str| -> Value { args.get(key).cloned().unwrap_or(Value::Null) };
        let get_str = |key: &str| -> String {
            args.get(key).and_then(|v| v.as_str()).unwrap_or("").to_string()
        };
        let get_f64 = |key: &str| -> f64 { args.get(key).and_then(|v| v.as_f64()).unwrap_or(0.0) };

        let result: MarketResponse = match name {
            "market_register_agent" => self.market.register_agent(args.clone()).await,
            "market_get_agent" => self.market.get_agent(get_str("agent_id")).await,
            "market_discover_agents" => self.market.discover(get_str("skill")).await,
            "market_search_agents" => self.market.search(get_str("query")).await,
            "market_publish_task" => self.market.publish_task(get("task")).await,
            "market_get_task" => self.market.get_task(get_str("task_id")).await,
            "market_submit_bid" => self.market.submit_bid(get("bid")).await,
            "market_match_task" => self.market.match_task(get_str("task_id")).await,
            "market_submit_result" => self.market.submit_result(get("envelope")).await,
            "market_verify_result" => {
                let approvals = args.get("approvals").and_then(|v| v.as_u64()).unwrap_or(3) as u32;
                let size = args.get("committee_size").and_then(|v| v.as_u64()).unwrap_or(4) as u32;
                self.market.verify_result(get_str("task_id"), approvals, size).await
            }
            "market_settle_task" => self.market.settle_task(get_str("task_id")).await,
            "market_open_dispute" => self.market.open_dispute(get("dispute")).await,
            "market_arbitrate" => {
                let guilty = args.get("guilty").and_then(|v| v.as_bool()).unwrap_or(false);
                let slash = get_f64("slash_amount");
                self.market.arbitrate(get_str("dispute_id"), guilty, slash).await
            }
            "market_deposit" => self.market.deposit(get_str("account"), get_f64("amount")).await,
            "market_balance" => self.market.balance(get_str("account")).await,
            "market_conservation" => self.market.conservation().await,
            "market_leaderboard" => {
                let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(10) as usize;
                self.market.leaderboard(limit).await
            }
            "market_stats" => self.market.stats().await,
            _ => return ToolResult::error(format!("未知工具: {name}")),
        };

        match result {
            MarketResponse::Ok(v) => ToolResult {
                content: vec![ToolContent::Text {
                    text: serde_json::to_string_pretty(&v).unwrap_or_default(),
                }],
                is_error: false,
            },
            MarketResponse::Err(e) => ToolResult::error(e),
        }
    }
}

/// 构造工具定义的辅助
fn tool(name: &str, description: &str) -> ToolDefinition {
    ToolDefinition::new(name.to_string(), description.to_string())
}

/// 参数构造辅助（trait 扩展，链式）
trait ParamBuilder {
    fn param(self, name: &str, ty: &str, desc: &str, required: bool) -> Self;
}

impl ParamBuilder for ToolDefinition {
    fn param(mut self, name: &str, ty: &str, desc: &str, required: bool) -> Self {
        // object/array 类型在 schema 里补充对应结构
        let schema = if ty == "object" {
            json!({"type": "object", "description": desc})
        } else if ty == "array" {
            json!({"type": "array", "description": desc})
        } else {
            json!({"type": ty, "description": desc})
        };
        self.input_schema.properties.insert(name.to_string(), schema);
        if required {
            self.input_schema.required.push(name.to_string());
        }
        self
    }
}
