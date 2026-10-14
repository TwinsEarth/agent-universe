# v6 引用尽调报告（Reference Due Diligence）

> 目的：在把《多 Agent 超大规模跨网络协作系统：架构设计与实施方案》（下称"蓝图"）转化为代码与正式文档前，逐一核验其引用的论文、协议、网站与量化数据，区分**可引用的真实出处**与**无法证实/张冠李戴的来源**，避免把"幻觉引用"与未经核验的数字写进仓库。
>
> 核验日期：2026-10-15（UTC+8）
> 核验方式：公开搜索 + 一手来源（arXiv / IETF / 项目官网）抓取核对
> 配套文档：[v6-vision-multi-agent-fleet.md](v6-vision-multi-agent-fleet.md)（蓝图原文）、[v5-memory-roadmap.md](v5-memory-roadmap.md)（v5 记忆路线与 v6 边界）

---

## 一、总判断

1. **蓝图的架构思想大多有真实研究支撑**——MMP 记忆协议、FadeMem 主动遗忘、EdgeCitadel 边缘编排、CID Broker 变更准入、可移植记忆压缩、A2A 身份生态等，均可作为 v6 的思想依据。
2. **但蓝图给出的具体链接约一半无法证实或张冠李戴**，集中在三个 Zenodo 记录（21923809 / 21783184 / 20684817）、PyPI `intermesh` 包以及 EdgeCitadel 的 ACM 链接。
3. **蓝图中的量化数据（20–40%、45%、37%、85–120ms、35–78%、135/3157/7502 等）均未能从一手来源确认**；pcell.si 的数字更是随时间持续变化的快照。**这些数字只能作为"待一手文献核验的参考值"，不得写成既定事实。**

---

## 二、核验结论总表

| # | 蓝图引用 | 蓝图给的链接 | 结论 | 核验到的正确出处 |
|---|---|---|---|---|
| 1 | MMP 记忆协议（P1/P2/P3、CAT7/SVAF/lineage/remix） | arXiv 2604.19540 | ✅ 准确 | arXiv 2604.19540 |
| 2 | CID Broker / 变更准入 | arXiv 2607.00041（蓝图称"Team Agents"） | ⚠️ 存在，标题张冠李戴 | arXiv 2607.00041（实际标题《ATM: CID-Brokered Pre-Write Admission …》） |
| 3 | EdgeCitadel（NATS–MQTT 混合编排） | ACM `dlnext.acm.org/...3814871`（带 `__cf_chl_tk`） | ⚠️ 描述准确，ACM 链接不可达 | arXiv 2606.14710 |
| 4 | FadeMem（主动遗忘） | Zenodo 21923809 | ⚠️ 概念存在，链接错误 | arXiv 2601.18642 |
| 5 | pcell.si（共识自动接受运行数据） | pcell.si | ⚠️ 网站真实，数字为动态快照 | pcell.si（须标注确切时间点） |
| 6 | A2A 协议 / 共识自动接受 | Zenodo 20684817 `a2a_protocol.pdf` | ⚠️ 协议生态真实，链接未命中 | 多个 IETF 2026 草案（见下） |
| 7 | Knowledge Compiler / 记忆压缩降 token | Zenodo 21923809 | ⚠️ 思想有真实对应，命名未证实 | arXiv 2605.11032 等 |
| 8 | 多 Agent 记忆综述（同步/访问控制/可扩展性） | Springer DOI `10.1007/978-981-92-1468-6_10` | ⚠️ 论断有支撑，该 DOI 未直接确认 | arXiv 2606.24535、2603.07670 等 |
| 9 | InterMesh 通用 Agent 协调协议 | PyPI `intermesh/0.4.1` | ❌ 查无匹配协议包 | 见下"未证实项" |
| 10 | AIX Protocol（.aix envelope / 9 维身份） | Zenodo 21923809 | ❌ 未找到该命名 | 相关但不同：AIP / ANX / Portable Agent Memory |
| 11 | AURA Council Protocol（七阶段/六角色） | Zenodo 21783184 | ❌ 未找到该记录 | 其他委员会论文（见下） |

图例：✅ 一手来源核对一致；⚠️ 部分成立但链接/命名/数字需修正；❌ 未能证实。

---

## 三、核验通过项（可引用）

### 1. MMP（Mesh Memory Protocol）✅

- 一手来源：《Mesh Memory Protocol: Semantic Infrastructure for Multi-Agent LLM Systems》，作者 Hongwei Xu，2026-04-21。
- 链接：<https://arxiv.org/pdf/2604.19540>
- 核对：P1 字段级接受、P2 来源可追溯（区分自身思考与回声）、P3 存储即相关；四个可组合原语 **CAT7（七字段认知记忆块）、SVAF（字段级评估）、inter-agent lineage（内容哈希键的父/祖先链）、remix（仅存储自身角色评估，不存储原始同伴信号）**——与蓝图描述一致。
- 用法：可作为 v5/v6 集体记忆与跨会话记忆块设计的直接依据。

