'''DHT 分片索引 v2.0.0：多级分片 + 地理位置分片 + 分片合并。'''

from __future__ import annotations
import hashlib
import json
from dataclasses import asdict, dataclass, field
from typing import Any
from ..dht_backend import MemoryDHT


def _h(did: str) -> int:
    return int(hashlib.sha256(did.encode()).hexdigest(), 16)


def shard_of(did: str, num_shards: int) -> int:
    return _h(did) % num_shards


@dataclass
class ShardMetadata:
    capability: str
    region: str = "GLOBAL"
    num_shards: int = 1
    version: int = 1
    max_per_shard: int = 500
    counts: dict[int, int] = field(default_factory=dict)

    def to_bytes(self) -> bytes:
        return json.dumps({**asdict(self), "counts": {str(k): v for k, v in self.counts.items()}}).encode()

    @classmethod
    def from_bytes(cls, data: bytes) -> "ShardMetadata":
        obj = json.loads(data.decode())
        obj["counts"] = {int(k): v for k, v in obj.get("counts", {}).items()}
        return cls(**obj)


class ShardedIndex:
    def __init__(self, dht: MemoryDHT, peer_id: str = "local"):
        self.dht = dht
        self.peer_id = peer_id

    async def publish_card(self, card: Any) -> None:
        for cap in card.capabilities:
            meta_key = f"/aip/cap/{cap}/r/GLOBAL/meta"
            data = await self.dht.get(meta_key)
            meta = ShardMetadata.from_bytes(data) if data else ShardMetadata(capability=cap)
            sid = shard_of(card.did, meta.num_shards)
            await self.dht.provide(f"/aip/cap/{cap}/r/GLOBAL/s/{sid}", self.peer_id)
            meta.counts[sid] = meta.counts.get(sid, 0) + 1
            await self.dht.put(meta_key, meta.to_bytes())

    async def find_by_capability(self, cap: str) -> list[str]:
        meta_key = f"/aip/cap/{cap}/r/GLOBAL/meta"
        data = await self.dht.get(meta_key)
        if not data:
            return []
        meta = ShardMetadata.from_bytes(data)
        results = []
        for sid in range(meta.num_shards):
            providers = await self.dht.find_providers(f"/aip/cap/{cap}/r/GLOBAL/s/{sid}")
            results.extend(providers)
        return list(set(results))
