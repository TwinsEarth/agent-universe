"""内存 DHT 后端实现。"""

from typing import Optional


class MemoryDHT:
    """内存 DHT 实现，用于测试和开发。"""

    def __init__(self):
        self._store: dict[str, bytes] = {}

    def put(self, key: str, value: bytes) -> None:
        self._store[key] = value

    def get(self, key: str) -> Optional[bytes]:
        return self._store.get(key)

    def remove(self, key: str) -> None:
        self._store.pop(key, None)

    def keys(self) -> list[str]:
        return list(self._store.keys())

    def __len__(self) -> int:
        return len(self._store)
