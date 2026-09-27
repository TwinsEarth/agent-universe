// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title ReputationRegistry - 链下信誉的链上锚定（原 ReputationBridge）
/// @notice 修复 GAP §9.4：addVerifier 此前是 onlyVerifier（任一验证者可无限铸新验证者、无 owner/上限/移除）；
///         recordReputation 接受任意 uint32（与链下 0-10000bps 量纲不一致）；事件只带 honesty 一维；
///         getLatestReputation 对未知 DID revert 而非返回零。
contract ReputationRegistry {
    struct Scores {
        uint32 quality;
        uint32 speed;
        uint32 honesty;
        uint32 availability;
        bool set;
    }

    struct ReputationSnapshot {
        bytes32 agentDidHash;
        uint32 quality;
        uint32 speed;
        uint32 honesty;
        uint32 availability;
        uint64 epoch;
        uint64 finalizedAt;
    }

    struct EpochData {
        bool exists;
        address[] submitters;
        mapping(address => Scores) scores;
        bool finalized;
    }

    uint256 public constant MAX_VERIFIERS = 32;
    uint32 public constant MAX_BPS = 10000;

    address public owner;
    uint256 public verifierCount;
    mapping(address => bool) public verifiers;

    mapping(bytes32 => EpochData) private _epochs;
    mapping(bytes32 => bool) private _submitted;
    mapping(bytes32 => mapping(uint64 => ReputationSnapshot)) private _finalSnapshot;
    mapping(bytes32 => uint64[]) private _agentEpochs;

    event VerifierAdded(address indexed verifier);
    event VerifierRemoved(address indexed verifier);
    event ReputationSubmitted(bytes32 indexed agentDidHash, uint64 indexed epoch, address indexed verifier);
    event ReputationFinalized(
        bytes32 indexed agentDidHash,
        uint64 indexed epoch,
        uint32 quality,
        uint32 speed,
        uint32 honesty,
        uint32 availability
    );

    modifier onlyOwner() {
        require(msg.sender == owner, "not owner");
        _;
    }

    constructor() {
        owner = msg.sender;
    }

    function addVerifier(address v) external onlyOwner {
        require(v != address(0), "zero verifier");
        require(!verifiers[v], "already verifier");
        require(verifierCount < MAX_VERIFIERS, "verifier cap reached");
        verifiers[v] = true;
        verifierCount += 1;
        emit VerifierAdded(v);
    }

    function removeVerifier(address v) external onlyOwner {
        require(verifiers[v], "not verifier");
        verifiers[v] = false;
        verifierCount -= 1;
        emit VerifierRemoved(v);
    }

    function _epochKey(bytes32 agent, uint64 epoch) private pure returns (bytes32) {
        return keccak256(abi.encode(agent, epoch));
    }

    /// @notice 验证者提交某 epoch 的信誉评分（各维度 0..10000 bps）。
    ///         同一验证者对同一 (agent,epoch) 重复提交：内容完全相同视为幂等 no-op，内容冲突则 revert。
    function recordReputation(
        bytes32 agentDidHash,
        uint64 epoch,
        uint32 quality,
        uint32 speed,
        uint32 honesty,
        uint32 availability
    ) external {
        require(verifiers[msg.sender], "not verifier");
        require(quality <= MAX_BPS && speed <= MAX_BPS && honesty <= MAX_BPS && availability <= MAX_BPS,
            "score out of bps range");

        bytes32 ek = _epochKey(agentDidHash, epoch);
        bytes32 sk = keccak256(abi.encode(agentDidHash, epoch, msg.sender));

        if (_submitted[sk]) {
            Scores storage prev = _epochs[ek].scores[msg.sender];
            require(
                prev.quality == quality && prev.speed == speed &&
                prev.honesty == honesty && prev.availability == availability,
                "conflicting resubmission"
            );
            return; // 幂等 no-op
        }

        EpochData storage e = _epochs[ek];
        if (!e.exists) {
            e.exists = true;
        }
        e.submitters.push(msg.sender);
        e.scores[msg.sender] = Scores({
            quality: quality,
            speed: speed,
            honesty: honesty,
            availability: availability,
            set: true
        });
        _submitted[sk] = true;
        emit ReputationSubmitted(agentDidHash, epoch, msg.sender);
    }

    function requiredQuorum() public view returns (uint256) {
        require(verifierCount > 0, "no verifiers");
        return verifierCount / 2 + 1;
    }

    /// @notice 当某 (agent,epoch) 达到验证者多数提交时，对各维度取中位数定稿（任何人可触发）。
    function finalize(bytes32 agentDidHash, uint64 epoch) external {
        bytes32 ek = _epochKey(agentDidHash, epoch);
        EpochData storage e = _epochs[ek];
        require(e.exists, "no submissions");
        require(!e.finalized, "already finalized");
        require(e.submitters.length >= requiredQuorum(), "quorum not reached");

        uint256 n = e.submitters.length;
        uint256[] memory q = new uint256[](n);
        uint256[] memory s = new uint256[](n);
        uint256[] memory h = new uint256[](n);
        uint256[] memory a = new uint256[](n);
        for (uint256 i = 0; i < n; i++) {
            Scores storage sc = e.scores[e.submitters[i]];
            q[i] = sc.quality;
            s[i] = sc.speed;
            h[i] = sc.honesty;
            a[i] = sc.availability;
        }

        ReputationSnapshot memory snap = ReputationSnapshot({
            agentDidHash: agentDidHash,
            quality: uint32(median(q)),
            speed: uint32(median(s)),
            honesty: uint32(median(h)),
            availability: uint32(median(a)),
            epoch: epoch,
            finalizedAt: uint64(block.timestamp)
        });

        e.finalized = true;
        _finalSnapshot[agentDidHash][epoch] = snap;
        _agentEpochs[agentDidHash].push(epoch);
        emit ReputationFinalized(agentDidHash, epoch, snap.quality, snap.speed, snap.honesty, snap.availability);
    }

    /// @notice 返回某 agent 最新定稿快照；从未定稿则返回全零快照（不 revert）。
    function getLatestReputation(bytes32 agentDidHash) external view returns (ReputationSnapshot memory) {
        uint64[] storage eps = _agentEpochs[agentDidHash];
        if (eps.length == 0) {
            return ReputationSnapshot({
                agentDidHash: agentDidHash,
                quality: 0, speed: 0, honesty: 0, availability: 0,
                epoch: 0, finalizedAt: 0
            });
        }
        return _finalSnapshot[agentDidHash][eps[eps.length - 1]];
    }

    function getFinalizedEpochCount(bytes32 agentDidHash) external view returns (uint256) {
        return _agentEpochs[agentDidHash].length;
    }

    function submissionCount(bytes32 agentDidHash, uint64 epoch) external view returns (uint256) {
        return _epochs[_epochKey(agentDidHash, epoch)].submitters.length;
    }

    function median(uint256[] memory vals) private pure returns (uint256) {
        for (uint256 i = 1; i < vals.length; i++) {
            uint256 key = vals[i];
            int256 j = int256(i) - 1;
            while (j >= 0 && vals[uint256(j)] > key) {
                vals[uint256(j) + 1] = vals[uint256(j)];
                j--;
            }
            // j == -1 时插入位置为 0；不能用 uint256(-1)+1（会溢出 panic 0x11）。
            uint256 pos = j < 0 ? 0 : uint256(j) + 1;
            vals[pos] = key;
        }
        return vals[vals.length / 2];
    }
}
