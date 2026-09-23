// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title ReputationBridge - 信誉跨链移植
/// @notice 将链下不可转让信誉锚定到链上
contract ReputationBridge {
    struct ReputationSnapshot {
        bytes32 agentDidHash;
        uint32 quality;
        uint32 speed;
        uint32 honesty;
        uint32 availability;
        uint64 recordedAt;
    }

    mapping(bytes32 => ReputationSnapshot[]) public snapshots;
    mapping(address => bool) public verifiers;

    event ReputationUpdated(bytes32 indexed agentDidHash, uint32 honesty, uint64 timestamp);

    modifier onlyVerifier() {
        require(verifiers[msg.sender], "not verifier");
        _;
    }

    constructor() {
        verifiers[msg.sender] = true;
    }

    function addVerifier(address v) external onlyVerifier {
        verifiers[v] = true;
    }

    function recordReputation(
        bytes32 agentDidHash,
        uint32 quality,
        uint32 speed,
        uint32 honesty,
        uint32 availability
    ) external onlyVerifier {
        snapshots[agentDidHash].push(ReputationSnapshot({
            agentDidHash: agentDidHash,
            quality: quality,
            speed: speed,
            honesty: honesty,
            availability: availability,
            recordedAt: uint64(block.timestamp)
        }));
        emit ReputationUpdated(agentDidHash, honesty, uint64(block.timestamp));
    }

    function getLatestReputation(bytes32 agentDidHash) external view returns (ReputationSnapshot memory) {
        ReputationSnapshot[] storage snaps = snapshots[agentDidHash];
        require(snaps.length > 0, "no snapshots");
        return snaps[snaps.length - 1];
    }

    function getSnapshotCount(bytes32 agentDidHash) external view returns (uint256) {
        return snapshots[agentDidHash].length;
    }
}
