"""Agent Universe AIP SDK v2.0.0"""

from .models import AgentCard, SkillSpec, Pricing, Task, TaskStatus
from .dht_backend import MemoryDHT
from .index.sharded import ShardedIndex, ShardMetadata, shard_of

__version__ = "2.0.0"
