// lib/dht.js
// MemoryDHT / ShardedIndex / shardOf（SHA-256 分片）

const crypto = require('node:crypto');

/** 计算 key 所属分片：SHA-256(key) 前 8 hex 转整数，对 numShards 取模 */
function shardOf(key, numShards) {
  const hash = crypto.createHash('sha256').update(String(key)).digest('hex');
  const prefix = parseInt(hash.slice(0, 8), 16);
  return prefix % numShards;
}

/** 内存 DHT（键值存储） */
class MemoryDHT {
  constructor() {
    this.store = new Map();
  }

  put(key, value) {
    this.store.set(String(key), value);
  }

  get(key) {
    return this.store.get(String(key));
  }

  has(key) {
    return this.store.has(String(key));
  }

  remove(key) {
    return this.store.delete(String(key));
  }

  get size() {
    return this.store.size;
  }

  /** 按前缀查找 */
  findByPrefix(prefix) {
    const result = [];
    for (const [k, v] of this.store) {
      if (k.startsWith(prefix)) result.push({ key: k, value: v });
    }
    return result;
  }
}

/** 分片索引：把 key 均匀分布到 numShards 个分片 */
class ShardedIndex {
  constructor(numShards = 16) {
    this.numShards = numShards;
    this.shards = [];
    for (let i = 0; i < numShards; i++) this.shards.push(new Map());
  }

  put(key, value) {
    const s = shardOf(key, this.numShards);
    this.shards[s].set(String(key), value);
    return s;
  }

  get(key) {
    const s = shardOf(key, this.numShards);
    return this.shards[s].get(String(key));
  }

  has(key) {
    const s = shardOf(key, this.numShards);
    return this.shards[s].has(String(key));
  }

  remove(key) {
    const s = shardOf(key, this.numShards);
    return this.shards[s].delete(String(key));
  }

  shardSize(i) {
    return this.shards[i].size;
  }

  get size() {
    return this.shards.reduce((sum, m) => sum + m.size, 0);
  }

  /** 各分片条目数（用于验证分布均匀性） */
  distribution() {
    return this.shards.map((m) => m.size);
  }
}

module.exports = { MemoryDHT, ShardedIndex, shardOf };
