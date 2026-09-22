"""AIP SDK 基础测试。"""
import asyncio
from aip.models import AgentCard, Task, TaskStatus
from aip.dht_backend import MemoryDHT
from aip.index.sharded import ShardedIndex, shard_of


def test_agent_card():
    card = AgentCard.new("did:aip:test", "test-agent")
    card.with_capability("text.translate")
    assert card.did == "did:aip:test"
    assert len(card.capabilities) == 1


def test_task_state_machine():
    task = Task(id="t1", requester="did:aip:r", capability="text.translate", payload={"text": "hello"})
    assert task.status == TaskStatus.PENDING
    task.assign("did:aip:e")
    task.start()
    task.complete({"ok": True})
    task.verify()
    task.settle()
    assert task.status == TaskStatus.SETTLED


def test_shard_of():
    assert 0 <= shard_of("did:aip:xyz", 4) < 4


def test_publish_and_find():
    async def _run():
        dht = MemoryDHT()
        index = ShardedIndex(dht, peer_id="peer-a")
        card = AgentCard.new("did:aip:0x1", "agent-1")
        card.with_capability("text.translate")
        await index.publish_card(card)
        results = await index.find_by_capability("text.translate")
        assert "peer-a" in results
    asyncio.run(_run())
