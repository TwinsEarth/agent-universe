"""AIP SDK 测试。"""

import pytest
from aip import (
    AgentCard, Task, TaskStatus, SkillSpec, Pricing,
    MemoryDHT, ShardedIndex, shard_of,
)


def test_agent_card():
    card = AgentCard.new("did:aip:test", "Test Agent")
    card.with_capability("text-generation")
    assert card.did == "did:aip:test"
    assert card.name == "Test Agent"
    assert "text-generation" in card.capabilities


def test_task_state_machine():
    task = Task(
        id="task-1",
        requester="did:aip:requester",
        capability="text-generation",
        payload={"prompt": "hello"},
    )
    assert task.status == TaskStatus.PENDING

    task.assign("did:aip:executor")
    assert task.status == TaskStatus.ASSIGNED

    task.start()
    assert task.status == TaskStatus.RUNNING

    task.complete({"result": "world"})
    assert task.status == TaskStatus.COMPLETED

    task.verify()
    assert task.status == TaskStatus.VERIFIED

    task.settle()
    assert task.status == TaskStatus.SETTLED


def test_shard_of():
    shard1 = shard_of("key1", 16)
    shard2 = shard_of("key1", 16)
    assert shard1 == shard2
    assert 0 <= shard1 < 16


def test_publish_and_find():
    dht = MemoryDHT()
    index = ShardedIndex(num_shards=4)

    card = AgentCard.new("did:aip:agent1", "Agent 1")
    card.with_capability("code-generation")

    import json
    dht.put("did:aip:agent1", json.dumps(card.__dict__).encode())
    index.put("did:aip:agent1", card)

    assert len(dht) == 1
    assert len(index) == 1
    assert index.get("did:aip:agent1").name == "Agent 1"


def test_skill_spec():
    skill = SkillSpec(
        skill_id="text-gen",
        version="1.0.0",
        description="Text generation skill",
        price=100,
    )
    assert skill.skill_id == "text-gen"
    assert skill.price == 100


def test_pricing():
    pricing = Pricing(
        base_price=1000,
        per_task_price=10,
        currency="GSP",
    )
    assert pricing.base_price == 1000
    assert pricing.per_task_price == 10
