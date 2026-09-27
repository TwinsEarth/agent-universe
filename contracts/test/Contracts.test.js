// GAP §9.4 逐缺陷回归测试：每个 GAP 记录的合约缺陷都有一个对应测试证明已修复。
const { expect } = require("chai");
const { ethers } = require("hardhat");

const id = (s) => ethers.keccak256(ethers.toUtf8Bytes(s));
const b32 = (s) => ethers.encodeBytes32String(s);

describe("GAP §9.4 — GovernorToken", function () {
  let token, owner, a;
  const SUPPLY = ethers.parseEther("1000000000");

  beforeEach(async () => {
    [owner, a] = await ethers.getSigners();
    token = await ethers.deployContract("GovernorToken");
    await token.waitForDeployment();
  });

  it("mints supply and self-delegates voting power", async () => {
    expect(await token.totalSupply()).to.equal(SUPPLY);
    expect(await token.getVotes(owner.address)).to.equal(SUPPLY);
  });

  it("delegate moves voting power (old delegateVotes only-subtracted and always reverted)", async () => {
    await token.transfer(a.address, ethers.parseEther("100"));
    expect(await token.getVotes(a.address)).to.equal(ethers.parseEther("100"));
    // owner 把自己的投票权委托给 a
    await token.connect(owner).delegate(a.address);
    expect(await token.getVotes(a.address)).to.equal(SUPPLY);
    expect(await token.getVotes(owner.address)).to.equal(0);
  });

  it("getPastVotes returns historical power via checkpoints", async () => {
    const before = await ethers.provider.getBlockNumber();
    await token.transfer(a.address, ethers.parseEther("100"));
    expect(await token.getPastVotes(owner.address, before)).to.equal(SUPPLY);
  });
});

describe("GAP §9.4 — AgentCardAnchor", function () {
  let anchor, owner, stranger;
  const cid = b32("cid-1");
  const did = b32("did-1");
  const wrongDid = b32("did-2");

  beforeEach(async () => {
    [owner, stranger] = await ethers.getSigners();
    anchor = await ethers.deployContract("AgentCardAnchor");
    await anchor.waitForDeployment();
  });

  it("rejects anchor from an unauthorized address", async () => {
    await expect(
      anchor.connect(stranger).anchor(cid, did)
    ).to.be.revertedWith("not authorized anchorer");
  });

  it("first anchor is immutable: re-anchoring the same cid reverts", async () => {
    await anchor.anchor(cid, did);
    await expect(anchor.anchor(cid, did)).to.be.revertedWith("anchor already exists; immutable");
  });

  it("verify checks BOTH cid and agentDidHash", async () => {
    await anchor.anchor(cid, did);
    expect(await anchor.verify(cid, did)).to.equal(true);
    expect(await anchor.verify(cid, wrongDid)).to.equal(false);
    expect(await anchor.verify(b32("other"), did)).to.equal(false);
  });
});

