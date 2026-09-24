"""MCP（Model Context Protocol）HTTP 客户端。

通过 gsn-daemon 的 ``POST /api/v1/mcp`` JSON-RPC 端点调用市场工具，
完整走 MCP 生命周期：initialize → notifications/initialized →
ping / tools/list / tools/call。

tools/call 由 daemon 端的 MarketMcpBridge 真实路由到市场 Actor 执行，
不是模拟返回。

仅使用标准库 urllib。
"""

from __future__ import annotations

import json
import urllib.error
import urllib.request
from typing import Any, Dict, List, Optional

MCP_PROTOCOL_VERSION = "2024-11-05"
_SDK_VERSION = "2.3.6"


class McpError(RuntimeError):
    """JSON-RPC 错误。"""

    def __init__(self, code: int, message: str):
        super().__init__(f"[JSON-RPC {code}] {message}")
        self.code = code
        self.message = message


class McpHttpClient:
    """MCP over HTTP 客户端。"""

    def __init__(
        self,
        base_url: str = "http://127.0.0.1:4002",
        path: str = "/api/v1/mcp",
        timeout: float = 10.0,
    ):
        self.endpoint = base_url.rstrip("/") + path
        self.timeout = timeout
        self._next = 0
        self._initialized = False

    def _id(self) -> int:
        self._next += 1
        return self._next

    def _post(self, payload: Dict[str, Any], await_response: bool = True) -> Any:
        data = json.dumps(payload).encode("utf-8")
        req = urllib.request.Request(
            self.endpoint,
            data=data,
            headers={
                "Content-Type": "application/json",
                "Accept": "application/json",
            },
            method="POST",
        )
        try:
            with urllib.request.urlopen(req, timeout=self.timeout) as resp:
                raw = resp.read().decode("utf-8")
                status = resp.status
        except urllib.error.HTTPError as exc:
            raw = exc.read().decode("utf-8")
            status = exc.code

        # 通知（无 id）返回 202 且通常无 body
        if not await_response or not raw:
            return None

        response = json.loads(raw)
        if "error" in response and response["error"] is not None:
            err = response["error"]
            raise McpError(err.get("code", -32603), err.get("message", "unknown"))
        return response.get("result")

    def initialize(self) -> Dict[str, Any]:
        """执行 MCP 初始化握手。"""
        result = self._post(
            {
                "jsonrpc": "2.0",
                "id": self._id(),
                "method": "initialize",
                "params": {
                    "protocolVersion": MCP_PROTOCOL_VERSION,
                    "clientInfo": {"name": "aip-sdk", "version": _SDK_VERSION},
                    "capabilities": {},
                },
            }
        )
        # 初始化完成通知（无 id，不期待响应）
        self._post(
            {"jsonrpc": "2.0", "method": "notifications/initialized"},
            await_response=False,
        )
        self._initialized = True
        return result

    def ping(self) -> bool:
        """心跳，成功返回 True。"""
        self._post({"jsonrpc": "2.0", "id": self._id(), "method": "ping"})
        return True

    def list_tools(self) -> List[Dict[str, Any]]:
        """列出服务端工具。未初始化时自动初始化。"""
        if not self._initialized:
            self.initialize()
        result = self._post(
            {"jsonrpc": "2.0", "id": self._id(), "method": "tools/list"}
        )
        return result.get("tools", [])

    def call_tool(self, name: str, arguments: Optional[Dict[str, Any]] = None) -> Any:
        """调用工具，返回 ToolResult（含 content / isError）。"""
        if not self._initialized:
            self.initialize()
        result = self._post(
            {
                "jsonrpc": "2.0",
                "id": self._id(),
                "method": "tools/call",
                "params": {"name": name, "arguments": arguments or {}},
            }
        )
        return result
