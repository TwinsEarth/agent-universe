"""分片索引实现。"""

import hashlib
from dataclasses import dataclass, field
from typing import Any, Optional


def shard_of(key: str, num_shards: int = 16) -> int:
    """计算 key 所属的分片编号。"""
    h = hashlib.sha256(key.encode()).hexdigest()
    return int(h[:8], 16) % num_shards


@dataclass
class ShardMetadata:
    """分片元数据。"""
    shard_id: int
    count: int = 0
    keys: list[str] = field(default_factory=list)


class ShardedIndex:
    """分片索引。"""

    def __init__(self, num_shards: int = 16):
        self.num_shards = num_shards
        self.shards: dict[int, ShardMetadata] = {}
        self.data: dict[str, Any] = {}

    def put(self, key: str, value: Any) -> int:
        shard_id = shard_of(key, self.num_shards)
        self.data[key] = value

        if shard_id not in self.shards:
            self.shards[shard_id] = ShardMetadata(shard_id=shard_id)

        shard = self.shards[shard_id]
        shard.count += 1
        shard.keys.append(key)

        return shard_id

    def get(self, key: str) -> Optional[Any]:
        return self.data.get(key)

    def get_shard_keys(self, shard_id: int) -> list[str]:
        if shard_id not in self.shards:
            return []
        return self.shards[shard_id].keys.copy()

    def get_shard_stats(self) -> dict[int, int]:
        return {sid: meta.count for sid, meta in self.shards.items()}

    def __len__(self) -> int:
        return len(self.data)
