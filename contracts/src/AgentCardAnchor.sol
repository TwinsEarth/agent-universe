// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title AgentCardAnchor - Agent 磁力链锚定
/// @notice 将 AgentManifest 的 CID 哈希锚定到链上
contract AgentCardAnchor {
    struct Anchor {
        bytes32 cidHash;
        bytes32 agentDidHash;
        uint64 anchoredAt;
        address anchorer;
    }

    mapping(bytes32 => Anchor) public anchors;
    mapping(address => bytes32[]) public agentAnchors;

    event Anchored(bytes32 indexed cidHash, bytes32 indexed agentDidHash, uint64 indexed anchoredAt, address anchorer);

    function anchor(bytes32 cidHash, bytes32 agentDidHash) external {
        anchors[cidHash] = Anchor({
            cidHash: cidHash,
            agentDidHash: agentDidHash,
            anchoredAt: uint64(block.timestamp),
            anchorer: msg.sender
        });
        agentAnchors[msg.sender].push(cidHash);
        emit Anchored(cidHash, agentDidHash, uint64(block.timestamp), msg.sender);
    }

    function verify(bytes32 cidHash) external view returns (bool) {
        return anchors[cidHash].anchoredAt > 0;
    }

    function getAnchor(bytes32 cidHash) external view returns (Anchor memory) {
        return anchors[cidHash];
    }
}
