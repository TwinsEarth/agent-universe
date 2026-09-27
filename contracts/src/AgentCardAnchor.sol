// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title AgentCardAnchor - Agent 清单 CID 锚定
/// @notice 将 AgentManifest 的 CID 哈希锚定到链上。修复 GAP §9.4：
///         anchor 此前无认证、无条件覆盖已有锚；verify 不校验 agentDidHash。
contract AgentCardAnchor {
    struct Anchor {
        bytes32 cidHash;
        bytes32 agentDidHash;
        uint64 anchoredAt;
        address anchorer;
    }

    address public owner;
    mapping(address => bool) public authorizedAnchorers;
    mapping(bytes32 => Anchor) public anchors;
    mapping(address => bytes32[]) public agentAnchors;

    event AnchorerAdded(address indexed anchorer);
    event AnchorerRemoved(address indexed anchorer);
    event Anchored(bytes32 indexed cidHash, bytes32 indexed agentDidHash, uint64 indexed anchoredAt, address anchorer);

    modifier onlyOwner() {
        require(msg.sender == owner, "not owner");
        _;
    }

    constructor() {
        owner = msg.sender;
        authorizedAnchorers[msg.sender] = true;
        emit AnchorerAdded(msg.sender);
    }

    function addAnchorer(address a) external onlyOwner {
        authorizedAnchorers[a] = true;
        emit AnchorerAdded(a);
    }

    function removeAnchorer(address a) external onlyOwner {
        authorizedAnchorers[a] = false;
        emit AnchorerRemoved(a);
    }

    /// @notice 锚定一个 CID；仅授权锚定者可调用，且同一 cidHash 首写后不可变（不可覆盖）。
    function anchor(bytes32 cidHash, bytes32 agentDidHash) external {
        require(authorizedAnchorers[msg.sender], "not authorized anchorer");
        require(cidHash != bytes32(0), "empty cid");
        require(agentDidHash != bytes32(0), "empty did");
        require(anchors[cidHash].anchoredAt == 0, "anchor already exists; immutable");

        anchors[cidHash] = Anchor({
            cidHash: cidHash,
            agentDidHash: agentDidHash,
            anchoredAt: uint64(block.timestamp),
            anchorer: msg.sender
        });
        agentAnchors[msg.sender].push(cidHash);
        emit Anchored(cidHash, agentDidHash, uint64(block.timestamp), msg.sender);
    }

    /// @notice 校验 cidHash 已锚定，且其 agentDidHash 与调用者提供的一致（双校验）。
    function verify(bytes32 cidHash, bytes32 agentDidHash) external view returns (bool) {
        Anchor storage a = anchors[cidHash];
        return a.anchoredAt > 0 && a.agentDidHash == agentDidHash;
    }

    function getAnchor(bytes32 cidHash) external view returns (Anchor memory) {
        return anchors[cidHash];
    }
}
