//! 市场 MCP 工具桥接
//!
//! v2.3.5: 把 AgentMarket 的全部能力注册为标准 MCP 工具，
//! 让大模型（Claude / Cursor / 豆包 等）能安全、规范地完成
//! 注册 → 发现 → 匹配 → 执行 → 验证 → 结算 → 信誉 全流程。
//!
//! 桥接持有 MarketActorHandle，tools/call 时路由到对应 actor 方法。

use crate::api::market_actor::{MarketActorHandle, MarketResponse};
use crate::mcp::tool::*;
use serde_json::Value;

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
                .param(
                    "skills",
                    "array",
                    "技能标签数组，如 [\"translation\"]",
                    false,
                )
                .param(
                    "stake",
                    "integer",
                    "质押金额（整数，须 ≥ 最低质押 100，注册前先 deposit）",
                    false,
                )
                .param("price", "integer", "单次调用价格（整数）", false)
                .param("description", "string", "能力描述", false),
            tool("market_get_agent", "按 agent_id 查询智能体详情").param(
                "agent_id",
                "string",
                "智能体 DID",
                true,
            ),
            tool("market_discover_agents", "按技能标签发现智能体").param(
                "skill",
                "string",
                "技能标签，如 translation",
                true,
            ),
            tool("market_search_agents", "按关键词搜索智能体").param(
                "query",
                "string",
                "搜索关键词",
                true,
            ),
            tool("market_publish_task", "发布一个任务到市场").param(
                "task",
                "object",
                "完整 TaskSpec JSON",
                true,
            ),
            tool("market_get_task", "按 task_id 查询任务").param(
                "task_id",
                "string",
                "任务 ID",
                true,
            ),
            tool("market_submit_bid", "对任务提交投标").param(
                "bid",
                "object",
                "投标 JSON（agent_id/task_id/proposed_price）",
                true,
            ),
            tool("market_match_task", "为任务匹配最优智能体").param(
                "task_id",
                "string",
                "任务 ID",
                true,
            ),
            tool("market_submit_result", "提交任务执行结果").param(
                "envelope",
                "object",
                "ResultEnvelope JSON",
                true,
            ),
            tool(
                "market_verify_result",
                "认证式 QA 委员会验证结果（v2.5.9，BFT-lite）",
            )
            .param("task_id", "string", "任务 ID", true)
            .param(
                "members",
                "array",
                "固定委员集 [{\"did\":..,\"public_key\":hex32}]",
                true,
            )
            .param(
                "signed_votes",
                "array",
                "委员私钥签发的投票数组（vote 传 Stop/Continue）",
                true,
            )
            .param(
                "round",
                "integer",
                "视图轮次（默认 0；必须为 u32 范围内整数）",
                false,
            ),
            tool("market_settle_task", "结算已验收任务").param(
                "task_id",
                "string",
                "任务 ID",
                true,
            ),
            tool("market_reject_task", "终局拒绝验证中/返工任务（v2.8.5）").param(
                "task_id",
                "string",
                "任务 ID",
                true,
            ),
            tool("market_open_dispute", "对任务发起争议").param(
                "dispute",
                "object",
                "争议 JSON（task_id/complainant/reason）",
                true,
            ),
            tool(
                "market_arbitrate",
                "仲裁争议，罚没金额由服务端规则决定（v2.8.5）",
            )
            .param("dispute_id", "string", "争议 ID", true)
            .param("arbitrator", "string", "仲裁者身份（必填，拒绝匿名）", true)
            .param("guilty", "boolean", "是否裁定有罪", true),
            tool("market_deposit", "向账户充值")
                .param("account", "string", "账户 DID", true)
                .param("amount", "integer", "充值金额（整数）", true),
            tool("market_balance", "查询账户余额").param("account", "string", "账户 DID", true),
            tool("market_conservation", "检查结算守恒不变量（无参数）"),
            tool(
                "market_audit",
                "独立审计：从只追加流水独立重放，检测守恒无法发现的账实不符（v2.5.9，无参数）",
            ),
            tool("market_leaderboard", "查询信誉排行榜").param(
                "limit",
                "number",
                "返回条数",
                false,
            ),
            tool("market_stats", "查询市场概览统计（无参数）"),
        ]
    }

    /// 执行工具调用，返回 MCP ToolResult
    pub async fn call(&self, name: &str, args: &Value) -> ToolResult {
        let get = |key: &str| -> Value { args.get(key).cloned().unwrap_or(Value::Null) };
        let get_str = |key: &str| -> String {
            args.get(key)
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string()
        };
        let get_money = |key: &str| -> Result<crate::marketplace::Money, ToolResult> {
            // v2.8.6（GAP §4.1）：金额入口只接受整数（i64），拒绝 JSON 浮点
            // （10.5 截断、10.0 亦为 f64）。非法值由 validate_arguments 先返 -32602，
            // 此处纵深防御，绝不做浮点→整数截断。
            // v3.5.3（AU-36）：取不到合法整数时旧实现 `unwrap_or(Money::ZERO)` 会把金额
            // 静默写成 0（往账户存 0，脚枪）。改为显式工具错误，不得静默当 0。
            match args.get(key).and_then(|v| v.as_i64()) {
                Some(n) => Ok(crate::marketplace::Money::new(n)),
                None => Err(ToolResult::error(format!(
                    "INVALID_PARAM(-32602): {key} 必须为整数金额，实际: {}",
                    args.get(key).cloned().unwrap_or(serde_json::Value::Null)
                ))),
            }
        };

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
                // v3.5.3（AU-35）：round 必须是 u32 范围内的合法整数。旧实现
                // `as_u64().unwrap_or(0) as u32`：1.5 经 number 校验后静默退 0、5e9 经
                // `as u32` 静默截断。现在非整数/负数/越界一律返 -32602 风格参数错误。
                let round: u32 = match args.get("round") {
                    None | Some(serde_json::Value::Null) => 0,
                    Some(v) => match v.as_i64() {
                        Some(n) => match u32::try_from(n) {
                            Ok(r) => r,
                            Err(_) => {
                                return ToolResult::error(format!(
                                    "INVALID_PARAM(-32602): round 超出 u32 范围，实际: {n}"
                                ));
                            }
                        },
                        None => {
                            return ToolResult::error(format!(
                                "INVALID_PARAM(-32602): round 必须为整数，实际: {v}"
                            ));
                        }
                    },
                };
                let members = match crate::api::rest::parse_committee_members(args.get("members")) {
                    Ok(m) => m,
                    Err(e) => return ToolResult::error(e),
                };
                let votes = match crate::api::rest::parse_signed_votes(args.get("signed_votes")) {
                    Ok(v) => v,
                    Err(e) => return ToolResult::error(e),
                };
                self.market
                    .verify_result(
                        get_str("task_id"),
                        round,
                        members,
                        votes,
                        crate::api::rest::unix_now(),
                    )
                    .await
            }
            "market_settle_task" => self.market.settle_task(get_str("task_id")).await,
            "market_reject_task" => self.market.reject_task(get_str("task_id")).await,
            "market_open_dispute" => self.market.open_dispute(get("dispute")).await,
            "market_arbitrate" => {
                let guilty = args
                    .get("guilty")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                // v2.8.5：仲裁者身份必填，罚没金额服务端规则决定。
                let arbitrator = args
                    .get("arbitrator")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                if arbitrator.trim().is_empty() {
                    MarketResponse::err("缺少仲裁者身份（arbitrator）")
                } else {
                    self.market
                        .arbitrate(get_str("dispute_id"), arbitrator, guilty)
                        .await
                }
            }
            "market_deposit" => {
                let amount = match get_money("amount") {
                    Ok(m) => m,
                    Err(e) => return e,
                };
                self.market.deposit(get_str("account"), amount).await
            }
            "market_balance" => self.market.balance(get_str("account")).await,
            "market_conservation" => self.market.conservation().await,
            "market_audit" => self.market.audit().await,
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

// 构造工具定义与参数链式构造的 tool() / ParamBuilder 已上移到
// mcp::tool（公共），通过顶部 `use crate::mcp::tool::*` 引入。
