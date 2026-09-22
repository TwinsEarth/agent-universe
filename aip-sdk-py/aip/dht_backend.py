"""DHT 后端：内存版。"""

from dataclasses import dataclass, field


@dataclass
class MemoryDHT:
    _values: dict[str, bytes] = field(default_factory=dict)
    _providers: dict[str, list[str]] = field(default_factory=dict)

    async def get(self, key: str) -> bytes | None:
        return self._values.get(key)

    async def put(self, key: str, value: bytes) -> None:
        self._values[key] = value

    async def provide(self, key: str, peer_id: str = "local") -> None:
        if key not in self._providers:
            self._providers[key] = []
        self._providers[key].append(peer_id)

    async def find_providers(self, key: str) -> list[str]:
        return self._providers.get(key, [])
