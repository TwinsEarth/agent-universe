// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title PoCVSettlement - PoCV 任务结算
/// @notice 任务奖励托管、执行者质押、验证者投票、结算与罚没。修复 GAP §9.4：
///         createTask 此前不读 msg.value（可零资金建任务、结算锁死）；verify/dispute 无访问控制；
///         settle 先外呼后记账（违背 CEI）、无重入保护；requester 可自接；争议仍全额付执行者；
///         无执行者时把资金发给 address(0)。
contract PoCVSettlement {
    enum TaskStatus { None, Open, InProgress, Verified, Disputed, Settled }

    struct Task {
        address requester;
        address executor;
        uint256 rewardAmount;
        uint256 stakeAmount;
        TaskStatus status;
        uint64 createdAt;
        uint64 completedAt;
    }

    address public owner;
    address public treasury;
    uint256 public minExecutorStake;
    uint256 public verifierCount;

    mapping(bytes32 => Task) public tasks;
    mapping(address => uint256) public totalStaked;
    mapping(address => uint256) public claimable;

    mapping(address => bool) public verifiers;
    mapping(bytes32 => mapping(address => bool)) private _hasVoted;
    mapping(bytes32 => uint256) public verifyVoteCount;
    mapping(bytes32 => uint256) public disputeVoteCount;

    bool private _locked;

    event TaskCreated(bytes32 indexed taskId, address indexed requester, uint256 reward);
    event TaskAccepted(bytes32 indexed taskId, address indexed executor, uint256 stake);
    event TaskVerified(bytes32 indexed taskId, uint256 reward);
    event TaskDisputed(bytes32 indexed taskId);
    event TaskSettled(bytes32 indexed taskId, address indexed executor, uint256 amount);

    modifier onlyOwner() {
        require(msg.sender == owner, "not owner");
        _;
    }

    modifier nonReentrant() {
        require(!_locked, "reentrant call");
        _locked = true;
        _;
        _locked = false;
    }

    constructor() {
        owner = msg.sender;
        treasury = msg.sender;
    }

    // ── 资金托管 ──
    /// @notice 创建任务并托管奖励；msg.value 即奖励金额（资金 == 奖励，拒绝零资金）。
    function createTask(bytes32 taskId) external payable {
        require(tasks[taskId].status == TaskStatus.None, "task exists");
        require(msg.value > 0, "reward must be positive");
        tasks[taskId] = Task({
            requester: msg.sender,
            executor: address(0),
            rewardAmount: msg.value,
            stakeAmount: 0,
            status: TaskStatus.Open,
            createdAt: uint64(block.timestamp),
            completedAt: 0
        });
        emit TaskCreated(taskId, msg.sender, msg.value);
    }

    /// @notice 执行者接单并质押；禁止 requester 自接，质押不低于下限。
    function acceptTask(bytes32 taskId) external payable nonReentrant {
        Task storage t = tasks[taskId];
        require(t.status == TaskStatus.Open, "task not open");
        require(msg.sender != t.requester, "requester cannot self-execute");
        require(msg.value >= minExecutorStake, "stake below minimum");
        t.executor = msg.sender;
        t.stakeAmount = msg.value;
        t.status = TaskStatus.InProgress;
        totalStaked[msg.sender] += msg.value;
        emit TaskAccepted(taskId, msg.sender, msg.value);
    }

    // ── 验证者治理 ──
    function addVerifier(address v) external onlyOwner {
        require(!verifiers[v], "already verifier");
        verifiers[v] = true;
        verifierCount += 1;
    }

    function removeVerifier(address v) external onlyOwner {
        require(verifiers[v], "not verifier");
        verifiers[v] = false;
        verifierCount -= 1;
    }

    function setTreasury(address t) external onlyOwner {
        require(t != address(0), "zero treasury");
        treasury = t;
    }

    function setMinExecutorStake(uint256 v) external onlyOwner {
        minExecutorStake = v;
    }

    function quorum() public view returns (uint256) {
        require(verifierCount > 0, "no verifiers registered");
        return verifierCount / 2 + 1;
    }

    /// @notice 验证者投票（approve=true 验收 / false 争议）；每位验证者每任务一票，达到多数即定态。
    function castVerification(bytes32 taskId, bool approve) external {
        require(verifiers[msg.sender], "not verifier");
        Task storage t = tasks[taskId];
        require(t.status == TaskStatus.InProgress, "task not in progress");
        require(!_hasVoted[taskId][msg.sender], "verifier already voted");
        _hasVoted[taskId][msg.sender] = true;

        uint256 q = quorum();
        if (approve) {
            verifyVoteCount[taskId] += 1;
            if (verifyVoteCount[taskId] >= q) {
                t.status = TaskStatus.Verified;
                t.completedAt = uint64(block.timestamp);
                emit TaskVerified(taskId, t.rewardAmount);
            }
        } else {
            disputeVoteCount[taskId] += 1;
            if (disputeVoteCount[taskId] >= q) {
                t.status = TaskStatus.Disputed;
                t.completedAt = uint64(block.timestamp);
                emit TaskDisputed(taskId);
            }
        }
    }

    // ── 结算（状态先行、写 claimable，无外呼 = CEI；资金以 pull 方式领取）──
    function settleTask(bytes32 taskId) external {
        Task storage t = tasks[taskId];
        require(t.status == TaskStatus.Verified || t.status == TaskStatus.Disputed, "task not resolved");
        bool verified = t.status == TaskStatus.Verified;

        totalStaked[t.executor] -= t.stakeAmount;
        t.status = TaskStatus.Settled;

        if (verified) {
            claimable[t.executor] += t.rewardAmount + t.stakeAmount;
            emit TaskSettled(taskId, t.executor, t.rewardAmount + t.stakeAmount);
        } else {
            // 争议：奖励退还请求者；执行者质押罚没入 treasury（不发给 address(0)）。
            claimable[t.requester] += t.rewardAmount;
            claimable[treasury] += t.stakeAmount;
            emit TaskSettled(taskId, t.executor, 0);
        }
    }

    /// @notice 领取结算分配（pull 支付 + 重入保护）。
    function withdraw() external nonReentrant {
        uint256 amount = claimable[msg.sender];
        require(amount > 0, "nothing to withdraw");
        claimable[msg.sender] = 0;
        (bool ok, ) = msg.sender.call{value: amount}("");
        require(ok, "transfer failed");
    }
}
