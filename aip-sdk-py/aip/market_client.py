"""智能体市场 REST 客户端。

通过 gsn-daemon 的 HTTP API（默认 http://127.0.0.1:4002）完成完整市场闭环：
充值 → 注册（质押）→ 发布任务 → 投标 → 匹配 → 提交结果 → BFT 验证 →
结算 → 守恒检查 → 排行榜。

仅使用标准库 urllib，不强制引入第三方依赖。
"""

from __future__ import annotations

import json
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
from typing import Any, Dict, List, Optional


class MarketError(RuntimeError):
    """市场 HTTP/业务错误。"""

    def __init__(self, status: int, message: Any):
        super().__init__(f"[HTTP {status}] {message}")
        self.status = status
        self.message = message


def build_task_spec(
    requester: str,
    goal: str,
    todo: List[str],
    budget: float,
    required_skills: List[str],
    *,
    context: str = "",
    owner: Optional[str] = None,
    deadline_ms: Optional[int] = None,
    bft_n: int = 4,
    bft_f: int = 1,
) -> Dict[str, Any]:
    """构造符合六字段校验的 TaskSpec 请求体。

    默认验证策略 BftLite(n=4, f=1)，与市场默认 QA 委员会一致。
    """
    now_ms = int(time.time() * 1000)
    return {
        "task_id": str(uuid.uuid4()),
        "goal": goal,
        "context": context or goal,
        "done": [],
        "todo": list(todo),
        "trace": [],
        "owner": owner or requester,
        "budget": budget,
        "deadline": deadline_ms or (now_ms + 300_000),
        "required_skills": list(required_skills),
        "verification_policy": {"BftLite": {"n": bft_n, "f": bft_f}},
        "requester": requester,
        "state": "Open",
        "created_at": int(time.time()),
    }


