"""Agent Universe AIP SDK v2.3.6"""

from .models import AgentCard, Task, TaskStatus, SkillSpec, Pricing
from .dht_backend import MemoryDHT
from .index.sharded import ShardedIndex, ShardMetadata, shard_of

# 身份与签名（cryptography 为可选后端，未安装时仅签名/验签不可用）
from .crypto import AipIdentity, canonical_payload, did_from_public_key

# 市场 REST 客户端
from .market_client import MarketClient, MarketError, build_task_spec

# MCP HTTP 客户端
from .mcp_client import McpHttpClient, McpError, MCP_PROTOCOL_VERSION

# ACA 对象构造、签名与验签
from .aca import (
    build_manifest,
    build_envelope,
    build_message,
    build_receipt,
    handshake_message,
    proposal_message,
    receipt_message,
    verify_manifest,
    verify_message,
    verify_receipt,
    verify_receipt_result,
)

__version__ = "2.3.6"
__all__ = [
    # 数据模型
    "AgentCard", "Task", "TaskStatus", "SkillSpec", "Pricing",
    "MemoryDHT", "ShardedIndex", "ShardMetadata", "shard_of",
    # 身份
    "AipIdentity", "canonical_payload", "did_from_public_key",
    # 市场
    "MarketClient", "MarketError", "build_task_spec",
    # MCP
    "McpHttpClient", "McpError", "MCP_PROTOCOL_VERSION",
    # ACA
    "build_manifest", "build_envelope", "build_message", "build_receipt",
    "handshake_message", "proposal_message", "receipt_message",
    "verify_manifest", "verify_message", "verify_receipt",
    "verify_receipt_result",
]
