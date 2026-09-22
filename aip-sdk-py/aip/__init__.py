"""Agent Universe AIP SDK v1.0.0"""

from .models import AgentCard, Task, TaskStatus
from .dht_backend import MemoryDHT
from .index.sharded import ShardedIndex, shard_of

__version__ = "1.0.0"
