// SPDX-License-Identifier: MIT
pragma solidity ^0.8.20;

/// @title GovernorToken - GSN 治理代币
/// @notice 治理代币，用于投票和质押
contract GovernorToken {
    string public name = "Agent Universe Governance";
    string public symbol = "AGU";
    uint8 public constant decimals = 18;
    uint256 public totalSupply;

    mapping(address => uint256) public balanceOf;
    mapping(address => mapping(address => uint256)) public allowance;
    mapping(address => uint256) public votes;
    mapping(address => uint256) public delegatedVotes;

    address public owner;
    address[] public holders;

    event Transfer(address indexed from, address indexed to, uint256 value);
    event Approval(address indexed owner, address indexed spender, uint256 value);
    event DelegateVotes(address indexed delegator, address indexed delegate, uint256 amount);

    modifier onlyOwner() {
        require(msg.sender == owner, "not owner");
        _;
    }

    constructor() {
        owner = msg.sender;
        totalSupply = 1_000_000_000 * 10 ** decimals;
        balanceOf[msg.sender] = totalSupply;
        holders.push(msg.sender);
        emit Transfer(address(0), msg.sender, totalSupply);
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

    function delegateVotes(address delegate, uint256 amount) external {
        require(balanceOf[msg.sender] >= amount, "insufficient balance");
        votes[msg.sender] -= amount;
        delegatedVotes[delegate] += amount;
        emit DelegateVotes(msg.sender, delegate, amount);
    }

    function _transfer(address from, address to, uint256 amount) internal {
        require(balanceOf[from] >= amount, "insufficient balance");
        balanceOf[from] -= amount;
        balanceOf[to] += amount;
        if (balanceOf[to] == amount) {
            holders.push(to);
        }
        emit Transfer(from, to, amount);
    }
}