### 2. CID Broker（变更准入平面）⚠️→概念可用

- 一手来源：arXiv 2607.00041，**实际标题为《ATM: CID-Brokered Pre-Write Admission for Multi-Agent Code Co-Synthesis》**，并非蓝图所称的"Team Agents"。
- 链接：<https://arxiv.org/pdf/2607.00041>
- 核对：CID Broker 作为跨任务的变更/写入准入仲裁者（mutation-admission plane）概念属实，是蓝图"协调"职责的核心。
- 用法：引用时必须使用正确标题 ATM，不得沿用"Team Agents"。

### 3. EdgeCitadel（边缘混合编排）⚠️→描述可用

- 一手来源：arXiv 2606.14710《Poster: EdgeCitadel — Hybrid NATS–MQTT Orchestration for Edge Multi-Agent Systems》。
- 链接：<https://arxiv.org/html/2606.14710>
- 核对：单个 NATS 2.10 服务器内置 MQTT 适配器、JetStream 持久化与重放、直接对等委派（direct peer delegation）、被动聚合器（passive aggregator）、ARM64/x64/Android 测试床——与蓝图描述一致。
- 说明：蓝图给的 ACM `dlnext` 链接含 `__cf_chl_tk` 反爬挑战参数、无法确认；**以 arXiv 为准**。

### 4. FadeMem（主动遗忘）⚠️→概念可用

- 一手来源：arXiv 2601.18642《FadeMem: Biologically-Inspired Forgetting for Efficient Agent Memory》（双层记忆、自适应指数衰减）。
- 链接：<https://arxiv.org/html/2601.18642v1>
- 核对：生物启发的主动遗忘机制存在，多跳推理/检索表现与存储效率是其卖点。
- 说明：蓝图给的 Zenodo 21923809 无法访问；**"存储减少 45%"未在摘要中确认**，引用前须读全文。

### 5. 可移植记忆压缩（对应 Knowledge Compiler 思想）⚠️

- 一手来源：arXiv 2605.11032《Portable Agent Memory: Provenance-Verified Memory Transfer》。
- 链接：<https://arxiv.org/pdf/2605.11032.pdf>
- 核对：`token_budget`（让接收方在加载前规划上下文）、按相关性分级（高相关保留原文 / 中等相关摘要 / 低相关丢弃）、来源校验——与蓝图"Knowledge Compiler"思想高度吻合。
- 说明：**"Knowledge Compiler"命名与"35–78% 降 token"未获证实**；引用时使用 Portable Agent Memory 并到全文核对数字。

### 6. A2A 身份与协议生态 ⚠️

生态真实存在，2026 年有多份 IETF 草案：

- <https://www.ietf.org/archive/id/draft-chapman-a2a-mls-03.txt>：Ed25519 身份与加密绑定。
- <https://www.ietf.org/archive/id/draft-fane-opena2a-aip-00.txt>：OpenA2A Agent Identity Protocol，行为信任 + Ed25519 / ML-DSA-65 + Merkle 透明日志。
- <https://www.ietf.org/archive/id/draft-laxsharma-pact-00.txt>：引用 "Agent2Agent (A2A) Protocol Specification, v1.0"。

说明：蓝图给的 Zenodo 20684817 未直接命中；"共识自动接受""Sentinel + MetaSentinel 递归治理"的思想与上述草案及 A2A 生态相容，但具体出处须重新定位。

### 7. 多 Agent 记忆研究领域 ⚠️

蓝图"记忆在多 Agent 系统中是独特前沿、面临同步/访问控制/可扩展性/对齐/安全"的论断有大量真实 2026 文献支撑（蓝图给的具体 Springer DOI 未直接确认）：

- arXiv 2606.24535《Governed Shared Memory for Multi-Agent LLM Systems》（MemClaw；四种失败模式：未授权泄漏、陈旧传播、矛盾持久化、来源坍缩）：<https://arxiv.org/html/2606.24535>
- arXiv 2603.07670《Memory for Autonomous LLM Agents》：<https://arxiv.org/pdf/2603.07670>
- TechRxiv《Memory in LLM-based Multi-agent Systems》
- arXiv 2603.15183《Token Coherence: Adapting MESI Cache Protocols …》

---

## 四、未能证实项（不得当作事实引用）

1. **InterMesh 通用 Agent 协调协议**（蓝图链接 `pypi.org/project/intermesh/0.4.1`）
   - 该 PyPI 页面直接访问遇到 Client Challenge；公开搜索未找到蓝图所述"通用开源 Agent 协调协议（SHA-256 指纹身份 / JWT hub 令牌 / RBAC / 联邦）"的 PyPI 包。
   - 名称相近但实质不同的对象：`intermesh.network`（"个人网络 OS"，用 MCP 管理联系人）、Group-Office 的 Intermesh（企业 CRM）、IETF Mathematical Mesh（2017 年 PKI）。
   - **结论：v6 立项前须重新定位该协议的一手来源；在此之前不写进正式文档/代码。**