describe("GAP §9.4 — PoCVSettlement", function () {
  let C, owner, requester, executor, v1, v2, v3;
  const TID = id("task-1");
  const REWARD = ethers.parseEther("1");
  const STAKE = ethers.parseEther("0.2");

  beforeEach(async () => {
    [owner, requester, executor, v1, v2, v3] = await ethers.getSigners();
    C = await ethers.deployContract("PoCVSettlement");
    await C.waitForDeployment();
    for (const v of [v1, v2, v3]) {
      await C.addVerifier(v.address);
    }
  });

  async function fundAndAccept() {
    await C.connect(requester).createTask(TID, { value: REWARD });
    await C.connect(executor).acceptTask(TID, { value: STAKE });
  }

  it("createTask requires positive funding (zero-fund task reverts)", async () => {
    await expect(
      C.connect(requester).createTask(TID, { value: 0 })
    ).to.be.revertedWith("reward must be positive");
  });

  it("requester cannot self-execute", async () => {
    await C.connect(requester).createTask(TID, { value: REWARD });
    await expect(
      C.connect(requester).acceptTask(TID, { value: STAKE })
    ).to.be.revertedWith("requester cannot self-execute");
  });

  it("a non-verifier cannot cast verification", async () => {
    await fundAndAccept();
    await expect(
      C.connect(executor).castVerification(TID, true)
    ).to.be.revertedWith("not verifier");
  });

  it("requires a verifier majority to mark Verified", async () => {
    await fundAndAccept();
    await C.connect(v1).castVerification(TID, true);
    let t = await C.tasks(TID);
    expect(t.status).to.equal(2n); // InProgress
    await C.connect(v2).castVerification(TID, true);
    t = await C.tasks(TID);
    expect(t.status).to.equal(3n); // Verified
  });

  it("verified settlement lets executor claim reward + stake (pull, no address(0))", async () => {
    await fundAndAccept();
    await C.connect(v1).castVerification(TID, true);
    await C.connect(v2).castVerification(TID, true);
    await C.settleTask(TID);
    expect(await C.claimable(executor.address)).to.equal(REWARD + STAKE);
    expect(await C.claimable(requester.address)).to.equal(0);
    await C.connect(executor).withdraw();
    // pull 支付成功：可领余额清零（到账金额由内部转账保证，gas 不计入内部转账额）
    expect(await C.claimable(executor.address)).to.equal(0);
    expect(await ethers.provider.getBalance(C.target)).to.equal(0);
  });

  it("disputed settlement refunds reward to requester and slashes stake to treasury", async () => {
    await fundAndAccept();
    await C.connect(v1).castVerification(TID, false);
    await C.connect(v2).castVerification(TID, false);
    await C.settleTask(TID);
    expect(await C.claimable(requester.address)).to.equal(REWARD);
    expect(await C.claimable(owner.address)).to.equal(STAKE); // treasury == owner
    expect(await C.claimable(executor.address)).to.equal(0);
  });

  it("conservation: contract holds escrow before settlement and claimable after", async () => {
    await fundAndAccept();
    expect(await ethers.provider.getBalance(C.target)).to.equal(REWARD + STAKE);
    await C.connect(v1).castVerification(TID, true);
    await C.connect(v2).castVerification(TID, true);
    await C.settleTask(TID);
    expect(await ethers.provider.getBalance(C.target)).to.equal(REWARD + STAKE);
    await C.connect(executor).withdraw();
    expect(await ethers.provider.getBalance(C.target)).to.equal(0);
  });
});

describe("GAP §9.4 — ReputationRegistry", function () {
  let R, owner, stranger, v1, v2, v3;
  const agent = b32("agent-x");
  const EPOCH = 1n;

  beforeEach(async () => {
    [owner, stranger, v1, v2, v3] = await ethers.getSigners();
    R = await ethers.deployContract("ReputationRegistry");
    await R.waitForDeployment();
    for (const v of [v1, v2, v3]) await R.addVerifier(v.address);
  });

  it("only owner can add verifiers", async () => {
    await expect(R.connect(stranger).addVerifier(stranger.address)).to.be.revertedWith("not owner");
    expect(await R.verifierCount()).to.equal(3);
  });

  it("rejects scores above the bps range", async () => {
    await expect(
      R.connect(v1).recordReputation(agent, EPOCH, 10001, 9000, 9000, 9000)
    ).to.be.revertedWith("score out of bps range");
  });

  it("idempotent: identical resubmission is a no-op, conflicting one reverts", async () => {
    await R.connect(v1).recordReputation(agent, EPOCH, 8000, 9000, 9500, 7000);
    await R.connect(v1).recordReputation(agent, EPOCH, 8000, 9000, 9500, 7000); // no-op
    await expect(
      R.connect(v1).recordReputation(agent, EPOCH, 1, 9000, 9500, 7000)
    ).to.be.revertedWith("conflicting resubmission");
  });

  it("finalize requires quorum and takes the median", async () => {
    await R.connect(v1).recordReputation(agent, EPOCH, 8000, 9000, 9500, 7000);
    await expect(R.finalize(agent, EPOCH)).to.be.revertedWith("quorum not reached");
    await R.connect(v2).recordReputation(agent, EPOCH, 8200, 8800, 9600, 7200);
    await R.finalize(agent, EPOCH);
    const snap = await R.getLatestReputation(agent);
    expect(snap.quality).to.equal(8200); // median(8000,8200)
  });

  it("getLatestReputation returns a zero snapshot for an unknown agent (no revert)", async () => {
    const snap = await R.getLatestReputation(b32("nobody"));
    expect(snap.quality).to.equal(0);
    expect(snap.finalizedAt).to.equal(0);
  });
});
