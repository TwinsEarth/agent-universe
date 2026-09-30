// Agent Universe v2.9.0 — 委员身份与签名（与 gsn-core 口径一致）
//
// 标准 Ed25519 + SHA-256；DID = did:nau:<hex(sha256(pubkey)[0..8])>。
// 投票签名载荷格式见 gsn-core marketplace/qa_committee.rs signing_bytes。

import * as ed from '@noble/ed25519';
import { sha256 } from '@noble/hashes/sha256';
import { bytesToHex } from '@noble/hashes/utils';

// @noble/ed25519 v2：getPublicKey / sign / verify 为同步，无需设置 sha512Sync。

export function isWeakPubkey(pub) {
  if (pub.length !== 32) return true;
  // 全相同（涵盖全 0x00 / 全 0xFF）
  for (let i = 1; i < pub.length; i++) {
    if (pub[i] !== pub[0]) return false;
  }
  return true;
}

export function fingerprint(pub) {
  return bytesToHex(sha256(pub).slice(0, 8));
}

export function didFromPubkey(pub) {
  return 'did:nau:' + fingerprint(pub);
}

// 生成 n 个非弱公钥委员
export function makeCommittee(n) {
  const out = [];
  let guard = 0;
  while (out.length < n && guard < n * 60) {
    guard++;
    const priv = ed.utils.randomPrivateKey();
    const pub = ed.getPublicKey(priv);
    if (isWeakPubkey(pub)) continue;
    out.push({ did: didFromPubkey(pub), public_key: bytesToHex(pub), priv, pub });
  }
  if (out.length !== n) {
    throw new Error('委员密钥生成失败（弱公钥重试上限）');
  }
  return out;
}

function signingPayload(taskId, round, voter, tag, nonce, issuedAt, expiresAt) {
  return (
    'AU-QA-VOTE\n' +
    'task_id=' + taskId + '\n' +
    'round=' + round + '\n' +
    'voter=' + voter + '\n' +
    'vote=' + tag + '\n' +
    'nonce=' + nonce + '\n' +
    'issued_at=' + issuedAt + '\n' +
    'expires_at=' + expiresAt
  );
}

// 由委员签发一张票；voteTag: Stop / Continue / Silent
export function signVote(member, taskId, round, voteTag, nowSec) {
  const tag = voteTag.toUpperCase();
  const nonce = bytesToHex(ed.utils.randomPrivateKey()).slice(0, 24);
  const issuedAt = nowSec;
  const expiresAt = nowSec + 300;
  const msg = signingPayload(taskId, round, member.did, tag, nonce, issuedAt, expiresAt);
  const sig = ed.sign(new TextEncoder().encode(msg), member.priv);
  return {
    task_id: taskId,
    round,
    voter: member.did,
    vote: voteTag,
    nonce,
    issued_at: issuedAt,
    expires_at: expiresAt,
    signature: bytesToHex(sig),
  };
}

// 构造一次完整验证请求（委员 + 签名票）
export function buildVerification(taskId, round, n, voteTag) {
  const committee = makeCommittee(n);
  const now = Math.floor(Date.now() / 1000);
  return {
    round,
    members: committee.map((m) => ({ did: m.did, public_key: m.public_key })),
    signed_votes: committee.map((m) => signVote(m, taskId, round, voteTag, now)),
  };
}

// 为 Agent 生成独立身份（注册用），返回 {did, public_key, private_key}
export function makeAgentIdentity() {
  for (let i = 0; i < 60; i++) {
    const priv = ed.utils.randomPrivateKey();
    const pub = ed.getPublicKey(priv);
    if (!isWeakPubkey(pub)) {
      return {
        did: didFromPubkey(pub),
        public_key: bytesToHex(pub),
        private_key: bytesToHex(priv),
      };
    }
  }
  throw new Error('Agent 身份生成失败');
}
