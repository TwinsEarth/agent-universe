"""三端（Rust / JS / Python）逐字节互验签名钉向量（v3.5.3，AU-16）。

读取仓库根 ``conformance/vectors.json`` 中固定 seed=[0x01;32] 的绝对向量，断言
Python 侧由该 seed 派生的公钥 / DID / 对规范载荷的签名与 JSON 完全一致，坐实
Python 与 Rust(gsn-core/tests/cross_lang_signature.rs)、JS(js/test/test.js) 三端互验。
"""

from __future__ import annotations

import json
from pathlib import Path

import pytest

from aip import AipIdentity, canonical_payload
from aip.crypto import _HAVE_CRYPTO

pytestmark = pytest.mark.skipif(
    not _HAVE_CRYPTO, reason="需要 cryptography：pip install aip-sdk[crypto]"
)

# 本文件位于 <repo>/aip-sdk-py/tests/，仓库根为上两级。
REPO_ROOT = Path(__file__).resolve().parents[2]
VECTORS = json.loads((REPO_ROOT / "conformance" / "vectors.json").read_text("utf-8"))


def _vector() -> dict:
    for v in VECTORS:
        if v.get("source", "").endswith("cross_lang_signature.rs:11-14"):
            return v
    raise AssertionError("vectors.json 未找到 cross-lang 钉向量")


def test_cross_lang_identity_and_signature_vector():
    v = _vector()
    seed_hex = v["seed"]
    assert seed_hex.startswith("0x")
    seed = bytes.fromhex(seed_hex[2:])
    assert len(seed) == 32

    ident = AipIdentity.from_seed(seed)

    # 公钥逐字节一致。
    assert ident.public_key.hex() == v["public_key"], "公钥与钉向量不一致"

    # DID：upstream_did 是 did:aip:...（本模块派生口径）。
    assert ident.did == v["upstream_did"], f"DID 不一致: {ident.did} != {v['upstream_did']}"
    # new_did 仅前缀不同（did:nau:），哈希部分应一致。
    assert ident.did.removeprefix("did:aip:") == v["new_did"].removeprefix("did:nau:")

    # 规范载荷：从对象重建并断言与钉向量里的规范串逐字节一致（证明 canonicalization 对齐）。
    obj = {
        "capabilities": ["text-generation", "mcp"],
        "did": v["upstream_did"],
        "name": "CrossLang",
        "stake": 100,
    }
    canon = canonical_payload(obj).decode("utf-8")
    assert canon == v["canonical_payload"], f"规范载荷不一致:\n{canon}\n!=\n{v['canonical_payload']}"

    # 对规范载荷签名，与钉向量逐字节一致。
    sig = ident.sign_payload(canonical_payload(obj)).hex()
    assert sig == v["signature"], "签名与钉向量不一致"

    # 反向用钉向量公钥验签该签名，双保险。
    signed = dict(obj)
    signed["signature"] = v["signature"]
    assert AipIdentity.verify_object(signed, bytes.fromhex(v["public_key"]))