class MarketClient:
    """gsn-daemon 市场 REST 客户端。"""

    def __init__(self, base_url: str = "http://127.0.0.1:4002", timeout: float = 10.0):
        self.base_url = base_url.rstrip("/")
        self.timeout = timeout

    # ───────── 底层 HTTP ─────────
    def _request(
        self,
        method: str,
        path: str,
        body: Optional[Any] = None,
        query: Optional[Dict[str, Any]] = None,
    ) -> Any:
        url = self.base_url + path
        if query:
            url += "?" + urllib.parse.urlencode(query)

        data = None
        headers = {"Accept": "application/json"}
        if body is not None:
            data = json.dumps(body).encode("utf-8")
            headers["Content-Type"] = "application/json"

        req = urllib.request.Request(url, data=data, headers=headers, method=method)
        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as resp:
                raw = resp.read().decode("utf-8")
                status = resp.status
        except urllib.error.HTTPError as exc:
            raw = exc.read().decode("utf-8")
            status = exc.code

        parsed: Any = None
        if raw:
            try:
                parsed = json.loads(raw)
            except ValueError:
                parsed = raw

        if status >= 400:
            msg = parsed.get("error") if isinstance(parsed, dict) else parsed
            raise MarketError(status, msg or f"HTTP {status}")
        return parsed

    # ───────── 节点 ─────────
    def health(self) -> Dict[str, Any]:
        return self._request("GET", "/health")

    # ───────── 账户 ─────────
    def deposit(self, account: str, amount: float) -> Dict[str, Any]:
        return self._request(
            "POST", f"/api/v1/accounts/{urllib.parse.quote(account)}/deposit",
            {"amount": amount},
        )

    def balance(self, account: str) -> Dict[str, Any]:
        return self._request(
            "GET", f"/api/v1/accounts/{urllib.parse.quote(account)}/balance"
        )

    # ───────── 智能体 ─────────
    def register_agent(
        self,
        agent_id: str,
        name: str,
        *,
        stake: float = 100.0,
        price: float = 1.0,
        description: str = "",
        skills: Optional[List[str]] = None,
        modalities: Optional[List[str]] = None,
        endpoint: str = "",
        owner: str = "",
        currency: str = "credit",
        pricing_model: str = "per_call",
    ) -> Dict[str, Any]:
        body = {
            "agent_id": agent_id,
            "name": name,
            "description": description,
            "skills": skills or [],
            "modalities": modalities or ["text"],
            "models": [],
            "endpoint": endpoint,
            "owner": owner or agent_id,
            "stake": stake,
            "price": price,
            "currency": currency,
            "pricing_model": pricing_model,
        }
        return self._request("POST", "/api/v1/agents", body)

    def get_agent(self, agent_id: str) -> Dict[str, Any]:
        return self._request(
            "GET", f"/api/v1/agents/{urllib.parse.quote(agent_id)}"
        )

    def discover(self, skill: str) -> Dict[str, Any]:
        return self._request("GET", "/api/v1/agents", query={"skill": skill})

    def search(self, query: str) -> Dict[str, Any]:
        return self._request("GET", "/api/v1/agents", query={"q": query})

    # ───────── 任务 ─────────
    def publish_task(self, task_spec: Dict[str, Any]) -> Dict[str, Any]:
        return self._request("POST", "/api/v1/tasks", task_spec)

    def get_task(self, task_id: str) -> Dict[str, Any]:
        return self._request(
            "GET", f"/api/v1/tasks/{urllib.parse.quote(task_id)}"
        )

    def list_tasks(self) -> Dict[str, Any]:
        return self._request("GET", "/api/v1/tasks")

    def submit_bid(
        self,
        task_id: str,
        agent_id: str,
        proposed_price: float,
        estimated_latency_ms: int = 1000,
    ) -> Dict[str, Any]:
        body = {
            "agent_id": agent_id,
            "proposed_price": proposed_price,
            "estimated_latency_ms": estimated_latency_ms,
            "score": 0.0,
        }
        return self._request(
            "POST", f"/api/v1/tasks/{urllib.parse.quote(task_id)}/bids", body
        )

    def match_task(self, task_id: str) -> Dict[str, Any]:
        return self._request(
            "POST", f"/api/v1/tasks/{urllib.parse.quote(task_id)}/match"
        )

    def submit_result(
        self,
        task_id: str,
        agent_id: str,
        report: Any,
        *,
        confidence: float = 0.9,
        latency_ms: int = 100,
        trace_ref: str = "",
        evidence_grade: str = "Verified",
    ) -> Dict[str, Any]:
        body = {
            "agent_id": agent_id,
            "report": report if isinstance(report, str) else json.dumps(report),
            "confidence": confidence,
            "error_type": "None",
            "trace_ref": trace_ref,
            "evidence_grade": evidence_grade,
            "latency_ms": latency_ms,
        }
        return self._request(
            "POST", f"/api/v1/tasks/{urllib.parse.quote(task_id)}/results", body
        )

    def verify_result(
        self, task_id: str, approvals: int = 3, committee_size: int = 4
    ) -> Dict[str, Any]:
        return self._request(
            "POST",
            f"/api/v1/tasks/{urllib.parse.quote(task_id)}/verify",
            query={"approvals": approvals, "committee_size": committee_size},
        )

    def settle_task(self, task_id: str) -> Dict[str, Any]:
        return self._request(
            "POST", f"/api/v1/tasks/{urllib.parse.quote(task_id)}/settle"
        )

    # ───────── 争议与仲裁 ─────────
    def open_dispute(
        self,
        task_id: str,
        complainant: str,
        reason: str,
        *,
        respondent: str = "",
        dispute_id: Optional[str] = None,
    ) -> Dict[str, Any]:
        body = {
            "dispute_id": dispute_id or f"dispute-{uuid.uuid4()}",
            "task_id": task_id,
            "complainant": complainant,
            "respondent": respondent,
            "reason": reason,
        }
        return self._request("POST", "/api/v1/disputes", body)

    def arbitrate(
        self, dispute_id: str, guilty: bool, slash_amount: float = 0.0
    ) -> Dict[str, Any]:
        body = {"guilty": guilty, "slash_amount": slash_amount}
        return self._request(
            "POST",
            f"/api/v1/disputes/{urllib.parse.quote(dispute_id)}/arbitrate",
            body,
        )

    # ───────── 生态 ─────────
    def conservation(self) -> Dict[str, Any]:
        return self._request("GET", "/api/v1/conservation")

    def leaderboard(self, limit: int = 10) -> Dict[str, Any]:
        return self._request(
            "GET", "/api/v1/leaderboard", query={"limit": limit}
        )

    def stats(self) -> Dict[str, Any]:
        return self._request("GET", "/api/v1/stats")
