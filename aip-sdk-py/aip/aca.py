"""ACA（Agent Communication Architecture）对象构造、签名与验签。

提供与 Rust gsn-core 对齐的协议对象：

- ``AgentManifest``：智能体磁力链（能力 / 硬件 / 验证模式 / 质押）
- ``TaskEnvelope``：任务信封（输入 CID / 验证级别 / 隐私 / 预算）
- ``AcaMessage``：跨节点消息信封（携带上述对象并整体签名）
- ``Receipt``：执行收据（结果哈希 / 计量 / 证明，签名）

对象的 P2P 传输由 gsn-daemon 的 libp2p 层承担；本模块负责在 SDK 端
可信地构造、Ed25519 签名与验签。签名需要可选依赖
``cryptography``（``pip install aip-sdk[crypto]``）。
"""

from __future__ import annotations

import hashlib
import json
import time
import uuid
from typing import Any, Dict, List, Optional

from .crypto import AipIdentity

# ── 枚举标签（与 Rust serde 默认变体名一致） ──
MODE_SELF_REPORTED = "SelfReported"
MODE_REDUNDANT = "Redundant"
MODE_TEE = "TEE"
MODE_ZKML = "ZkML"
MODE_COMMITTEE = "CommitteeArbitration"

LEVEL_L0 = "L0Sample"
LEVEL_L1 = "L1Redundant"
LEVEL_L2 = "L2TEE"
LEVEL_L3 = "L3ZkML"
LEVEL_L4 = "L4Committee"

PRIVACY_PUBLIC = "Public"
PRIVACY_LOCAL = "LocalOnly"
PRIVACY_DP = "DifferentialPrivacy"
PRIVACY_TEE = "TEE"
PRIVACY_ZK = "ZK"

PRIORITY_NORMAL = "Normal"

STATUS_COMPLETED = "Completed"
STATUS_VERIFIED = "Verified"
STATUS_FAILED = "Failed"
STATUS_DISPUTED = "Disputed"
STATUS_ARBITRATED = "Arbitrated"

MSG_HANDSHAKE = "Handshake"
MSG_TASK_PROPOSAL = "TaskProposal"
MSG_TASK_ACCEPT = "TaskAccept"
MSG_TASK_REJECT = "TaskReject"
MSG_RECEIPT = "Receipt"


def _now() -> int:
    return int(time.time())


def build_manifest(
    identity: AipIdentity,
    name: str,
    capabilities: List[str],
    *,
    version: str = "2.3.6",
    endpoints: Optional[List[str]] = None,
    cpu_cores: int = 0,
    memory_mb: int = 0,
    disk_free_gb: int = 0,
    platform: str = "desktop",
    verification_modes: Optional[List[str]] = None,
    stake: int = 0,
    mcp_tool_count: int = 0,
) -> Dict[str, Any]:
    """构造并签名 AgentManifest。"""
    manifest: Dict[str, Any] = {
        "did": identity.did,
        "name": name,
        "version": version,
        "capabilities": list(capabilities),
        "endpoints": endpoints or [],
        "hardware": {
            "cpu_cores": cpu_cores,
            "memory_mb": memory_mb,
            "disk_free_gb": disk_free_gb,
            "gpu": None,
            "bandwidth_up_mbps": 0,
            "bandwidth_down_mbps": 0,
            "platform": platform,
        },
        "verification_modes": verification_modes or [MODE_SELF_REPORTED],
        "stake": stake,
        "reputation_ref": None,
        "model_hash": None,
        "mcp_tool_count": mcp_tool_count,
        "signature": "",
        "timestamp": _now(),
    }
    return identity.sign_into(manifest)


