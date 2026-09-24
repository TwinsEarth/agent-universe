"""身份、签名与 ACA 对象测试（需要 cryptography：pip install aip-sdk[crypto]）。"""

import pytest

from aip import (
    AipIdentity,
    build_manifest,
    build_envelope,
    build_message,
    build_receipt,
    proposal_message,
    verify_manifest,
    verify_message,
    verify_receipt,
    verify_receipt_result,
    build_task_spec,
    canonical_payload,
)
from aip.crypto import _HAVE_CRYPTO

pytestmark = pytest.mark.skipif(
    not _HAVE_CRYPTO, reason="需要 cryptography：pip install aip-sdk[crypto]"
)


@pytest.fixture
def owner():
    return AipIdentity.generate()


@pytest.fixture
def peer():
    return AipIdentity.generate()


def test_did_format(owner):
    assert owner.did.startswith("did:aip:")
    # sha256 前 8 字节 → 16 hex 字符
    assert len(owner.did) == len("did:aip:") + 16


def test_from_seed_deterministic():
    seed = bytes(range(32))
    a = AipIdentity.from_seed(seed)
    b = AipIdentity.from_seed(seed)
    assert a.did == b.did
    assert a.public_key == b.public_key


def test_from_seed_bad_length():
    with pytest.raises(ValueError):
        AipIdentity.from_seed(b"short")


def test_manifest_sign_verify(owner):
    manifest = build_manifest(
        owner, "Agent1", ["text-generation"], stake=100, mcp_tool_count=3
    )
    assert manifest["signature"]
    assert verify_manifest(manifest, owner.public_key)


def test_manifest_tamper_rejected(owner):
    manifest = build_manifest(owner, "Agent1", ["x"])
    tampered = dict(manifest)
    tampered["name"] = "Fake"
    assert not verify_manifest(tampered, owner.public_key)


def test_message_sign_verify(owner, peer):
    env = build_envelope(owner.did, "text-generation", "out", budget=10)
    msg = proposal_message(owner, peer.did, env)
    assert verify_message(msg, owner.public_key)
    # 用他人公钥验签应失败
    assert not verify_message(msg, peer.public_key)


def test_message_tamper_rejected(owner, peer):
    env = build_envelope(owner.did, "cap", "out")
    msg = build_message(owner, "TaskProposal", peer.did, env)
    tampered = dict(msg)
    tampered["to_did"] = "did:aip:deadbeef"
    assert not verify_message(tampered, owner.public_key)


def test_receipt_sign_verify_and_hash(owner):
    result = b"hello result"
    receipt = build_receipt(owner, "task-1", result)
    assert verify_receipt(receipt, owner.public_key)
    assert verify_receipt_result(receipt, result)
    assert not verify_receipt_result(receipt, b"different")


def test_receipt_tamper_rejected(owner):
    receipt = build_receipt(owner, "task-1", b"data")
    tampered = dict(receipt)
    tampered["status"] = "Verified"
    assert not verify_receipt(tampered, owner.public_key)


def test_canonical_deterministic_and_sorted():
    obj = {"b": 1, "a": 2, "signature": "deadbeef"}
    p1 = canonical_payload(obj)
    p2 = canonical_payload(obj)
    assert p1 == p2
    # signature 被移除
    assert b"signature" not in p1
    # 紧凑、按 key 字典序：a 在 b 前
    assert p1.index(b'"a"') < p1.index(b'"b"')
    # 紧凑分隔符（无多余空格）
    assert b": " not in p1 and b", " not in p1


def test_build_task_spec_six_fields(owner):
    spec = build_task_spec(
        owner.did, "完成目标", ["步骤1", "步骤2"], 10.0, ["text-generation"]
    )
    for field in ("goal", "context", "todo", "budget", "required_skills", "requester"):
        assert field in spec
    assert spec["budget"] == 10.0
    assert spec["verification_policy"]["BftLite"] == {"n": 4, "f": 1}
