// 部署全部合约（hardhat <network> run scripts/deploy.js）
const hre = require("hardhat");

async function main() {
  const names = ["GovernorToken", "AgentCardAnchor", "PoCVSettlement", "ReputationRegistry"];
  for (const name of names) {
    const c = await hre.ethers.deployContract(name);
    await c.waitForDeployment();
    console.log(`${name.padEnd(20)} -> ${await c.getAddress()}`);
  }
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
