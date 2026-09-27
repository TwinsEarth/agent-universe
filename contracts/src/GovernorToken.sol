// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title GovernorToken - GSN 治理代币（含委托投票与历史投票权检查点）
/// @notice 自包含 ERC20 + 委托投票；修复 GAP §9.4：
///         delegateVotes 此前只减不增（任何非零调用必 revert）、onlyOwner 从未应用、holders 无界。
contract GovernorToken {
    string public constant name = "Agent Universe Governance";
    string public constant symbol = "AGU";
    uint8 public constant decimals = 18;
    uint256 public totalSupply;

    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;

    struct Checkpoint {
        uint64 fromBlock;
        uint256 votes;
    }
    mapping(address => Checkpoint[]) private _checkpoints;
    mapping(address => address) public delegates;

    address public owner;

    event Transfer(address indexed from, address indexed to, uint256 value);
    event Approval(address indexed owner, address indexed spender, uint256 value);
    event DelegateChanged(address indexed delegator, address indexed fromDelegate, address indexed toDelegate);
    event DelegateVotesChanged(address indexed delegate, uint256 previousBalance, uint256 newBalance);

    modifier onlyOwner() {
        require(msg.sender == owner, "not owner");
        _;
    }

    constructor() {
        owner = msg.sender;
        _mint(msg.sender, 1_000_000_000 * 10 ** decimals);
    }

    function _mint(address to, uint256 amount) internal {
        totalSupply += amount;
        balanceOf[to] += amount;
        emit Transfer(address(0), to, amount);
        if (delegates[to] == address(0)) {
            delegates[to] = to;
            emit DelegateChanged(to, address(0), to);
        }
        _writeCheckpoint(delegates[to], _add, amount);
    }

    function transfer(address to, uint256 amount) external returns (bool) {
        _transfer(msg.sender, to, amount);
        return true;
    }

    function approve(address spender, uint256 amount) external returns (bool) {
        allowance[msg.sender][spender] = amount;
        emit Approval(msg.sender, spender, amount);
        return true;
    }

    function transferFrom(address from, address to, uint256 amount) external returns (bool) {
        require(allowance[from][msg.sender] >= amount, "insufficient allowance");
        allowance[from][msg.sender] -= amount;
        _transfer(from, to, amount);
        return true;
    }

    /// @notice 把调用者的投票权委托给 `to`，并按当前余额立即迁移投票权、写检查点。
    function delegate(address to) external {
        _delegate(msg.sender, to);
    }

    function _delegate(address delegator, address to) internal {
        address oldDelegate = delegates[delegator];
        if (oldDelegate == address(0)) oldDelegate = delegator;
        uint256 amount = balanceOf[delegator];
        delegates[delegator] = to;
        emit DelegateChanged(delegator, oldDelegate, to);
        _moveVotingPower(oldDelegate, to, amount);
    }

    function _transfer(address from, address to, uint256 amount) internal {
        require(to != address(0), "transfer to zero");
        require(balanceOf[from] >= amount, "insufficient balance");
        balanceOf[from] -= amount;
        balanceOf[to] += amount;
        emit Transfer(from, to, amount);

        if (delegates[from] == address(0)) {
            delegates[from] = from;
            emit DelegateChanged(from, address(0), from);
        }
        if (delegates[to] == address(0)) {
            delegates[to] = to;
            emit DelegateChanged(to, address(0), to);
        }
        _moveVotingPower(delegates[from], delegates[to], amount);
    }

    function _moveVotingPower(address src, address dst, uint256 amount) internal {
        if (src == dst || amount == 0) return;
        _writeCheckpoint(src, _subtract, amount);
        _writeCheckpoint(dst, _add, amount);
    }

    function _writeCheckpoint(address delegatee, function(uint256, uint256) view returns (uint256) op, uint256 delta) internal {
        Checkpoint[] storage ckpts = _checkpoints[delegatee];
        uint256 pos = ckpts.length;
        uint256 old = pos == 0 ? 0 : ckpts[pos - 1].votes;
        uint256 neu = op(old, delta);
        if (pos > 0 && ckpts[pos - 1].fromBlock == uint64(block.number)) {
            ckpts[pos - 1].votes = neu;
        } else {
            ckpts.push(Checkpoint({fromBlock: uint64(block.number), votes: neu}));
        }
        emit DelegateVotesChanged(delegatee, old, neu);
    }

    function _add(uint256 a, uint256 b) private pure returns (uint256) {
        return a + b;
    }

    function _subtract(uint256 a, uint256 b) private pure returns (uint256) {
        require(a >= b, "votes underflow");
        return a - b;
    }

    function numCheckpoints(address account) external view returns (uint256) {
        return _checkpoints[account].length;
    }

    /// @return 当前（最新检查点）投票权。
    function getVotes(address account) external view returns (uint256) {
        Checkpoint[] storage ckpts = _checkpoints[account];
        return ckpts.length == 0 ? 0 : ckpts[ckpts.length - 1].votes;
    }

    /// @notice 历史投票权（区块 `blockNumber` 时刻），二分查找检查点。
    function getPastVotes(address account, uint256 blockNumber) external view returns (uint256) {
        require(blockNumber < block.number, "block not yet mined");
        Checkpoint[] storage ckpts = _checkpoints[account];
        if (ckpts.length == 0) return 0;
        if (uint256(ckpts[0].fromBlock) > blockNumber) return 0;

        uint256 lo = 0;
        uint256 hi = ckpts.length - 1;
        while (lo < hi) {
            uint256 mid = lo + (hi - lo + 1) / 2;
            if (uint256(ckpts[mid].fromBlock) <= blockNumber) lo = mid;
            else hi = mid - 1;
        }
        return ckpts[lo].votes;
    }
}