def build_envelope(
    requester_did: str,
    capability: str,
    output_spec: str,
    *,
    input_cid: str = "",
    verification_level: str = LEVEL_L0,
    privacy: str = PRIVACY_PUBLIC,
    budget: int = 0,
    timeout_secs: int = 300,
    priority: str = PRIORITY_NORMAL,
    mcp_tool: Optional[str] = None,
) -> Dict[str, Any]:
    """构造 TaskEnvelope（无独立 signature；由携带它的 AcaMessage 整体签名）。"""
    return {
        "task_id": str(uuid.uuid4()),
        "requester_did": requester_did,
        "capability": capability,
        "input_cid": input_cid,
        "output_spec": output_spec,
        "verification_level": verification_level,
        "privacy": privacy,
        "budget": budget,
        "timeout_secs": timeout_secs,
        "priority": priority,
        "mcp_tool": mcp_tool,
        "created_at": _now(),
    }


def build_message(
    identity: AipIdentity,
    msg_type: str,
    to_did: str,
    payload: Any,
) -> Dict[str, Any]:
    """构造并签名 AcaMessage。payload 为 manifest/envelope/receipt 的 dict。"""
    message: Dict[str, Any] = {
        "message_id": str(uuid.uuid4()),
        "msg_type": msg_type,
        "from_did": identity.did,
        "to_did": to_did,
        "payload": payload,
        "timestamp": _now(),
        "signature": "",
    }
    return identity.sign_into(message)


def handshake_message(
    identity: AipIdentity, manifest: Dict[str, Any]
) -> Dict[str, Any]:
    """广播握手消息（to="*"），携带已签名 manifest。"""
    return build_message(identity, MSG_HANDSHAKE, "*", manifest)


def proposal_message(
    identity: AipIdentity, to_did: str, envelope: Dict[str, Any]
) -> Dict[str, Any]:
    """任务提议消息，携带 envelope。"""
    return build_message(identity, MSG_TASK_PROPOSAL, to_did, envelope)


def build_receipt(
    identity: AipIdentity,
    task_id: str,
    result: bytes,
    *,
    metering: Optional[Dict[str, Any]] = None,
    tee_quote: Optional[str] = None,
    zk_proof: Optional[str] = None,
    status: str = STATUS_COMPLETED,
) -> Dict[str, Any]:
    """构造并签名执行 Receipt。"""
    digest = hashlib.sha256(result).digest()
    result_hash = list(digest)
    default_metering = {
        "compute_ms": 0,
        "memory_peak_mb": 0,
        "bandwidth_mb": 0.0,
        "storage_bytes": 0,
        "energy_joules": 0.0,
    }
    default_metering.update(metering or {})
    receipt: Dict[str, Any] = {
        "task_id": task_id,
        "executor_did": identity.did,
        "result_cid": "cid:" + digest.hex()[:16],
        "result_hash": result_hash,
        "status": status,
        "metering": default_metering,
        "tee_quote": tee_quote,
        "zk_proof": zk_proof,
        "completed_at": _now(),
        "signature": "",
    }
    return identity.sign_into(receipt)


def receipt_message(
    identity: AipIdentity, to_did: str, receipt: Dict[str, Any]
) -> Dict[str, Any]:
    """交付收据消息，携带已签名 receipt。"""
    return build_message(identity, MSG_RECEIPT, to_did, receipt)


def verify_manifest(manifest: Dict[str, Any], public_key: bytes) -> bool:
    """验 manifest 签名。"""
    return AipIdentity.verify_object(manifest, public_key)


def verify_message(message: Dict[str, Any], public_key: bytes) -> bool:
    """验消息签名。"""
    return AipIdentity.verify_object(message, public_key)


def verify_receipt(receipt: Dict[str, Any], public_key: bytes) -> bool:
    """验收据签名。"""
    return AipIdentity.verify_object(receipt, public_key)


def verify_receipt_result(receipt: Dict[str, Any], result: bytes) -> bool:
    """校验收据内 result_hash 与实际结果是否一致。"""
    expected = list(hashlib.sha256(result).digest())
    return receipt.get("result_hash") == expected
