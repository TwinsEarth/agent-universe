"""AIP 协议核心数据模型。"""

from dataclasses import dataclass, field
from enum import Enum
from typing import Any, Optional


class TaskStatus(str, Enum):
    PENDING = "pending"
    ASSIGNED = "assigned"
    RUNNING = "running"
    COMPLETED = "completed"
    VERIFIED = "verified"
    SETTLED = "settled"
    FAILED = "failed"


@dataclass
class AgentCard:
    did: str
    name: str
    capabilities: list[str] = field(default_factory=list)
    endpoints: list[str] = field(default_factory=list)
    reputation_bps: int = 5000

    @classmethod
    def new(cls, did: str, name: str):
        return cls(did=did, name=name)

    def with_capability(self, cap: str):
        self.capabilities.append(cap)
        return self


@dataclass
class Task:
    id: str
    requester: str
    capability: str
    payload: dict[str, Any]
    executor: Optional[str] = None
    result: Optional[dict[str, Any]] = None
    status: TaskStatus = TaskStatus.PENDING
    budget: int = 0

    def assign(self, executor: str) -> None:
        self.executor = executor
        self.status = TaskStatus.ASSIGNED

    def start(self) -> None:
        self.status = TaskStatus.RUNNING

    def complete(self, result: dict[str, Any]) -> None:
        self.result = result
        self.status = TaskStatus.COMPLETED

    def verify(self) -> None:
        self.status = TaskStatus.VERIFIED

    def settle(self) -> None:
        self.status = TaskStatus.SETTLED

    def fail(self) -> None:
        self.status = TaskStatus.FAILED
