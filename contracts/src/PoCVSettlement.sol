// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title PoCVSettlement - PoCV 任务结算
/// @notice 任务质押、结算和罚没
contract PoCVSettlement {
    enum TaskStatus { None, Staked, InProgress, Verified, Disputed, Settled }

    struct Task {
        bytes32 taskId;
        address executor;
        address requester;
        uint256 stakeAmount;
        uint256 rewardAmount;
        TaskStatus status;
        uint64 createdAt;
        uint64 completedAt;
    }

    mapping(bytes32 => Task) public tasks;
    mapping(address => uint256) public totalStaked;

    event TaskCreated(bytes32 indexed taskId, address indexed requester, uint256 reward);
    event TaskAccepted(bytes32 indexed taskId, address indexed executor, uint256 stake);
    event TaskVerified(bytes32 indexed taskId, uint256 reward);
    event TaskDisputed(bytes32 indexed taskId);
    event TaskSettled(bytes32 indexed taskId, address indexed executor, uint256 amount);

    function createTask(bytes32 taskId, uint256 rewardAmount) external payable {
        require(tasks[taskId].createdAt == 0, "task exists");
        tasks[taskId] = Task({
            taskId: taskId,
            executor: address(0),
            requester: msg.sender,
            stakeAmount: 0,
            rewardAmount: rewardAmount,
            status: TaskStatus.Staked,
            createdAt: uint64(block.timestamp),
            completedAt: 0
        });
        emit TaskCreated(taskId, msg.sender, rewardAmount);
    }

    function acceptTask(bytes32 taskId) external payable {
        Task storage t = tasks[taskId];
        require(t.createdAt > 0, "task not found");
        require(t.status == TaskStatus.Staked, "not stakeable");
        t.executor = msg.sender;
        t.stakeAmount = msg.value;
        t.status = TaskStatus.InProgress;
        totalStaked[msg.sender] += msg.value;
        emit TaskAccepted(taskId, msg.sender, msg.value);
    }

    function verifyTask(bytes32 taskId) external {
        Task storage t = tasks[taskId];
        require(t.status == TaskStatus.InProgress, "not in progress");
        t.status = TaskStatus.Verified;
        t.completedAt = uint64(block.timestamp);
        emit TaskVerified(taskId, t.rewardAmount);
    }

    function disputeTask(bytes32 taskId) external {
        Task storage t = tasks[taskId];
        require(t.status == TaskStatus.InProgress, "not disputable");
        t.status = TaskStatus.Disputed;
        emit TaskDisputed(taskId);
    }

    function settleTask(bytes32 taskId) external {
        Task storage t = tasks[taskId];
        require(t.status == TaskStatus.Verified || t.status == TaskStatus.Disputed, "not settleable");

        // 先记录是否 verified，再改状态
        bool verified = (t.status == TaskStatus.Verified);
        t.status = TaskStatus.Settled;

        uint256 payout = t.rewardAmount;
        if (verified) {
            payout += t.stakeAmount;
        }

        (bool success, ) = t.executor.call{value: payout}("");
        require(success, "transfer failed");
        totalStaked[t.executor] -= t.stakeAmount;
        emit TaskSettled(taskId, t.executor, payout);
    }
}