2. **AIX Protocol**（蓝图链接 Zenodo 21923809；".aix envelope / 9 维身份快照 / Nexus / 隐私层"）
   - 未找到该命名与对应规范。相关但不同：arXiv 2509.13597（agent 身份）、AIP 草案、ANX（arXiv 2604.04820）。

3. **AURA Council Protocol**（蓝图链接 Zenodo 21783184；六角色委员会 / 七阶段决策）
   - 该 Zenodo 记录抓取失败、公开搜索无对应。
   - 委员会/治理方向真实存在的其他论文：AgentCity（arXiv 2604.07007，六阶段立法）、MADA（Zenodo 20781885，三角色 Risk/Legal/Strategist，Borda/二次/加权投票）、COA-MAS（Zenodo 19057202）、Deliberative Curation（Zenodo 20113137）。

---

## 五、未获确认的量化数据（清单）

以下数字在蓝图中被当作事实陈述，但本次核验**未能从一手来源确认**；在读到原文并核对口径前，一律标注为"待核验"：

| 数字 | 蓝图语境 | 状态 |
|---|---|---|
| 异构 Agent 收敛快 20–40% | 编队 Leader-Follower | 未核验 |
| FadeMem 存储减少 45% | 主动遗忘 | 摘要未确认 |
| ZenBrain 睡眠巩固稳定性 +37% | 记忆巩固 | 未核验 |
| 协同崩溃决策延迟 85–120ms、相邻特征 65% 相似、缓存命中 35–42% | 大规模扩展 | 未核验 |
| Knowledge Compiler 降 35–78% token | 记忆压缩 | 未核验 |
| pcell.si：135 Agent / 3157 声明 / 7502 验证 | 共识自动接受 | 早期快照，非当前值 |

**pcell.si 数据随时间变化（特别说明）**：

- 2026-06-14 BornCity 报道：545 个 Agent、2578 笔记、20655 注释、6377 知识声明、约 17000 同行评审。
- 2026-10-15 首页实际显示：901 个 Agent、510 个领域、128412 条知识声明。
- 蓝图引用的 135/3157/7502 疑为上线初期（2026-05）快照。
- **引用规则**：任何 pcell.si 数字都必须同时标注**抓取日期与页面口径**，不得作为稳定的当前事实。

---

## 六、对 v6 立项的建议

1. **思想可借鉴，链接须换源**：正式文档/代码中的引用改用本报告核验到的一手来源（以 arXiv / IETF 为主），不照搬蓝图的 Zenodo / PyPI 链接。
2. **数字先核后写**：所有量化数据先读一手论文确认，并写明测量口径、规模与时间点；无法确认则保留"待核验"。
3. **"来源可验证"应成为 v6 记忆/治理的内生能力**：本次核验暴露的问题——Agent 可能产生无法验证或张冠李戴的引用——与 MMP 的 P2（来源可追溯）、蓝图的 lineage、以及 v5.0.0 审核 Agent 的职责一致。建议在 v6 集体记忆中为每条声明/引用强制携带可验证的来源句柄（内容哈希 + 一手链接 + 抓取时间），由审核 Agent 校验，**从机制上杜绝"幻觉引用"**。
4. **范围纪律**：本报告确认的所有编队/委员会/协防/数值权威/多协议/大规模扩展能力，**均归入 v6.0.0+**；v5 系列（v5.0.1–v5.2.0）只做记忆系统深化，不提前实现上述模块。

---

## 附：核验所依据的一手来源（URL 原样保留）

- <https://arxiv.org/pdf/2604.19540>（MMP）
- <https://arxiv.org/pdf/2607.00041>（ATM / CID Broker）
- <https://arxiv.org/html/2606.14710>（EdgeCitadel）
- <https://arxiv.org/html/2601.18642v1>（FadeMem）
- <https://arxiv.org/pdf/2605.11032.pdf>（Portable Agent Memory）
- <https://www.ietf.org/archive/id/draft-chapman-a2a-mls-03.txt>
- <https://www.ietf.org/archive/id/draft-fane-opena2a-aip-00.txt>
- <https://www.ietf.org/archive/id/draft-laxsharma-pact-00.txt>
- <https://arxiv.org/html/2606.24535>（Governed Shared Memory / MemClaw）
- <https://arxiv.org/pdf/2603.07670>（Memory for Autonomous LLM Agents）
- <https://pcell.si/>（2026-10-15 抓取）
- <https://borncity.com/news/545-autonome-ki-agenten-pcell-si-schreibt-wirtschaft-neu/>
