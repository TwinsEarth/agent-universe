"""AIP/ACA 身份与 Ed25519 签名。

签名能力依赖可选后端 cryptography（安装方式：``pip install aip-sdk[crypto]``）。
未安装时，签名与验签会抛出带安装指引的 RuntimeError，其余不涉及签名的
SDK 功能（数据模型、内存 DHT、分片索引）仍可正常使用。

跨语言一致性
    待签名载荷（canonical payload）统一为：移除 ``signature`` 键后，
    以紧凑分隔符、按 key 字典序、不转义非 ASCII 字符的 JSON 字节。
    Rust gsn-core 的 serde_json 默认基于 BTreeMap（按 key 排序），
    与本模块的 ``sort_keys=True`` 产出一致，因此 Python 与 Rust
    可以跨语言互验签名。
"""

from __future__ import annotations

import hashlib
import json
from typing import Any, Dict

try:
    from cryptography.exceptions import InvalidSignature
    from cryptography.hazmat.primitives.asymmetric.ed25519 import (
        Ed25519PrivateKey,
        Ed25519PublicKey,
    )

    _HAVE_CRYPTO = True
except ImportError:  # pragma: no cover - 取决于运行环境
    _HAVE_CRYPTO = False
    InvalidSignature = Exception  # type: ignore


def _require_crypto() -> None:
    if not _HAVE_CRYPTO:
        raise RuntimeError(
            "Ed25519 签名需要可选依赖 cryptography：pip install aip-sdk[crypto]"
        )


def did_from_public_key(public_key: bytes) -> str:
    """由公钥生成 DID：did:aip:{sha256(pubkey)前8字节hex}。"""
    digest = hashlib.sha256(public_key).digest()
    return "did:aip:" + digest[:8].hex()


def canonical_payload(obj: Dict[str, Any]) -> bytes:
    """构造规范待签名载荷：移除 signature 键，紧凑、字典序、原样 UTF-8。"""
    data = dict(obj)
    data.pop("signature", None)
    return json.dumps(
        data,
        separators=(",", ":"),
        sort_keys=True,
        ensure_ascii=False,
    ).encode("utf-8")


class AipIdentity:
    """节点身份：Ed25519 密钥对 + 由公钥派生的 DID。"""

    def __init__(self, private_key: "Ed25519PrivateKey"):
        _require_crypto()
        self._private_key = private_key
        self._public_key_obj = private_key.public_key()
        self._public_key = self._public_key_obj.public_bytes_raw()
        self.did = did_from_public_key(self._public_key)

    @classmethod
    def generate(cls) -> "AipIdentity":
        """生成新身份。"""
        _require_crypto()
        return cls(Ed25519PrivateKey.generate())

    @classmethod
    def from_seed(cls, seed: bytes) -> "AipIdentity":
        """由 32 字节种子恢复身份。"""
        _require_crypto()
        if len(seed) != 32:
            raise ValueError("seed 必须为 32 字节")
        return cls(Ed25519PrivateKey.from_private_bytes(seed))

    @property
    def public_key(self) -> bytes:
        """32 字节公钥。"""
        return self._public_key

    def sign_payload(self, payload: bytes) -> bytes:
        """对任意字节签名，返回 64 字节签名。"""
        return self._private_key.sign(payload)

    def sign_object(self, obj: Dict[str, Any]) -> str:
        """对 JSON 对象签名，返回 hex 签名（不修改原对象）。"""
        return self.sign_payload(canonical_payload(obj)).hex()

    def sign_into(self, obj: Dict[str, Any]) -> Dict[str, Any]:
        """把 hex 签名写入对象的 signature 字段并返回该对象。"""
        obj["signature"] = self.sign_object(obj)
        return obj

    @staticmethod
    def verify_object(obj: Dict[str, Any], public_key: bytes) -> bool:
        """用公钥验证对象签名；签名缺失、格式错误或不匹配均返回 False。"""
        _require_crypto()
        signature_hex = obj.get("signature", "")
        if not signature_hex:
            return False
        try:
            signature = bytes.fromhex(signature_hex)
        except ValueError:
            return False
        try:
            Ed25519PublicKey.from_public_bytes(public_key).verify(
                signature, canonical_payload(obj)
            )
            return True
        except (InvalidSignature, ValueError):
            return False
