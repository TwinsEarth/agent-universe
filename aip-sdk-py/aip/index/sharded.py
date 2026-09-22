"""DHT 分片索引。"""

from __future__ import annotations
import hashlib
from typing import Any
from ..dht_backend import MemoryDHT


def _h(did: str) -> int:
    return int(hashlib.sha256(did.encode()).hexdigest(), 16)


def shard_of(did: str, num_shards: int) -> int:
    return _h(did) % num_shards


class ShardedIndex:
    def __init__(self, dht: MemoryDHT, peer_id: str = "local"):
        self.dht = dht
        self.peer_id = peer_id

    async def publish_card(self, card: Any) -> None:
        for cap in card.capabilities:
            sid = shard_of(card.did, 1)
            await self.dht.provide(f"/aip/cap/{cap}/s/{sid}", self.peer_id)

    async def find_by_capability(self, cap: str) -> list[str]:
        providers = await self.dht.find_providers(f"/aip/cap/{cap}/s/0")
        return list(set(providers))
