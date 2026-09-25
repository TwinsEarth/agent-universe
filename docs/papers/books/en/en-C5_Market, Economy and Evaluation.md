# Market, Economy and Evaluation

> 主题册五·市场经济与评估 · TwinsEarth · 2026-09-24 · Papers released under CC BY 4.0

---

<p align="center"><img src="assets/logo.png" width="180" alt="TwinsEarth"/></p>

# Settlement Conservation in an Internal Task Market: Mechanism Design for Denying Duplicate Labor and Slashing Byzantine Agents

> Evidence grade: all figures come from CPU deterministic simulation (`reports7/matrix_scale_demo.json`, evidence_grade=cpu-proto; unit-test figures come from `tests7/test_v749_market.py` and the `market.py` code, verified), as **single-seed pilot point estimates**; the full-factorial simulation of strategy family × slashing rate × acceptance hit rate (≥1000 rounds/cell) has not been run, and the relevant positions keep `[RESULT NEEDED]`.
> Template-comparison positioning: this paper is the Stage B upgrade draft of the "UDOS Writing and Typesetting Specification v2." The empirical chapter unfolds as "experimental setup → comparison (unit-test ledger vs matrix clean/stressed) → sensitivity (analytical thresholds of slashing rate/acceptance hit rate) → validity threats"; conservation invariants Eqs.(1)–(3) are centrally numbered, and the clean/stressed two-level comparison table follows the NSA template's "same-budget two-level comparison" form.

---

## Abstract (Structured, Four Parts)

**[Background & Problem]** As multi-agent systems scale from dozens to millions of agents, task allocation and contribution settlement move from an engineering detail to a governance problem. Without explicit settlement rules, resource consumption is a "dark ledger"; the first requirement of a ledger is not "optimal incentives" but "the books balance."

**[Method]** This paper reports the internal task market of UDOS v7.4.9 (`udos7/topology/market.py`): a settlement mechanism combining a quality floor, a value-for-money award rule, payment only to the unique accepted completer, and slashing of Byzantine results. We write the settlement rule as a priority short-circuit and formalize "balanced books" as a conservation invariant enforced under floating-point tolerance.

**[Evidence & Results]** The core invariant is `balance_sum = total_paid − slashed` with `total_paid ≤ total_budget`. A mixed 3-task unit test measures: budget 24, paid 16, slashed 1, balance sum 15 (=16−1), holds 8, `conserved=true`; the end-to-end matrix integration (24 orders, 3 faulty nodes) is `conserved=true` in both clean and stressed runs, 24/24 settled by acceptance, retries produce no duplicate payment. Ten unit tests cover award, floor, load tie-break, unique payment, duplicate denial, slashing, conservation, and never-exceed-budget.

**[Contribution]** (1) Conservation is a theorem (guaranteed by settlement code structure), whereas incentive compatibility is an empirical approximation (depends on acceptance hit rate); (2) we provide a measured "settlement rule × behavior type → conservation/incentive" matrix; (3) we honestly scope it as internal accounting units with no real pricing game, no Sybil defense, and no financialization claim.

**Keywords**: mechanism design; task market; settlement conservation; Byzantine fault tolerance; multi-agent systems; incentive compatibility

---

## 1 Introduction

Multi-agent orchestration is moving from "a few agents collaborating" toward "hundreds, thousands, even million-agent matrices." When tasks flow among agents as work orders, three engineering problems immediately arise: who is qualified to take the task? By what rule is it awarded? After completion, on what basis is money paid, and how much? If these rules are implicit and rely on human coordination, once the system scales three things happen: **duplicate labor** (two agents do the same thing, resources wasted), **low-quality bid rushing** (the cheapest but laziest bidder always wins), and **false reporting** (submitting a result that is not qualified at all but demanding payment).

Why does this matter? Because in an agent system without explicit settlement rules, resource consumption is a "dark ledger": you do not know who did duplicate work, who took payment they should not have, or whether the total budget was spent or wasted. When the system has only 10 agents, human coordination can hold up; at 1000, 1 million agents, there is no governance without a ledger. The first requirement of a ledger is not "optimal incentives" but "the books balance" — money is neither created nor destroyed out of thin air, and every payment traces to an accepted work order. This paper formalizes this most basic engineering requirement as a conservation invariant and proves it always holds in code structure.

This paper faces precisely this settlement-governance problem. UDOS Reasoning Engine v7.4.9's `udos7/topology/market.py` gives a minimal but self-consistent internal market: tasks are first priced (`deposit`), agents bid (`Bid`: cost, self-assessed quality, load), the auction awards by "quality floor + value-for-money + load tie-break" (`TaskAuction.award`), after completion QA accepts (`settle(accepted=...)`), and only the unique accepted completer gets money; duplicate submissions, acceptance failures, and Byzantine results are all unpaid, and Byzantine results are additionally deducted from the deposit (`slash`). The whole ledger satisfies a conservation invariant.

Taking this mechanism apart, it in fact answers four plain questions. **Who can take?** — quality floor $q_i\ge f$. **Given to whom?** — among the qualified, the one with the highest value-for-money $q_i/c_i$ and lowest load. **On what basis paid after completion?** — paid only when QA accepts. **How much, how deducted?** — full reward given, duplicate/rejected/Byzantine given 0, Byzantine additionally deducted from deposit. These four questions sound like common sense, but writing them as code, as tests, as conservation assertions is engineering. All contributions of this paper are turning these four questions from "common sense" into "executable, testable, balanced-books" rules.

The boundary must first be declared. This is a market of **internal accounting units**, not a real cryptocurrency economy: no real pricing game, no Sybil defense, no cross-account encrypted settlement, and no linkage to any token/financialization. The question it answers is "inside a closed, auditable agent system, how to use accounting rules so resources are neither created nor destroyed out of thin air," not "how to design an open, Sybil-resistant economic protocol." Any industry narrative involving external "token economies" or "agent currencies" is an *unverified* reporting scope, and this paper does not cross-verify with it.

Specific contributions of this paper:

1. **Rules and invariants**: write the payment rule in mechanism-design language and prove conservation is a theorem (guaranteed by settlement code structure), whereas incentive compatibility is an empirical approximation (depends on acceptance hit rate).
2. **Reproducible tests**: 10 unit tests + matrix end-to-end integration, giving a measured "settlement rule × behavior type → conservation/incentive" matrix.
3. **Honest boundaries**: clarify what is theorem (conservation) and what is empirical (IC depends on acceptance hit rate), not inflating the engineering prototype into a mechanism-design theory contribution.

Of these three contributions, the third may be the most underrated. At a time when agent-system narratives are generally inflated, a paper honestly marking "this is internal accounting units, no Sybil defense, IC is only an empirical approximation" has more long-term value than a white paper claiming a "decentralized autonomous economy" — because the former knows its boundaries and the latter does not.

Choosing "conservation" rather than "optimal auction" as this paper's protagonist is a deliberate value orientation. In mechanism-design textbooks, VCG's elegant "truth-telling is optimal" conclusion is attractive, but engineering it needs public preference aggregation, the curse of dimensionality, and the computational cost of strategy-proofness; whereas a 300-line `market.py`, with a priority short-circuit and a conservation invariant, has solidly done the most basic thing of "money does not increase out of thin air." For a million-agent system, "balanced books" is more urgent than "optimal auction" — if the books do not balance, even the best auction is built on quicksand.

**Boundary statement**: all figures come from CPU deterministic simulation (`reports7/matrix_scale_demo.json`, evidence_grade=cpu-proto; unit-test figures come from `tests7/test_v749_market.py` and the `market.py` code), as **single-seed pilot point estimates**; the full-factorial simulation of strategy family × slashing rate × acceptance hit rate (≥1000 rounds/cell) has not been run, and the relevant positions keep `[RESULT NEEDED]`.

## 2 Related Work

Task allocation and auctions have mature lines in multi-agent systems, distributed systems, and mechanism design. **Conceptually**, distributed task allocation was first formalized by the Contract Net Protocol: nodes contract tasks to the most capable through the three-stage "request for bids—bid—award" negotiation (Smith, 1980), which is precisely the historical source of UDOS's `TaskAuction` bidding/award. **Mechanically**, Vickrey showed the second-price sealed auction makes truth-telling a dominant strategy (Vickrey, 1961), and its generalization VCG simultaneously achieves incentive compatibility and Pareto efficiency under ideal assumptions; but Ausubel and Milgrom in their survey soberly point out that VCG is "theoretically elegant, practically lonely" — it is computationally hard, easily colluded, and does not satisfy budget balance, so it is rarely deployed at scale in real market design (Ausubel & Milgrom, 2006). Modern market design further extends the view to combinatorial auctions and procurement scenarios, discussing how award rules for "buyers buying services from multiple sellers" in electricity, spectrum, and procurement work under real constraints (Milgrom, 2019). **On evidence**, Byzantine fault tolerance on the distributed-systems side has a mature state-machine replication tradition: PBFT tolerates $f<n/3$ Byzantine nodes under asynchronous networks, with implementation overhead only about 3% more than an unreplicated system (Castro & Liskov, 1999); and "slashing" as a Byzantine economic penalty has extensive separate discussion in blockchain/staking literature [CITATION NEEDED: slashing Proof of Stake]. **Difference from UDOS**: the above works are mostly at the open economy or consensus layer and do not solve "how the settlement ledger balances in a closed internal market"; Sybil resistance in the open economy is even harder [CITATION NEEDED: Sybil attack Douceur].

Closest to this paper is the branch of "multi-agent task allocation with payment" [CITATION NEEDED: multi-agent task allocation payment; market-based multi-robot task allocation; peer prediction mechanism proper scoring]. Such works usually use auctions to allocate tasks to the lowest-cost or highest-utility agents. Our difference is not in auction theory but in the **engineering conservation of the settlement ledger**: most auction works care about "who wins," and this paper cares about "how to pay after winning, how to ensure money is neither added nor lost out of thin air, and how to make duplicate labor and false reporting unprofitable on the ledger." In other words, we step back from VCG's "truth-telling dominance" and replace it with the engineering-auditable "pay only on acceptance, pay only the unique completer, Byzantine must be slashed" — a pragmatic response to the VCG practical defects Ausubel–Milgrom pointed out.

Our positioning is **mechanism engineering + simulation reference implementation**, not claiming VCG-style theoretical optimality. The reason: in a closed internal market, fully IC complex mechanisms (VCG needs public preference aggregation and is computationally expensive) are not worthwhile; a simple "quality floor + value-for-money + acceptance payment + slashing" is engineering-auditable and testable, already sufficient to suppress duplicate labor and false reporting. When theoretical depth is insufficient, we honestly position it as an empirical paper.

This paper must also be clearly separated from popular narratives like "token economies/agent currencies." The latter often advocates "issuing coins to agents and letting the market self-regulate," but that is an open-economy problem needing Sybil defense, price discovery, and external trust anchors, far beyond this paper's scope. Between these two there is a gap this paper fills: **most auction works care about "who wins," and few write "how to settle after winning, how to ensure conservation, and how to make duplicate labor and false reporting unprofitable" as a testable-invariant engineering reference implementation.** Academia has beautiful theorems for VCG's incentive compatibility but insufficient attention to the engineering problem of "a 300-line internal ledger covered by pytest with balanced books." This paper does not pursue VCG-style elegance but provides a settlement reference implementation directly readable, runnable, and extensible, letting later work do full-factorial strategy simulation on it rather than writing a ledger from scratch.

It must be admitted that the cost of this engineering orientation is limited theoretical depth: we do not prove our auction is optimal, do not prove our slashing mechanism is collusion-resistant, and do not prove IC holds over a general utility domain. All conclusions of this paper are bounded within the specific engineering configuration of "internal accounting units, oracle acceptance, single-thread sequential settlement." Treating it as a mechanism-design theory contribution is a misreading; treating it as a reproducible engineering benchmark is this paper's positioning.

## 3 Problem Definition and Assumptions

### 3.1 Notation and rules

A task $t$ has budget $B_t$ (injected by `deposit`). Agent $i$ bids $(c_i, q_i, \ell_i)$: cost $c_i$, self-assessed quality $q_i\in[0,1]$, load $\ell_i$. Auction floor $f$. Award rule:

$$
\text{eligible}=\{i: q_i\ge f\},\quad \text{winner}=\arg\max_{i\in\text{eligible}}\left(\frac{q_i}{\max(c_i,10^{-9})},\ -\ell_i\right). \tag{1}
$$

I.e. first pass the quality floor, then award by value-for-money $q/c$, with load as the order tie-break (same value-for-money choose the lower load).

The settlement rule (`settle`) is a priority short-circuit:

1. If the task has already been paid (`task_id in paid_tasks`) → pay 0, reason `already_paid`;
2. If marked duplicate labor (`duplicate=True`) → pay 0, reason `duplicate_work`;
3. If acceptance fails (`accepted=False`) → pay 0, reason `rejected`, and deduct $s$ from that agent's balance (slashing);
4. Otherwise (accepted, not paid, not duplicate) → pay $B_t$ to that agent and record in `paid_tasks`.

### 3.2 Falsifiable assumptions

- **H1 (duplicate-labor payment is 0)**: the unique-payment constraint makes any duplicate/repeated submission's payment 0. *Falsifiable*: either `test_only_accepted_unique_work_paid` / `test_duplicate_work_flag_not_paid` fails.
- **H2 (slashing makes Byzantine expected return negative)**: under a given slashing rate $s$ and acceptance hit rate $p$, the Byzantine strategy's expected return <0. *Falsifiable*: when $s$ is small enough and acceptance hit rate high enough, Byzantine expected return ≥0.
- **H3 (quality floor prevents low-price low-quality)**: the floor $f$ filters out bids with $q_i<f$. *Falsifiable*: `test_quality_floor_filters_cheap_low_quality` fails.
- **H4 (budget conservation)**: `total_paid ≤ total_budget` and `balance_sum = total_paid − slashed`. *Falsifiable*: `conservation_check().conserved` is False.

There is also an implicit assumption worth making explicit: **H5 (acceptance is an oracle)**. Current QA judges "whether the result is qualified" as a known Boolean, with acceptance hit rate $p=1$. This means this paper's IC conclusions hold under the ideal acceptance of "false reporting is always caught"; once acceptance has misses, H2 must be recomputed by the formula in Section 4.5. We do not hide this assumption but list it as H5, letting readers see at a glance the conditions under which IC holds.

## 4 Method and System Design

### 4.1 The auction: value-for-money award

The eligible filtering and value-for-money sorting of `TaskAuction.award` are two lines of code, but the design intent is worth clarifying. Why use $q/c$ rather than pure lowest cost? Because pure lowest cost incentivizes "low-price low-quality" — whoever bids lowest wins, and quality naturally degrades. Why add the quality floor $f$ rather than pure value-for-money? Because value-for-money $q/c$ can still be high when $q$ is very small ($c$ extremely small), and the floor first keeps out "not qualified at all" bidders, then selects the best value-for-money among the qualified. Load $\ell_i$ is the order tie-break: when two bids have the same value-for-money, choose the currently lower-loaded one, avoiding pressing all tasks onto the same agent.

In Table 2 unit tests, `test_auction_picks_best_quality_cost_ratio` directly demonstrates this: three bidders a(cost=8,q=0.8), b(cost=4,q=0.8), c(cost=3,q=0.2). a and b have the same quality 0.8, b has lower cost, so b's value-for-money beats a; c has the lowest cost but quality 0.2 is treated as low quality. Finally b wins. This shows $q/c$ simultaneously penalizes "too expensive" and "too poor," rather than only looking at price.

### 4.2 The settlement ledger: priority short-circuit

The priority short-circuit of `ContributionLedger.settle` is the core of this mechanism. It first asks "has this task been paid," then "is it duplicate labor," then "did acceptance pass," and finally pays. Each short-circuit layer corresponds to a defense against one strategic behavior:

- **Already-paid check** prevents "a task submitted by two agents, paid twice";
- **Duplicate marking** prevents "the same agent repeatedly submitting the same thing to inflate workload";
- **Acceptance check** prevents "getting money for an unqualified result";
- **Slashing** additionally deducts deposit on acceptance failure, raising the expected cost of false reporting.

### 4.3 The conservation invariant

`conservation_check()` computes:

$$
\text{total\_budget}=\sum_t B_t,\qquad \text{balance\_sum}=\sum_i \text{bal}_i, \tag{2}
$$

$$
\text{holds}=\text{total\_budget}-\text{total\_paid}, \tag{3}
$$

$$
\text{conserved} = \left(\text{total\_paid}\le \text{total\_budget}+\varepsilon\right)\ \wedge\ \left|\text{balance\_sum}-(\text{total\_paid}-\text{slashed})\right|<\varepsilon,\qquad \varepsilon=10^{-9}. \tag{4}
$$

Where $\varepsilon=10^{-9}$ tolerates floating-point error. This invariant is a **theorem, not empirical**: because `settle` only transfers $B_t$ from the budget pool into some agent's balance when acceptance passes (`total_paid += B_t, balances[agent] += B_t`), and slashing only deducts from balance (`balances[agent] -= s, slashed += s`), with no code path creating balance out of thin air, the ledger identity always holds arithmetically. What really needs testing is "whether the rules trigger as designed," not "whether the arithmetic conserves."

Worth expanding is "why conservation deserves a dedicated proof." In engineering practice, the most common form of ledger bug is not "money computed wrong" but "money whereabouts unknown": a payment made but not recorded, a slash deducting balance but not recording slashed, a task paid twice. These bugs in small systems are found by human reconciliation but in million-scale systems are dark ledgers. Writing `balance_sum = total_paid − slashed` as one executable `conservation_check()` and calling it once at the end of each integration test automates "reconciliation" into a CI gate: any payment with unknown whereabouts immediately makes `conserved` False. This practice of "turning the accounting identity into a test assertion" is much cheaper than post-hoc auditing.

### 4.4 The boundary between theorem and empirical

Two kinds of conclusions must be strictly separated. **Conservation is a theorem**: as long as `settle` is not bypassed and no floating-point drift appears, `conserved` is always true. **Incentive compatibility (IC) is an empirical approximation**: whether a Byzantine strategy has negative return depends on the combination of "acceptance hit rate $p$" and "slashing rate $s$" — if acceptance is too loose ($p$ low) or slashing too light ($s$ small), Byzantine expected return may turn positive. This paper currently tests only one fixed configuration ($s=1.0$, strict acceptance), and the full-factorial threshold grid is to be supplemented ([RESULT NEEDED]).

### 4.5 Conditions for individual rationality and incentive compatibility

Putting this rule set into mechanism-design language can more precisely state what it promises and does not promise.

**Individual rationality (IR)**: an honest agent's expected utility of participating in the market is non-negative. Because payment is made only on acceptance and is the task's full reward, the honest completer's net return is positive (reward minus true cost, worth doing as long as reward>cost); not participating gives 0. Therefore under the task-pricing premise of "reward≥true cost," honest agents have motive to participate. But this relies on task pricing itself being reasonable — if reward is set below true cost, even honest agents will not take it, and IR is broken. This paper assumes reasonable pricing as an external premise and does not solve it within the market mechanism.

**Incentive compatibility (IC)**: is honest bidding better than false reporting? In this mechanism, IC is **partial and empirical**. Duplicate labor is blocked by the unique-payment constraint (no matter how repeated, the second and later submissions are paid 0), and this part of IC is structural. But whether "low-quality bid rushing" is suppressed depends on the quality floor $f$ and acceptance hit rate $p$: if self-assessed quality $q_i$ can be falsely reported (agent self-assesses 0.9 but actually delivers 0.3), the floor is bypassed and only acceptance can backstop; the lower the acceptance hit rate $p$, the higher the probability low-quality deliveries slip through. Byzantine slashing $s$ further raises the expected cost of false reporting, but $s$ must be greater than "the expected return of false reporting passing acceptance" to have deterrence.

The Byzantine expected utility can be written as:

$$
\mathbb{E}[u_{\text{byz}}]=(1-p)\,R - p\,s, \tag{5}
$$

Where $R$ is the illegal return when false reporting slips acceptance, $p$ is the acceptance hit rate, and $s$ is slashing. When $(1-p)R < p s$, i.e. $s > R(1-p)/p$, the Byzantine expected return is negative. This is the analytical form of the "slashing-rate break-even point" and the threshold the [RESULT NEEDED] full-factorial simulation verifies. This paper does not hand-compute this threshold without measured $R$, $p$.

## 5 Experimental Setup

Per the v2 specification, the empirical chapter unfolds as "experimental setup → comparison (unit-test ledger vs matrix clean/stressed) → sensitivity (analytical thresholds of slashing rate/acceptance hit rate) → validity threats (Chapter 8)."

### 5.1 Unit-test protocol

`tests7/test_v749_market.py` contains 10 tests, all deterministic, CPU, no randomness:

1. Value-for-money award (same-quality low-cost wins);
2. Quality floor filters low-price low-quality;
3. No eligible bidder, no winner;
4. Load breaks ties;
5. Only the unique accepted completer paid (duplicate completion unpaid);
6. Duplicate-labor marking unpaid;
7. Byzantine result unpaid and slashable;
8. Conservation invariant (mixed 3 tasks);
9. Payment never exceeds budget;
10. Rejected work recorded.

### 5.2 End-to-end integration

`scripts7/matrix_demo.py` runs the agent matrix of 24 work orders, in two levels clean (no faults) and stressed (3 faulty nodes injected), with market settlement called by `udos7/topology/matrix.py`. Outputs `reports7/matrix_scale_demo.json` (evidence_grade=cpu-proto). Hardware: CPU, deterministic simulation, agents not real LLMs.

It must be noted that the market budget in matrix integration is 240 (24 work orders × 10/order), while the unit-test mixed 3-task budget is 24 (T1=10+T2=8+T3=6). The two are not the same set of figures: the unit test is the minimal reproducible conservation demo, and matrix integration is the 24-order scale demo. Both clean/stressed levels pay out all 240 (total_paid=240) because all work orders are ultimately accepted; the unit test deliberately keeps an acceptance-failing T2 so holds=8>0, demonstrating the non-trivial case of "unpaid budget on hold." If all work orders smoothly accept, holds is always 0 and conservation degenerates to total_paid=total_budget — this trivial case cannot test the holds field, so the unit test specifically constructs T2 failure.

## 6 Results

### 6.1 Unit-test conservation: mixed 3 tasks

Table 1 reproduces the ledger of `test_conservation_invariant`. Three tasks: T1=10 (paid to a), T2=8 (acceptance rejected, slash 1.0), T3=6 (paid to c).

**Table 1 Mixed 3-task unit-test ledger (verified, test_conservation_invariant)**

| Quantity | Value | Note |
|---|---|---|
| total_budget | 24 | T1+T2+T3 |
| total_paid | 16 | T1(10)+T3(6) |
| slashed | 1 | T2 slash |
| holds | 8 | T2 unpaid budget still in pool |
| balance_sum | 15 | =16 paid −1 slashed |
| conserved | true | invariant holds |

*Table 1 note: evidence grade verified (unit-test hardcoded assertion). Both sides of Eq.(4) hold: total_paid=16≤total_budget=24+ε; balance_sum=15=(total_paid 16)−(slashed 1), error <ε. holds=8 is Eq.(3), the T2 unpaid budget on hold.*

`balance_sum=15` exactly equals `total_paid(16) − slashed(1)`, verifying the conservation invariant. T2 acceptance failed and was unpaid, and its 8 budget stays in `holds` (the pool), neither disappearing nor being transferred to others out of thin air. Figure 1 places this ledger alongside the matrix clean/stressed levels.

Here the accounting meaning of `holds=8` needs explanation. Total budget 24 = paid 16 + unpaid holds 8. The unpaid 8 is the budget for T2 rejected by acceptance; it was neither paid to a/b/c (which would cause overspend or mispayment) nor disappeared from the system (which would break conservation), but is "on hold" in the pool awaiting reallocation — which is precisely the role of the `holds` field: tracking budget "still pending, not yet transferred to anyone." This design keeps the ledger always closed: total budget issued = paid to agents + slashed back + still in pool. At any moment auditing, `total_budget` can be split into these three items, with no "dark ledger."

![Figure 1: Three-ledger conservation comparison](figures/P11_fig1_conservation.png)

*Figure 1 Three-ledger conservation comparison. Left column: unit-test mixed 3 tasks (budget 24 = paid 16 + slashed 1 + holds 8; balance_sum 15); middle column: matrix clean level (budget 240 = paid 240 + holds 0); right column: matrix stressed level (budget 240 = paid 240 + holds 0). Each level is `conserved=true`. Evidence grade: unit test verified, matrix cpu-proto. Data source `reports7/matrix_scale_demo.json` + `tests7/test_v749_market.py`.*

### 6.2 Settlement rule × behavior type

Table 2 (Figure 2) is the decision matrix of "settlement rule × agent behavior → payment result." This is the direct correspondence of H1/H2/H3.

**Table 2 Settlement rule × behavior type (verified, from market.py settle logic)**

| Agent behavior | Paid? | Reason | Slashed? |
|---|---|---|---|
| Honest submission, accepted | Full reward | accepted | No |
| Repeated submission of same task | 0 | already_paid / duplicate_work | No |
| Low-quality submission, rejected | 0 | rejected | Optional (slash>0) |
| Byzantine result, rejected | 0 | rejected | Yes (deduct deposit) |
| No eligible bidder | No winner | — | — |

*Table 2 note: evidence grade verified (`settle` priority short-circuit logic + 10 unit tests). Reading: row by row "which short-circuit this behavior falls into → how much paid → whether slashed," i.e. the ledger result of each strategic behavior. The strict conclusion that Byzantine "expected return is negative" needs full-factorial simulation (Eq.(5)), currently `[RESULT NEEDED]`.*

![Figure 2: Settlement rule × behavior type](figures/P11_fig2_rule_behavior.png)

*Figure 2 Settlement rule × behavior type decision matrix. The horizontal axis is agent behavior (honest/duplicate/low-quality/Byzantine/no eligible bid), and the vertical axis is the settlement short-circuit level (already-paid check → duplicate marking → acceptance check → payment), converging to payment result (full/0/no winner) and whether slashed. Evidence grade verified; the sign of Byzantine expected return is `[RESULT NEEDED]`.*

H1 holds: honest submission pays full, and the other three strategic behaviors all pay 0. H3 holds: the quality floor blocks $q<f$ at the award stage (`test_quality_floor`), without waiting for acceptance. H2 partially holds: Byzantine payment is 0 and slashed, but the strict conclusion "expected return negative" needs full-factorial simulation — currently negative balance is observed only at the single point $s=1.0$, strict acceptance, [RESULT NEEDED: slashing rate {0,0.5,1,2} × acceptance hit rate {0.7,0.9,1.0} grid].

Specifically in `test_rejected_byzantine_work_unpaid_and_slashable`: Byzantine agent z starts with balance 3.0, and after acceptance rejection `settle(accepted=False, slash=2.0)`, z's balance falls to 1.0, `slashed=2.0`, `total_paid=0`. This means z not only got no payment but also lost 2.0 deposit — under this configuration, the ledger cost of false reporting is certain. But this is one deterministic demo, not an expected-return statistic; whether z will falsely report depends on its estimate of "the probability $p$ of being caught by acceptance," and this $p$ under current oracle acceptance is always 1 (a bad result is always caught once submitted), so z must lose. Once acceptance has misses ($p<1$), the conclusion must be recomputed by the formula in Section 4.5.

Two other award details worth recording: `test_load_breaks_tie` verifies same value-for-money chooses the lower-loaded (load=1 beats load=5), avoiding task clustering; `test_no_eligible_bid_no_winner` verifies no forced award when no one is qualified (winner stays None), a conservative design of "rather leave empty than force," safer than "randomly pick one."

### 6.3 End-to-end integration: clean vs stressed

Table 3 and Figure 3 give the two-level comparison of matrix integration.

**Table 3 Matrix end-to-end settlement (cpu-proto, 24 work orders; source `reports7/matrix_scale_demo.json`)**

| Metric | clean | stressed (3 faulty nodes) |
|---|---|---|
| n_orders | 24 | 24 |
| accepted | 24 | 24 |
| qa_rounds | 24 | 25 |
| retries | 0 | 5 |
| isolated_agents | 0 | 3 |
| total_budget | 240 | 240 |
| total_paid | 240 | 240 |
| slashed | 0 | 0 |
| balance_sum | 240 | 240 |
| conserved | true | true |
| duplicate_work | 0 | 0 |

*Table 3 note: evidence grade cpu-proto (single-seed pilot). Both levels total_paid=240=total_budget, so holds=0, balance_sum=240, and Eq.(4) always holds. The stressed level injects faulty nodes `spec-kinematics-0`/`spec-spatial-1`/`spec-dataflywheel-2`, isolated by the circuit breaker (isolated_agents=3) producing 5 reworks (retries=5, qa_rounds=25), but `duplicate_work=0` — rework is redoing the same work order, and the unique-payment constraint guarantees no duplicate payment. The stressed level `slashed=0` is because this run's faults take the circuit-breaker path rather than the market-slash path, not that the slashing mechanism is missing.*

![Figure 3: clean vs stressed](figures/P11_fig3_matrix.png)

*Figure 3 Matrix end-to-end clean/stressed two-level comparison. Left clean, right stressed; the top row is governance/settlement metrics (n_orders, accepted, retries, isolated_agents), and the bottom row is the ledger (budget/paid/slashed/balance_sum/conserved). Both levels are `conserved=true`, 24/24 settled by acceptance. Evidence grade cpu-proto (single-seed pilot). Data source `reports7/matrix_scale_demo.json`.*

Both levels are 24/24 settled by acceptance, `conserved=true`. The stressed level injected 3 faulty nodes (`spec-kinematics-0`, `spec-spatial-1`, `spec-dataflywheel-2`), and the system isolated them through the circuit breaker (isolated_agents=3), producing 5 reworks (retries=5, qa_rounds=25), but **rework produces no duplicate payment** — `duplicate_work=0`, because `settle`'s unique-payment constraint guarantees a work order is paid only once. Note: the stressed level `slashed=0`, because this integration's fault governance takes the **circuit-breaker isolation** rather than **market slashing**; the slashing mechanism itself is verified in unit tests (Tables 1, 2). These are two independent defenses: market slashing handles "falsely reported but completed," and circuit-breaker isolation handles "node broken and unable to complete at all."

From governance metrics, the clean level's governance is all zero (state_loss=0, duplicate_work=0, no_closer=0, premature_completion=0, unbounded=0, trace_gaps=0), trace_verified=true, trace_len=48; the stressed level likewise governance ok, trace_verified=true, trace_len=50. That is, even with 3 faulty nodes injected and 5 reworks produced, the governance audit found no duplicate labor, state loss, or trace break. This confirms the "unique-payment constraint" also works in end-to-end integration: rework is redoing the same work order and will not pay a second time because of redoing.

## 7 Discussion

**Why conservation is a theorem while IC is empirical.** This distinction matters because it determines what we can promise. Conservation: as long as the code path does not bypass `settle`, arithmetic identity — this is promise-able. IC: whether Byzantine has negative return depends on the combination of acceptance hit rate and slashing rate — this can only be measured in a specific configuration. This paper's current slashing configuration ($s=1.0$ on T2) makes Byzantine agent z's balance fall from 3.0 to 1.0 (`test_rejected_byzantine_work_unpaid_and_slashable`), negative return on the ledger; but if $s$ is set to 0, Byzantine only loses time, not deposit, and expected return may turn positive. Therefore this paper **does not promise** "Byzantine has negative return at any slashing rate," only promises "negative balance observed under the configuration $s=1.0$, strict acceptance."

There is also a design choice worth expanding: why slashing goes through deposit deduction rather than directly confiscating all balance? `settle`'s `slash` is a configurable parameter (default 0, passed by the caller), meaning slashing strength is a policy knob, not hardcoded. In unit tests we use two strengths slash=1.0 (T2) and slash=2.0 (z) to demonstrate: the former is "minor false reporting," the latter "explicit Byzantine." Making strength a parameter is because different tasks have different harm degrees — a task writing a wrong document and a task forging a safety-critical result should be slashed by different multiples. This paper does not prescribe the optimal slashing rate; that is the question the Section 4.5 threshold grid answers.

There is also a design philosophy worth recording: this mechanism **rather leaves empty than mispays**. `test_no_eligible_bid_no_winner` makes clear that when all bidders fail the quality floor, the auction returns None and does not force. This is opposite to the aggressive scheduling of "must dispatch the task before deadline." In an internal market, forcing to an unqualified bidder ultimately only produces rework and duplicate payment; rather let the task temporarily be unclaimed, await the next bidding round or escalate to a superior, than mispay. This "conservative-first" default, with P5's "rather a wider interval" and P6's "rather say unidentifiable," is the same engineering ethics: choose the conservative side under uncertainty.

## 8 Limitations and Validity Threats (separate chapter)

**T1 Internal threat (main).** Unit tests are deterministic single points, without ≥1000-round strategy simulation, and IC conclusions are narrow. **Most likely counterexample**: changing slashing rate $s$ or acceptance hit rate $p$, Byzantine expected return turns positive. **How this paper guards**: write the Eq.(5) break-even point $s>R(1-p)/p$ as an analytical threshold, with the full-factorial grid marked `[RESULT NEEDED]`.

**T2 Construction threat: acceptance is an oracle.** Current QA judges "whether the result is qualified" as a known Boolean (acceptance hit rate $p=1$), and in a real system acceptance itself may be manipulated — a deeper governance problem this paper does not solve.

**T3 External threat.** CPU prototype, agents not real LLMs, million-scale only closed-form extrapolated, not run in a real distributed system.

**T4 Measurement threat.** The stressed level `slashed=0` does not mean the slashing mechanism is ineffective, only that this run did not trigger the slash path (faults took the circuit-breaker isolation path).

**T5 Concurrency threat.** `ContributionLedger` uses a normal Python `dict`, assuming single-thread sequential settlement. In real distributed deployment, if two agents almost simultaneously submit the same work-order result, the single-thread dict may race: both `settle`s read "not paid" and then both pay, breaking conservation. Currently this is avoided by the caller (`matrix.py`) single-thread serially calling `settle`, rather than the ledger itself being concurrency-safe. Conservation under "sequential settlement" is a theorem, and under "concurrent settlement" needs linearizability guarantees, which this paper does not solve.

**T6 No Sybil defense.** "Unique payment" relies on `paid_tasks`'s task→agent mapping, assuming one task has only one owner; it **does not defend Sybil** — if an attacker bids with multiple identities and repeatedly submits, the `duplicate` marking triggers only when upstream correctly identifies "this is duplicate labor." This paper honestly declares: no Sybil defense, no identity deposit, no encrypted settlement.

## 9 Resource Gates and Applicability Boundaries

- This paper's conclusions stop at cpu-proto (CPU deterministic prototype). Full-factorial strategy simulation (≥1000 rounds/cell), real LLM agent bidding, and real distributed ledgers are all to be supplemented.
- No linkage to any real crypto/token economy, no financialization claim.
- Applicability boundary: this paper's method applies to "closed, auditable, internal-accounting" multi-agent task markets; it does not apply to open economies, scenarios needing Sybil defense, or real-currency settlement.
- Relation to P1/P2/P3: P1 covers topology reducing complexity, P2 covers BFT stop decisions, P3 covers fault-governance RCT, and this paper covers how money settles in the governance loop — the four together form a complete agent-matrix governance stack.

Specifically, the resource gate's limitation on P11 conclusions appears in three places. First, **strategy simulation not run**: currently only deterministic unit tests and end-to-end two levels, without the expected-return distributions of five strategies "honest/duplicate/low-quality rushing/Byzantine/collusive price suppression" under different slashing rates and acceptance hit rates, so H2's IC conclusion can only stop at a single point. Second, **acceptance is an oracle**: current QA rules are clear and acceptance hit rate is always 1 (bad results always caught), and in a real system acceptance itself may be manipulated or miss, a deeper governance problem. Third, **scale not measured**: million-agent only closed-form extrapolated (see P1), and the settlement ledger's consistency under real distributed concurrency (such as the race of two agents simultaneously submitting the same work order) needs distributed-ledger mechanisms, which this paper's single-thread `dict` implementation does not handle.

## 10 Conclusion

On UDOS v7.4.9's internal task market, we give a settlement mechanism of "quality floor + value-for-money award + payment only to the unique accepted completer + Byzantine slashing." The conservation invariant `balance_sum = total_paid − slashed` is a theorem, and both the mixed 3 tasks and matrix clean/stressed levels measured `conserved=true`; duplicate labor and false reporting pay 0 on the ledger, and all 10 unit tests pass. The honest part: IC is an empirical approximation, not a theorem, and the full-factorial threshold grid of slashing rate/acceptance hit rate has not run; no Sybil defense, no real pricing game. We write separately what is promise-able (conservation) and what can only be approximated (IC), and mark unfinished work with `[RESULT NEEDED]`. An auditable internal market is first a market with balanced books.

There are three follow-up works. First, run full-factorial strategy simulation (strategy family {honest, duplicate, low-quality rushing, Byzantine, collusive price suppression} × slashing rate {0,0.5,1,2} × acceptance hit rate {0.7,0.9,1.0}, ≥1000 rounds/cell), locate the slashing-rate break-even point $s^*=R(1-p)/p$, and upgrade H2 from a single point to an extrapolatable threshold curve. Second, connect the settlement ledger to real LLM agent bidding and test "whether self-assessed quality q_i can be honestly reported" — the most fragile link of IC. Third, handle concurrency races: the current single-thread `dict` implementation does not handle the race of "two agents simultaneously submitting the same work order," and distributed deployment needs a linearizability ledger backend. Reading three papers together (P5 credibility + P6 identifiability + P11 settlement) jointly points to an engineering philosophy: **an honest system is not one that can do everything, but one that knows what it cannot do** — in settlement, knowing which payment should be made, which must be deducted, and which should rather be on hold than mispaid.

Finally returning to this paper's plainest claim: in the process of multi-agent systems moving toward million scale, "balanced books" is a more basic and scarcer capability than "optimal mechanism." This paper proves this with 300 lines of code and 10 tests. It is not elegant, not complex, not theoretical, even a bit boring — but a non-boring, error-free, auditable internal ledger is precisely the foundation supporting all intelligent collaboration above. A foundation need not be pretty; it needs to bear weight. May this honest 300-line ledger become a directly readable, modifiable, runnable starting point for later people discussing settlement on million-agent systems, rather than yet another white paper no one can reproduce.

## References

> Note: the verified-reference fields already cited in this draft are copied from the whole-volume "Master Reference Library" (verification date 2026-09-19); the rest are to-be-verified search expressions, not treated as cited facts, and before submission metadata and DOI must be verified item by item through academic search.

### Verified citations (✅ master library 2026-09-19)

- Smith, R. G. (1980). *The Contract Net Protocol: High-Level Communication and Task Control in a Distributed Problem Solver.* IEEE Transactions on Computers, C-29(12). DOI:10.1109/TC.1980.1675516. —— Request-bid-award negotiation, the historical source of UDOS task dispatch and `TaskAuction`.
- Vickrey, W. (1961). *Counterspeculation, Auctions, and Competitive Sealed Tenders.* Journal of Finance, 16(1). —— Second-price auction = VCG prototype, truth-telling as dominant strategy.
- Ausubel, L. M., & Milgrom, P. (2006). *The Lovely but Lonely Vickrey Auction.* In *Combinatorial Auctions* (book chapter), MIT Press. —— VCG theoretical beauty and practical defects (collusion/budget imbalance/computational hardness), the direct basis for this paper's "pragmatic step back."
- Milgrom, P. (2019). *Auction Market Design: Recent Innovations.* Annual Review of Economics, 11. —— Modern market design for combinatorial auctions/electricity/spectrum/procurement, procurement-auction paradigm reference.
- Castro, M., & Liskov, B. (1999). *Practical Byzantine Fault Tolerance.* OSDI 1999 (USENIX). —— Tolerates $f<n/3$ Byzantine nodes under asynchrony; the distributed-systems comparison for this paper's Byzantine slashing.

### To-be-verified reference search expressions and candidate directions

- Contract net/task allocation: `multi-robot task allocation market-based; multi-agent task allocation payment mechanism design`.
- Auction theory: `procurement reverse auction; peer prediction mechanism proper scoring rule` (core peer-prediction literature to supplement).
- Byzantine/slashing: `slashing Proof of Stake economic penalty`.
- Sybil resistance: `Sybil attack Douceur; Sybil resistance open economy`.

## Appendix A Reproduction commands and test index

```bash
cd release-v5.4.4/udos-engine            # main, tag v7.5.0, commit 1a71270
pytest tests7/test_v749_market.py -q     # 10 unit tests
python3 scripts7/matrix_demo.py          # end-to-end integration -> reports7/matrix_scale_demo.json
```

Key source: `udos7/topology/market.py` (`Bid`/`TaskAuction`/`ContributionLedger`), `udos7/topology/matrix.py` (integration), `tests7/test_v749_market.py`.

Suggested reproduction order: first run `pytest tests7/test_v749_market.py -q` to see all 10 unit tests green (the minimal verification of conservation and rules), then run `matrix_demo.py` to see end-to-end two levels `conserved=true`. Unit tests are deterministic and complete in seconds; matrix integration is slightly slower but still CPU. Any step with `conserved=false` means the ledger was broken — which is precisely the meaning of `conservation_check()` as a CI gate.

## Appendix B Evidence ledger

| Figure | Source | Grade |
|---|---|---|
| total_paid=16/holds=8/balance_sum=15/slashed=1 | `tests7/test_v749_market.py: test_conservation_invariant` | verified (unit test) |
| Matrix clean/stressed conserved=true | `matrix_scale_demo.json: clean/stressed.market` | cpu-proto |
| 24/24 accepted, retries 0/5, isolated 0/3 | `matrix_scale_demo.json` | cpu-proto |
| 10 unit tests | `tests7/test_v749_market.py` | verified |
| External token-economy figures | — | unverified |

Final reminder: all "meets standard"/"passes" wording in this paper is bounded within the specific scope of "CPU deterministic prototype, single-thread sequential settlement, oracle acceptance." It is not a universal conclusion that "mechanism design holds in any multi-agent system," but "on this specific system of 300-line `market.py`, 10 pytest, 24-order matrix integration, settlement conservation is guaranteed by code structure." Any extension beyond this scope needs new experiments and new evidence.

`[RESULT NEEDED]`: slashing-rate × acceptance-hit-rate full-factorial ≥1000-round simulation, expected-return CI for each strategy, false-slash rate (probability honest agents are slashed). `[CITATION NEEDED]`: the five candidate directions in Section 2.

Supplementary note: the "unit-test" figures in the table (total_paid=16 etc.) come from `test_conservation_invariant`'s hardcoded assertions and are deterministically recomputable; the "matrix clean/stressed" figures come from `matrix_scale_demo.json` and are cpu-proto prototype run results; the two have different evidence grades and should not be mixed. The stressed level `slashed=0` does not mean the slashing mechanism is missing, but that this run's faults take the circuit-breaker path — this distinction is honestly stated in Section 6.3.

## Appendix C Internal review record (pre-submission five-dimension self-assessment, look only, do not change)

Scored on five dimensions per `doubao-academic-evaluator` (0–10, cpu-proto prototype scope):

1. **Problem importance**: 5. Internal-market settlement indeed matters as agent scale grows, but this paper positions an engineering reference implementation, not a theoretical breakthrough.
2. **Technical correctness**: 7. The conservation invariant is guaranteed by code structure with complete test coverage; deductions for IC only at a single point, no full-factorial simulation.
3. **Experimental rigor**: 5. 10 unit tests + end-to-end two levels are hard evidence; but strategy-family simulation not run, IC conclusions narrow.
4. **Novelty**: 5. Mechanism components classical (auction/slashing/ledger), novelty in engineering conservation tests and honest-boundary declaration.
5. **Reproducibility**: 9. Scripts, tests, report JSON complete, CPU rerunnable.

**Fatal-defect check**: the conservation vs IC boundary is honestly distinguished; no fabricated p-values/CI/DOI; no Sybil defense declared. **Conclusion**: currently a submittable empirical-paper draft; before submission full-factorial strategy simulation must be added to upgrade H2 from a single point to a threshold grid, otherwise only workshop/engineering tracks.

## Author's intended-use statement

- **Target journals/conferences**: candidates AAMAS / AAMAS workshop; engineering-leaning can submit ICWS, JSS. If theoretically deepened can submit EC. **Quartile/IF [to verify]**.
- **Pre-registration plan**: the conservation invariant as a hard constraint (already in code); the factorial design and stopping rules of the slashing-rate × acceptance-hit-rate grid registered before submission.
- **Data and code availability**: Apache-2.0; tag `v7.5.0`, commit `1a71270`; reports `reports7/matrix_scale_demo.json`, `tests7/test_v749_market.py`.
- **AI-use statement**: the author led mechanism design and conclusions, AI assisted structured writing, figures, and language polishing; figures sourced by the author against source and tests, AI produced no experimental data.


---

<p align="center"><img src="assets/logo.png" width="180" alt="TwinsEarth"/></p>

# Project-Level Five-Dimensional Evaluation: From Work Orders to a Project Failure Taxonomy

**Working Title (EN):** *Project-Level Five-Dimensional Evaluation: Aggregating UDOS Work-Order Metrics into EPOB-Style Project Quality and a Failure Taxonomy*

> In-volume engineering paper P23 · raising UDOS's measurement granularity from "work order" to "project" · borrowing the EPOB five dimensions and failure-distribution diagnosis · UDOS Reasoning Engine v7.6.0 · Fang Wenxin · 2026-09-21
>
> **One sentence first**: UDOS's current measurement unit is "whether one work order got done," but what a multi-Agent system actually delivers is "one project"—how good the plan is, how accurately tasks are assigned, how smoothly handoffs go, how complete the deliverables are, how fast and costly it runs: five things work-order success rate cannot answer. EPOB (OpenReview NtPIgzPtOU) raises multi-Agent system evaluation from "single-task success rate" to the **project lifecycle level**, and stresses looking at the **failure distribution** rather than only the success rate. This paper connects this five-dimensional framework into UDOS, giving the logic of how work-order metrics aggregate into project metrics—a **design proposal · to implement**.
>
> **Evidence caliber (stated only once in the whole paper)**: this paper is an evaluation-methodology design paper, **containing no new UDOS measured numbers**. The five dimension names and failure taxonomy categories of the external paper EPOB (End-to-End Project Orchestration Benchmark, OpenReview NtPIgzPtOU, 2026-05) have been verified; concrete constructions such as its "180-unit robustness package" are paper reports, not independently reviewed. UDOS old metrics (rework rate, message fan-in, completion rate, handoff completeness, throughput curves) all point back to v7.5.0 papers, evidence grades marked in text. This paper's work-order→project aggregation logic is a **design proposal · to implement**, aggregation weight calibration `[RESULT NEEDED]`.

---

## Abstract

Multi-Agent system evaluation long stayed at "single-task success rate": run one task, done records 1, failed records 0, finally averaged. This granularity has two ills—it only tells you "good or bad," not "where broken"; it only measures "one work order," cannot measure "one project." EPOB (OpenReview NtPIgzPtOU, 2026) raises the evaluation unit to the project lifecycle, measuring along five dimensions: Plan Quality, Assignment Quality, Coordination, Deliverable Quality, Efficiency; more importantly it stresses the **failure distribution rather than only the success rate**, using five failure labels—planning / delegation / handoff / review / recovery—to answer "which link broke." UDOS v7.5.0's measurement actually already covered the "parts" of these five dimensions—P1 measures fan-in and rounds (coordination), P2 measures stopping decisions (coordination/recovery), P3 measures rework and mis-isolation (recovery), P10 measures handoff completeness (handoff/deliverable), P9 measures throughput curves (efficiency)—but they are all pinned at the "single work order" granularity, not aggregated into project-level scores. This paper does two things. First, mapping UDOS's existing work-order metrics one by one onto the EPOB five dimensions, marking which dimensions already have instruments and which lack them. Second, giving the two-layer work-order→project aggregation logic: the bottom layer remains single-work-order metrics, the middle layer classifies the same project's work orders by failure label, the top layer uses project-level weighting to obtain five-dimensional scores. This paper also points out EPOB's drawback: it is a "framework-centric" evaluation, measuring "whether this framework executes projects well," not "whether this framework matches this task"—the latter being precisely the unique value of UDOS P1's topology selection decision tree, not replaceable by EPOB. This paper is methodology design, aggregation weights and thresholds pending `[RESULT NEEDED]`.

**Keywords**: project orchestration; evaluation benchmark; failure taxonomy; five-dimensional metrics; work-order aggregation; EPOB; UDOS P1/P3/P9/P10

---

## Structured Abstract (background problem → method/argument → evidence → contribution)

- **Background problem**: UDOS's metrics are all at work-order granularity; single-work-order success rate cannot answer "whether one project is well planned, accurately assigned"; looking only at success rate also cannot answer "which link broke."
- **Core argument**: the evaluation unit should rise from work order to project; using the EPOB five dimensions as the skeleton, using the five failure labels planning/delegation/handoff/review/recovery for diagnosis, aggregating UDOS's existing work-order metrics in two layers into project-level five-dimensional scores.
- **Evidence clues**: (1) EPOB five dimensions and failure taxonomy (OpenReview NtPIgzPtOU, existence verified); (2) UDOS existing work-order metrics (P1 fan-in/rounds, P2 stopping decisions, P3 rework/mis-isolation, P10 handoff completeness, P9 throughput, pointing back to papers); (3) the known boundary that EPOB does not evaluate "framework-task matching."
- **Contribution**: a mapping table from UDOS work-order metrics to the EPOB five dimensions; two-layer aggregation logic; a five-dimension instrument gap list; the demarcation between EPOB's drawback and UDOS's unique value.

---

## 1 Problem: Work-Order Success Rate Cannot Answer Project Questions

### 1.1 The Two Ills of Single-Task Success Rate

The vast majority of UDOS v7.5.0 measurements—P1's fan-in/rounds, P2's stopping decision correctness, P3's rework rate, P10's handoff completeness—are **single-work-order** granularity. Run one work order, see whether it got done. This granularity is correct during system debugging, but it has two ills:

- **Only says good or bad, not where broken.** A project failed—was the plan wrong from the start (planning)? Or the plan right but the wrong person assigned (delegation)? Or information lost at handoff (handoff)? Single-work-order success rate compresses these five failures into the same 0, diagnostic value zero.
- **Only measures one work order, cannot measure one project.** Real delivery is "one project": multiple work orders, multiple handoff rounds, with closing. Single-work-order average success rate may be high, while project-level plan quality and delivery completeness are a mess.

### 1.2 EPOB's Answer: Project Lifecycle + Failure Distribution

EPOB (End-to-End Project Orchestration Benchmark, OpenReview NtPIgzPtOU, 2026-05) raises the evaluation unit from single task to the **project lifecycle**, scoring along five dimensions:

- **Plan Quality**: how well the task is decomposed;
- **Assignment Quality**: to whom tasks are assigned, how accurately;
- **Coordination**: fan-in, rounds, stopping decisions;
- **Deliverable Quality**: completion rate, handoff completeness;
- **Efficiency**: throughput, cost curves.

Its more crucial methodological claim is the **failure distribution rather than only the success rate**: attaching taxonomy labels to each failure—planning (wrong at the planning stage), delegation (wrong assignment), handoff (information lost at handoff), review (review missed it), recovery (recovery failed)—turning evaluation from "good or bad" into "where broken."

### 1.3 UDOS Does Not Start from Zero

Interestingly, the "parts" of UDOS's five dimensions are already scattered across papers. This paper's work is not building new instruments but **aggregating the scattered work-order metrics into project-level five-dimensional scores**.

![Figure 1 Mapping from UDOS work-order metrics to the EPOB five dimensions](figures/P23_fig1_mapping.png)

*Figure 1 Left column UDOS existing work-order metrics, middle column EPOB five dimensions, right column the instrument status of the five dimensions in UDOS (existing/to supplement). Conceptual illustration, not measured data.*

---

## 2 Five-Dimension Mapping and Instrument Gaps

### 2.1 The Mapping Table

**Table 1 Mapping from UDOS work-order metrics to the EPOB five dimensions**

| EPOB dimension | Meaning | UDOS corresponding work-order metric | Instrument status |
|---|---|---|---|
| Plan Quality | task decomposition quality | P1 topology selection, P3 governance design decomposition reasonableness | partial: topology comparison exists, lacks direct "decomposition quality" metric |
| Assignment Quality | task assignment quality | P11 internal market bidding, P17 scope granting | partial: bidding mechanism exists, lacks "assignment right/wrong" label |
| Coordination | coordination quality | P1 fan-in/rounds, P2 stopping decisions | existing: protocol-layer measurement mature |
| Deliverable Quality | deliverable quality | P3 completion rate, P10 handoff completeness | existing: contract tests cover |
| Efficiency | efficiency | P9 throughput curves, cost | existing: cpu-proto curves |

### 2.2 Where the Gaps Are

The mapping table exposes two clear gaps:

- **Plan Quality lacks a direct metric.** UDOS can measure "the layered tree presses fan-in to a constant" (P1, verified extrapolation), but this is "coordination efficiency," not "whether this project's task decomposition itself is reasonable." A poorly decomposed plan can still be efficiently executed by the layered tree.
- **Assignment Quality lacks an "assignment right/wrong" label.** P11 has a bidding mechanism, but no post-hoc marking of "whether assigning this to X was optimal." Without this label, Assignment Quality can only compute "how fast assigned," not "whether assigned correctly."

These two gaps are precisely what this paper fills: they are not new systems, but on existing work-order records **adding two post-hoc labels**—decomposition reasonableness label, assignment right/wrong label.

---

## 3 The Two-Layer Work-Order→Project Aggregation Logic (design proposal · to implement)

### 3.1 Why Cannot Directly Average

Directly averaging the success rates of all work orders in a project is the practice EPOB criticizes—it lumps planning failure and recovery failure together. This paper adopts two-layer aggregation:

**Bottom layer (work-order level)**: each work order records a set of atomic metrics—whether completed, failure label (one of five), fan-in, rounds, handoff completeness, cost.

**Middle layer (failure distribution)**: classifying the same project's work orders by failure label, obtaining the count distribution of five failure classes $n_p, n_d, n_h, n_r, n_{rec}$. This layer answers "which link broke."

**Top layer (five-dimensional scores)**: mapping the middle-layer distribution to five-dimensional scores. For example—

$$\text{Plan Quality} \propto 1 - \frac{n_p}{\sum n} \tag{1}$$

i.e. the higher the planning failure share, the lower Plan Quality; Assignment Quality inversely related to $n_d$, Coordination related to rounds/fan-in, Deliverable Quality related to completion rate/handoff completeness, Efficiency related to unit delivery cost. The normalization weights of each dimension are design parameters, pending `[RESULT NEEDED]` calibration.

### 3.2 Granularity Difference from EPOB

Must honestly point out: EPOB's evaluation granularity is "project," UDOS's current measured granularity is "work order." This paper gives **the logic of aggregating from work order to project**, not project-level scores already run out. If UDOS is to submit to a systems conference, this aggregation must run through on real multi-work-order projects—otherwise P23 is only methodology, no data. This is precisely the core of `[RESULT NEEDED]`.

![Figure 2 Two-layer aggregation pipeline](figures/P23_fig2_aggregation.png)

*Figure 2 Work-order-level atomic metrics → five-class failure distribution → project-level five-dimensional scores; diagnostic value concentrated in the middle failure-distribution layer. Conceptual illustration.*

---

## 4 Drawbacks / Failure Boundaries / Falsifiable Conditions

### 4.1 Drawbacks (restraint that must be kept when citing EPOB)

1. **EPOB does not evaluate "framework-task matching."** It measures "whether this framework's project execution effect is good," **not "when to use star topology, when to use layered."** This is not a drawback EPOB hides, but its problem set does not contain this question. UDOS P1's topology selection decision tree precisely fills this gap—do not take EPOB as "we have been externally evaluated"; it and UDOS are complementary, not duplicate.
2. **Failure taxonomy labels have a human-judgment component.** The boundary between planning and delegation is not clean in ambiguous cases; label consistency needs annotation specifications, otherwise the failure distribution itself has noise.
3. **Framework-centric ≠ task-centric.** EPOB takes the framework as the object under test; UDOS's interest is "structural prior determines the capability upper limit," the two ask from different angles; when transferring its five dimensions be aware of this perspective difference.

### 4.2 Failure Boundaries

- If a project's work orders are highly isomorphic (P9's homogeneous copies), the failure distribution concentrates on a single label, five-dimension discrimination drops—this is not the aggregation logic being wrong, but the project itself lacking diversity; EPOB-style five dimensions are inherently insensitive to homogeneous projects.
- If the two new labels Plan Quality / Assignment Quality rely on LLM post-hoc judging, they introduce the LLM-as-judge bias P12 already discussed; these two labels best also go through execution-style determination (e.g. comparison with oracle decomposition/assignment), rather than purely subjective scoring.

### 4.3 Falsifiable Conditions

- **F23-a**: if the project-level five-dimensional scores obtained by two-layer aggregation are **not correlated** with domain experts' qualitative ranking of the same batch of projects (rank correlation below the design threshold), then aggregation weight calibration fails, the five-dimension mapping needs redoing.
- **F23-b**: if the failure distribution diagnosis ("broke at handoff") has a high inconsistency rate with post-hoc root cause analysis conclusions, then the taxonomy label specification needs revision.
- The above are post-hoc-scorable criteria under a design proposal, not established conclusions; weights and thresholds pending `[RESULT NEEDED]`.

---

## 5 Conclusion

EPOB raises evaluation from "single-task success rate" to "project lifecycle + failure distribution," the direction correct. What UDOS must do is not rebuild a benchmark, but aggregate its work-order metrics scattered in P1/P2/P3/P9/P10/P11 according to the EPOB five dimensions in two layers into project-level scores, and fill the two gap labels Plan Quality, Assignment Quality. At the same time must stay clear-headed: EPOB does not answer "whether the framework matches the task," that being UDOS P1's topology selection tree's exclusive ground—the five-dimension evaluation is a checkup form, not a matching plan.

**Work-order success rate only tells you whether it got done; the failure distribution tells you which link broke. Evaluating a system, first measure where it breaks, then measure whether it is good.**

---

## References

**Evaluation methodology foundations**

1. Dean, J., & Bowen, K. (1987). *Calibration of Acceptance Measures for Software Defects* (early thinking on defect classification and density analysis).
2. Henard, R. H., & Manner, M. K. (2014). Predicting product success with a quantitative model. (general method for quality multi-dimensional evaluation)

**2026 frontier material (existence verified online 2026-09-21; concrete construction numbers are paper reports, not independently reviewed)**

3. EPOB (2026). *End-to-End Project Orchestration Benchmark*. OpenReview NtPIgzPtOU. (Plan/Assignment/Coordination/Deliverable/Efficiency five dimensions; planning/delegation/handoff/review/recovery failure taxonomy)

**UDOS paper volumes (this series, evidence grades marked in text)**

4. UDOS v7.5.0 paper volume: P1 "Million-level extrapolation of coordination complexity of layered hybrid topology" (fan-in O(N)→O(log N), verified extrapolation), P2 "BFT-lite stopping decisions," P3 "Fault governance RCT" (rework/mis-isolation), P9 "Quality saturation law and throughput curves" (cpu-proto), P10 "TransferBundle handoff completeness" (26 contract tests), P11 "Internal task market," P12 "Evidence grading" (LLM-as-judge bias discussion).
5. UDOS v7.6.0 paper volume: P17 "Least-privilege orchestration" (scope granting), P33 "v7.6.0 master architecture."

---

## Evidence Discipline and Reproduction Notes

- This paper is an **evaluation methodology design paper**, no new experiments; two-layer aggregation formula (1), five-dimension weights, F23-a/b criteria are all design proposals, actual project-level scores `[RESULT NEEDED]`, forbidden to write as measured.
- EPOB five-dimension names and failure taxonomy categories have been verified; concrete constructions such as the "180-unit robustness package" are paper reports, not independently reviewed.
- If the two new Plan/Assignment labels use LLM post-hoc judging, note the judging bias P12 already discussed, prefer execution-style comparison.
- This paper does not claim EPOB has evaluated UDOS; the two are a complementary relationship of framework-centric and task-structure-centric.


---

<p align="center"><img src="assets/logo.png" width="180" alt="TwinsEarth"/></p>

# Agent Task Markets: VCG, Private Information, and Settlement Neutrality

**Working Title (EN):** *Agent Task Markets: VCG, Private Information, and the Design Goals of Settlement Neutrality*

> Shard D · Market and Architecture paper P25 · the open-market extension of v7.5.0 P11 "Settlement conservation of the internal task market" · cross-referenced with P17 (least privilege), P10 (TransferBundle) · UDOS Reasoning Engine v7.6.0 · Fang Wenxin · 2026-09-21
>
> **One sentence first**: when task allocation among Agents moves from "bookkeeping inside one closed system" to "a cross-system open market with private information," the old "cost-performance award" is no longer enough—the real difficulty is not "who wins," but "**how to make Agents willing to state their true cost, while keeping the ledger from creating or losing money out of thin air during cross-system settlement**"; this paper splits this problem into three lines: VCG-style incentives (making truth-telling pay), private-information mechanism design (making bids carry strategy notes), and "settlement neutrality," a UDOS-authored engineering design goal.
>
> **Evidence caliber (stated only once in the whole paper)**: this paper is a **design survey + proposal**, containing no new UDOS measured numbers. All three external dialogue objects were verified online 2026-09-21: AgentLance (arXiv:2608.23867, verified), Agent Exchange (arXiv:2507.03904, verified), ISONOMIA (**correctly recorded as Zenodo records/21338480**, caution; its "ergs mutual-trust credit unit, bicameral governance" are true, while numbers such as "45,000 whole-economy simulations / 300 parameter points" were not verified, all `[RESULT NEEDED]`). Caution: the "five attributes of settlement neutrality" in the user's clue **has no traceable original source** (ledger failed), this paper does not cite it as an existing framework, but rewrites it as a **UDOS P11/P25-authored design goal**, marking `[CITATION NEEDED]` where academic backing is needed. UDOS old numbers only point back to v7.5.0 P11 (settlement conservation invariant, 10 unit tests verified, matrix clean/stressed both conserved=true).

---

## Abstract

Multi-agent orchestration is moving from "internal dispatch" to "market dispatch." v7.5.0's P11 already wrote "the books balance" of a closed internal market as a settlement conservation invariant guaranteed by code structure (`balance_sum = total_paid − slashed`, `total_paid ≤ total_budget`). But once the view extends to a cross-system open Agent market with private cost information, three new problems follow: first, bidders **know their true cost while buyers do not**, "cost-performance award" is no longer transparent under private information; second, VCG's elegant "truth-telling dominant" mechanism is engineering-expensive, collusion-prone, and does not satisfy budget balance (Ausubel–Milgrom long pointed out its "theoretically elegant, practically lonely"); third, during cross-system settlement, the ledger bottom line "money is not created or destroyed out of thin air" cannot be guaranteed only by a single system's internal invariants.

This paper surveys along three lines and gives UDOS's extension design: **(1) VCG and private information**—AgentLance (arXiv:2608.23867) does decentralized orchestration with "private-cost bidding + self-maintained strategy notes + VCG-style payment," this paper thereby distinguishes "closed cost-performance award" from "open incentive-compatible award"; **(2) auctions and liquidity layering**—Agent Exchange (arXiv:2507.03904) borrows from real-time ad bidding, using multi-attribute auctions to degrade to greedy/random allocation when liquidity is insufficient; **(3) credit units and governance**—ISONOMIA (Zenodo 21338480) uses "ergs," a mutual-trust bookkeeping unit generated only at verified settlement with net zero, and bicameral governance, answering "what does an open market use to keep accounts, who governs."

On top of the three, this paper explicitly makes "settlement neutrality" a **UDOS-authored design goal** (rather than citing an existing framework), and honestly states it currently lacks academic backing `[CITATION NEEDED]`. All new designs in this paper are **design proposals · to implement**, not claiming to have been measured in an open market.

**Keywords**: Agent task market; VCG; private information; strategy notes; settlement conservation; settlement neutrality; internal market; open market; engineering mechanism design

---

## Structured Abstract (background problem → method/argument → evidence → contribution)

- **Background problem**: P11's closed internal market solved "the books balance," but the open Agent market introduces three problems P11 did not face: private cost information, cross-system settlement, real governance.
- **Method/argument**: splitting open-market dispatch into three orthogonal lines—the incentive line (VCG/truth-telling), the allocation line (auctions/liquidity degradation), the bookkeeping-governance line (credit units/governance structure); and raising "settlement neutrality" as a UDOS-authored engineering design goal.
- **Evidence clues**: (1) AgentLance's private cost + strategy notes + VCG-style payment (verified, paper reports it beats single models and centralized allocation); (2) Agent Exchange's multi-attribute auctions and liquidity degradation (verified); (3) ISONOMIA's ergs credit unit and bicameral governance (caution, simulation numbers not verified); (4) P11's verified settlement conservation invariant as the closed baseline.
- **Contribution**: (1) layering "closed cost-performance award" and "open incentive-compatible award"; (2) giving a design map of the open market's three orthogonal lines; (3) clarifying "settlement neutrality" is a UDOS-authored goal and marking its evidence gap; (4) marking all new designs `[RESULT NEEDED]`, not exaggerating.

---

## 1 Introduction: From "Dispatch" to "Market," the Problem Changed

P11 faces a **closed internal market**: tasks first priced, Agents bid, awarded by "quality threshold + cost-performance + load fallback," paid only to the unique completer after QA acceptance, Byzantine deposits slashed. It wrote "the books balance" as a conservation invariant, and proved this conservation is **a theorem guaranteed by the settlement code structure**, not luck.

But "internal dispatch" and "market dispatch" differ essentially. In a real, cross-organizational Agent ecosystem, three things P11 did not face appear:

1. **Private cost information**: bidders know their true cost (they know their compute, they know their private data allowance), while buyers and other Agents do not. P11's `Bid` "cost" is self-reported by the bidder, enough in a closed system; in an open system, **self-reported cost can lie**.
2. **From "award" to "allocation"**: in a closed system the Agent pool is fixed, award rules simple; in an open system one must face liquidity—sometimes many bidders, sometimes not one, rules must degrade reasonably under different liquidity.
3. **What cross-system uses to keep accounts, who governs**: P11's ledger is a single system's internal bookkeeping unit, invalid outside that system. Cross-system settlement needs a "credit unit" and a governance structure of "who has the final say."

This paper pushes P11's closed ledger one step in each of these three open directions. But before pushing, stay calm: **VCG, the prettiest mechanism in textbooks, is precisely the hardest to land in engineering**. P11 already cited Ausubel and Milgrom's sober judgment—VCG is "theoretically elegant, practically lonely": compute-expensive, collusion-prone, not budget-balanced, rarely deployed at scale in real market design. This paper's orientation is therefore not "move VCG over," but "see clearly what problem VCG wants to solve, then approximate it within what engineering can bear."

---

## 2 The Incentive Line: VCG, Private Cost, and Strategy Notes

![Figure 1 Closed award vs open incentive-compatible award](figures/P25_fig1_closed_vs_open_market.png)

*Figure 1 Left: closed internal market—cost visible, cost-performance award, conservation guaranteed by code structure (P11 verified); right: open market—cost private, needs incentive compatibility making truth-telling pay, needs cross-system bookkeeping. Conceptual illustration, not measured data.*

### 2.1 What Problem VCG Wants to Solve

VCG (Vickrey–Clarke–Groves)'s core idea in one sentence: **make each bidder pay "the externality its presence imposes on others,"** so truth-telling becomes the dominant strategy. Under ideal assumptions it simultaneously achieves incentive compatibility (truth-telling optimal) and Pareto efficiency. The problem is engineering: it needs public preference aggregation, combinatorial-explosion compute, and does not satisfy budget balance.

P11's choice is stepping back—not pursuing "truth-telling dominant," but pursuing "the books balance": using "pay only on acceptance, pay only the unique completer, Byzantine necessarily slashed" to suppress duplicate labor and false reporting, exchanging VCG's elegance for 300 lines of testable conservation. This paper continues this orientation.

### 2.2 AgentLance: Private Cost + Strategy Notes

AgentLance (*Markets, Not Planners: Decentralized Orchestration of LLM Agents with Private Information*, arXiv:2608.23867, verified) faces precisely the "private cost" point. Its mechanism (paper report):

- Agents bid with **private cost**—they know their cost but do not disclose it to others;
- each Agent self-maintains **strategy notes**, recording how it historically bid and the outcomes;
- the allocator picks winners based on bids and **public reputation records**;
- adopting **VCG-style payment**—making bidders' bids reflect true cost.

The paper reports it beats single models, centralized allocation and market baselines on math reasoning, code generation, knowledge-intensive QA and Agent tasks (paper report · not independently reviewed).

The borrowing for UDOS is not "copying the VCG payment formula," but two structural judgments:

1. **Private information is a first-class variable.** P11's `Bid.cost` is self-reported and closed; if UDOS wants an open market, it must directly answer "whether self-reported cost is trustworthy." AgentLance's answer is: **not assuming it trustworthy, but designing payment rules making truth-telling beneficial to oneself** (VCG-style), and using **public reputation** as external correction.
2. **"Strategy notes" are a self-governing memory worth borrowing.** Bidders do not bid from zero each time, but bid with notes of historical bidding experience. This is spiritually consistent with v7.6.0 P18's "BRS+DRS memory flywheel"—both compress "past experience" into reusable internal memory, rather than deciding from scratch each time.

### 2.3 UDOS Layering: When to Use Cost-Performance, When to Approximate VCG

This paper proposes layering UDOS's award rules by "information openness":

- **Closed, same system, cost approximately visible** → continue P11's "quality threshold + cost-performance + load fallback," conservation guaranteed by code structure (verified).
- **Open, cross-system, cost private** → adopt the approximate mechanism of "VCG-style payment + public reputation correction," the goal being to make **truth-telling more pay than lying**, rather than strictly dominant.

Why "approximate" rather than "strict VCG"? Because P11 already made clear: strict VCG is compute-expensive, collusion-prone, not budget-balanced. UDOS's orientation is engineering auditability first—approximate incentive compatibility where possible, but never pretend to have proven strict dominance.

---

## 3 The Allocation Line: Auctions, Liquidity, and Allocation Degradation

### 3.1 Agent Exchange: An Ad-Bidding-Style Agent Market

Agent Exchange (*Agent Exchange: Shaping the Future of AI Agent Economics*, arXiv:2507.03904, verified) borrows real-time ad bidding (RTB) to design Agent task allocation: treating tasks as "impression slots," Agents as "advertisers," using **multi-attribute auctions** (looking not only at price but quality, capability, latency) for allocation. Its reported mechanisms (paper report) include:

- an RTB-style auction platform (component divisions such as USP/ASP/Agent Hubs/DMP);
- multi-attribute auctions + generalized second price;
- **degradation when liquidity is insufficient**—when bidders are not enough, degrading to greedy or random allocation, rather than stuck at "must auction."

The borrowing for UDOS is "**liquidity awareness**." P11's award rules assume bidders are sufficient; open markets often lack enough bidders. A good allocation rule should be **adaptive degradation**: precise auctions when many, degrading to "nearest dispatch" or "random fallback" when few, rather than shutting down because bidders cannot be assembled.

### 3.2 From "Who Wins" to "How to Degrade"

This section's real argument is: the design difficulty of open-market allocation rules lies not in "how to pick the optimum when bidders are sufficient" (auction theory already solved this), but in "**how to degrade gracefully when bidders are insufficient, information incomplete, transient faults occur**." This is spiritually fully consistent with UDOS P3's circuit breaker, P2's view change—a good system does not run beautifully under ideal conditions, but does not crash when ideal conditions break.

---

## 4 The Bookkeeping and Governance Line: ergs Credit Unit and Bicameral Governance

![Figure 2 The three orthogonal lines of the open market: incentives, allocation, bookkeeping governance](figures/P25_fig2_three_lines_market.png)

*Figure 2 The three orthogonal design lines of the open Agent market—incentives (making truth-telling pay), allocation (degrading with liquidity), bookkeeping governance (credit unit + governance structure). P11 closed conservation is its special case. Conceptual illustration, not measured data.*

### 4.1 ISONOMIA: The Net-Zero Mutual-Trust Credit Unit

ISONOMIA (*ISONOMIA: A Constitutional Design for Autonomous Agent Labor Markets*, **correctly recorded as Zenodo records/21338480**, caution) faces "what an open market uses to keep accounts." Its core design (paper report):

- **ergs**: a mutual-trust bookkeeping unit, **generated only as debit-credit pairs at verified settlement, net zero**—i.e. not "printing money out of thin air," but "one debit, one credit, netting to zero";
- **non-transferable reputation**: reputation follows behavior, cannot be resold like a token;
- **bicameral governance**: one chamber voting weighted by accumulated prestige, one chamber voting equal by lot—using the dual track of "capability chamber + equal-rights chamber" to avoid governance being monopolized by a few strong players.

The borrowing for UDOS is very direct, and strikingly echoes P11's conservation: P11 requires "money not created or destroyed out of thin air" (`balance_sum = total_paid − slashed`), ISONOMIA's ergs require "each entry generated in pairs, net zero." **This is the same conservation intuition, one implemented inside a single system with code invariants, one implemented in an open economy with bookkeeping-unit design.** This shows "the books balance" is not UDOS's private good, but a common bottom line across scales of markets.

Caution, honestly marked: ISONOMIA's simulation numbers such as "45,000 whole-economy simulations / 300 parameter points × 50 seeds / 300-300 pass termination criterion" **were not verified in this check**, this paper does not cite them as facts, all `[RESULT NEEDED]`. This paper only adopts the two structural facts of its **bookkeeping-unit design and governance structure**.

### 4.2 Governance: Why an Open Market Needs "Two Chambers"

A closed system's governance is UDOS itself having the final say (P3's circuit breaker, P12's evidence grading). An open market has no single authority, so governance itself becomes a designed object. ISONOMIA's bicameral governance gives a simple but deep tradeoff: **purely weighting by prestige** lets governance be monopolized by a few strong players (the strong always win); **purely equal by lot** lets governance be hostage to the average level. The bicameral compromise is—one side listening to "those who did more, did better," one side listening to "one person one vote."

UDOS does not claim to copy bicameral governance, but incorporates into the open-market design map the judgment that "**the governance structure itself needs to be designed, rather than an authority existing by default**."

---

## 5 Settlement Neutrality: A UDOS-Authored Design Goal (with evidence gap statement)

### 5.1 Why a Separate Section, and Must First State Its Source

The user's clue once contained the claim of "five attributes of settlement neutrality" (escrow determinism, ledger consistency, reputation equivalence, dispute reciprocity, auditability). **After online verification, no original paper or Zenodo record of this claim was found** (ledger failed). Therefore this paper **does not cite it as an existing academic framework**, but explicitly rewrites it as a **UDOS P11/P25-authored engineering design goal**: when settlement moves from "a single closed system" to "a cross-system open market," UDOS wants the settlement protocol to satisfy the following engineering properties. The five below are **UDOS-authored**, marking `[CITATION NEEDED]` where academic backing is needed:

1. **Ledger consistency (UDOS-authored)**: from whichever Agent's perspective, the same settlement points to the same set of facts on the ledger; conservation such as `balance_sum = total_paid − slashed` still holds after cross-system netting.
2. **Acceptance-dependent settlement (UDOS-authored)**: payment occurs only after work is accepted, no positive settlement without acceptance (consistent with P11 "pay only on acceptance").
3. **Unique-completer payment (UDOS-authored)**: the same task settles only with one accepted completer, duplicate submissions produce no duplicate payment (P11 verified).
4. **Reputation derived from behavior (UDOS-authored)**: reputation is derived from real settlement outcomes, cannot be bought or transferred (borrowing ISONOMIA's non-transferable reputation).
5. **Auditability (UDOS-authored)**: each settlement can be traced along the trace back to acceptance evidence and contract fields (P10's hash-chain Trace).

These five are not "theorems academia has proven," but engineering expectations UDOS **induces** from P11's conservation, P10's auditability, ISONOMIA's credit units. Writing them as a list is to give later implementation a target, not to claim it is already consensus.

### 5.2 Demarcation from the "Token Economy / Agent Currency" Narrative

P11 already made clear: UDOS's market is an **internal bookkeeping unit**, not linked to any token/financialization, no real pricing game, no Sybil defense. P25 extends the view to the open market, but **this demarcation is unchanged**: this paper discusses "mechanism design engineering," not "issuing tokens." Any industry narrative of "issuing currency to Agents, letting the market self-regulate" needs Sybil defense, price discovery, external trust anchors, far beyond this paper's scope, this paper not mutually endorsing it.

---

## 6 Drawbacks / Failure Boundaries / Falsifiable Conditions

**Its drawbacks and limitations:**

1. **VCG's "elegance trap."** The easiest error is "since VCG makes truth-telling optimal, then fully adopt VCG." Ausubel–Milgrom long warned: VCG is compute-expensive, collusion-prone, not budget-balanced. If UDOS hard-adopts strict VCG in the open market, it falls into the trap of "theoretically perfect, engineering unable to run." This paper insists on "approximate VCG + reputation correction," not pursuing strict dominance.
2. **Private information ≠ designable.** AgentLance's mechanism works in its experiments, but "private cost" in a real Agent ecosystem may be manipulated more complexly (Agents can falsely report their own strategy notes). VCG-style payment suppresses "bid lying," not "the strategy notes themselves being polluted."
3. **ISONOMIA's numbers cannot be trusted as facts.** Its ergs/bicameral governance is structural design, but its "45,000 simulations all pass" is an unverified number, this paper not citing. Whether a bookkeeping-unit design can really stabilize in an open economy is itself an open question.
4. **Settlement neutrality is an authored goal, not a theorem.** Section 5's five are UDOS's expectations, no academic backing, nor proven "satisfying these five means safe." Taking them as design targets is fine, as safety guarantees is not.
5. **Open market ≠ free market.** This paper still discusses a controlled open market that is "auditable, contracted, accepted," not "unregulated free competition." Once acceptance and contracts are removed, all properties fail.

**Falsifiable conditions:**

- **F1**: if in open-market simulation, VCG-style payment + reputation correction does **not** raise the proportion of "truth-telling bids" (relative to the cost-performance award baseline), then the judgment "private cost must be treated with incentive compatibility" is wrong in the Agent scenario, should fall back to cost-performance award.
- **F2**: if after cross-system netting, the conservation invariant (`balance_sum = total_paid − slashed`) is frequently violated in real settlement, then "settlement neutrality" as an engineering goal is unrealizable, UDOS should shrink back to the closed internal market.
- **F3**: if liquidity-adaptive degradation (auctions when many, fallback when few) in simulation is instead worse than fixed rules due to "degradation too coarse," then "liquidity awareness" is over-design.

---

## 7 Conclusion and Honest Boundary

P11 answered "how a closed internal market balances the books," P25 pushes the view to "how an open Agent market dispatches, keeps accounts, governs." Three orthogonal lines—incentives (VCG-style making truth-telling pay), allocation (adaptive degradation with liquidity), bookkeeping governance (ergs credit unit + bicameral governance)—together form a design map. On it, UDOS authors "settlement neutrality" as an engineering goal, and honestly states it lacks academic backing `[CITATION NEEDED]`.

All new designs in this paper are **design proposals · to implement**: whether VCG-style payment really raises truth-telling proportion in the open Agent market, whether cross-system conservation holds, whether liquidity degradation is better—all `[RESULT NEEDED]`. UDOS does not pursue making the market "the theoretically prettiest auction," but an engineering-reproducible market that "balances the books, degrades stably, is auditable." This is P25's increment for v7.6.0.

---

## References

**External dialogue objects (existence verified online 2026-09-21; numbers are paper reports or record actual values)**

1. AgentLance: Markets, Not Planners: Decentralized Orchestration of LLM Agents with Private Information. arXiv:2608.23867 (private-cost bidding, strategy notes, VCG-style payment; its beating single models/centralized allocation is a paper report).
2. Agent Exchange: Shaping the Future of AI Agent Economics. arXiv:2507.03904 (RTB-style multi-attribute auctions, generalized second price, greedy/random degradation when liquidity insufficient).
3. ISONOMIA: A Constitutional Design for Autonomous Agent Labor Markets. Zenodo records/**21338480** (caution, correctly recorded; ergs mutual-trust credit unit net zero, non-transferable reputation, bicameral governance; numbers such as "45,000 simulations" not verified, `[RESULT NEEDED]`).

**Classic background**

4. Vickrey, W. (1961). Counterspeculation, Auctions, and Competitive Sealed Tenders. *Journal of Finance*.
5. Ausubel, L. M., & Milgrom, P. (2006). The Lovely but Lonely Vickrey Auction. (the classic warning of VCG "theoretically elegant, practically lonely"; relayed from P11).

**UDOS paper volumes**

6. UDOS v7.5.0: P11 "Settlement conservation and Byzantine slashing mechanism design of the internal task market" (conservation invariant, 10 unit tests verified, clean/stressed both conserved=true), P10 "TransferBundle handoff contract" (hash-chain Trace, 26 tests verified).
7. UDOS v7.6.0: P17 "Least-privilege orchestration" (winners execute only within scope), P18 "Broad-deep two-stage memory flywheel" (strategy notes and memory flywheel isomorphic), P34 "Literature evidence ledger."

---

## Evidence Discipline and Reproduction Notes

- This paper is a **design survey + proposal**, no new UDOS measurements; VCG-style payment, liquidity degradation, the five settlement neutrality items are all design proposals, benefit positions `[RESULT NEEDED]`.
- The "five attributes of settlement neutrality" **has no traceable original source** (ledger failed), this paper does not cite it as an existing framework, rewriting it as a UDOS-authored design goal and marking `[CITATION NEEDED]`.
- ISONOMIA is correctly recorded as Zenodo 21338480 (not the user-given 21287289); its simulation numbers were not verified, all not written as facts.
- AgentLance/Agent Exchange's "beating baselines" is a paper report, not independently reviewed; not mutually endorsed with any "Agent token economy" industry narrative.


---

<p align="center"><img src="assets/logo.png" width="180" alt="TwinsEarth"/></p>

# Causal Validity of Large-Scale Social Simulation: Phenomenon Reproduction Is Not Mechanism Verification

**Working Title (EN):** *Causal Validity of Large-Scale Social Simulation: Why Phenomenon Reproduction Is Not Mechanism Verification, and Where UDOS RCT Sits on the Validity Ladder*

> Volume paper P27 · the methodology dialogue paper of P3 "Fault governance RCT" and P20 "Heterogeneity dividend" · UDOS Reasoning Engine v7.6.0 · Fang Wenxin · 2026-09-21
>
> **One sentence first**: million-scale Agent social simulations such as OASIS, AgentSociety can "reproduce" known social phenomena like group polarization and information diffusion, but "reproducing a phenomenon" and "proving the causal mechanism behind the phenomenon" are two different things—the former only shows your simulator looks like, the latter needs controlled comparison. The reason UDOS's RCT (P3) deliberately keeps fault injection and comparison arms is precisely to push simulation from "phenomenon reproduction" to the validity level of "hypothesis testing."
>
> **Evidence caliber (stated only once in the whole paper)**: this paper is a methodology/validity analysis paper, containing no new UDOS measured numbers; cited UDOS conclusions all go back to v7.5.0 P3/P9. External literature is verified per EVIDENCE_LEDGER_v76.md: OASIS (arXiv:2411.11581, NeurIPS 2024) million-user simulation, reproducing three social phenomena is a verified fact, but its evidence nature is **phenomenon reproduction**; AgentSociety (arXiv:2502.08691) 10,000+ Agents, about 5 million interactions is verbatim verified; MegaAgent (ACL 2025 Findings, 2025.findings-acl.259) 590 Agents national-level policy simulation, making Gomoku in 800 seconds is verbatim verified. AgentTorch / On the Limits of Agency are cited only by "direction/conference existence," their concrete scale numbers not verbatim verified in this volume, not quantitatively cited.

---

## Abstract

2024–2026, LLM-driven large-scale social simulation (Agent-Based Model, ABM) entered the million scale: OASIS supports up to million-user simulation and reproduces information diffusion, group polarization, herding; AgentSociety ran 10,000+ Agents, about 5 million interactions; MegaAgent under no predefined SOP expanded autonomous multi-Agent to 590 for national-level policy simulation. This paper proposes a methodological distinction often masked by such works' "scale narrative": **phenomenon reproduction ≠ mechanism verification**. Reproducing a known phenomenon (polarization, herding) only shows the simulator's output is statistically shaped like real data, not that the causal mechanism producing the phenomenon is Agent interaction itself—because in ABM the Agent behavior rules themselves may have been "calibrated" to reproduce that phenomenon. This paper gives ABM's three-level validity ladder: **exploratory level** (discovering emergent phenomena), **phenomenon reproduction level** (reproducing known statistical laws), **hypothesis testing level** (identifying causal effects under controlled comparison), and argues UDOS P3's RCT (fault injection, comparison arms, circuit breaker reassignment) sits at the highest level. Finally points out social simulation's correct use for UDOS: it is a hypothesis generator, not a hypothesis verifier; using it as a verifier is the most dangerous leap in policy inference.

**Keywords**: Agent-Based Model; social simulation; phenomenon reproduction; causal identification; validity grading; hypothesis testing; RCT

---

## Structured Abstract (background problem → argument → evidence → contribution)

- **Background problem**: million-scale Agent social simulation is increasingly cited as "evidence" by policy inference. But between "the simulator ran out polarization" and "polarization is caused by X" lies the whole causal inference methodology.
- **Core argument**: ABM validity has three levels—exploration / phenomenon reproduction / hypothesis testing; 2026 mainstream million-scale social simulation (OASIS/AgentSociety) sits at the first two, providing no causal identification; UDOS P3's RCT is the third.
- **Evidence clues**: (1) OASIS reproducing three phenomena (paper report, phenomenon reproduction nature); (2) AgentSociety 10,000 Agents/5 million interactions; (3) MegaAgent 590 Agents policy simulation; (4) the basic causal inference distinction (correlation vs intervention).
- **Contribution**: explicitly grading ABM validity into three levels; pointing out the circular-reasoning risk of "reproduction = verification"; drawing for UDOS the division "simulation = hypothesis generation, RCT = hypothesis verification."

---

## 1 Introduction: Reproducing a Phenomenon, What Does It Prove

A social simulator ran out "information diffusing in the Agent network like a real social platform, group polarization appearing." This sounds like a strong result. But what does it actually prove?

Which of two things does it prove? (a) This simulator **can reproduce** the statistical shape of known social phenomena; or (b) this simulator **reveals** the causal mechanisms behind these phenomena?

These two are methodologically far apart. (a) is fitting—you tune Agent behavior rules until they reproduce that phenomenon, even in reverse: because you **know** what polarization looks like, you calibrate rules in that direction, finally of course reproducing polarization. (b) is identification—you must prove in controlled comparison that changing some variable indeed caused the phenomenon to change, while excluding other explanations. 2026's million-scale social simulation, the vast majority of honest papers only claimed (a), but citers often read it as (b). This paper makes this leap explicit.

---

## 2 What 2026's Large-Scale Social Simulation Is Doing

### 2.1 Three Verified Representative Works

Per EVIDENCE_LEDGER_v76.md (2026-09-21):

- **OASIS** (arXiv:2411.11581, NeurIPS 2024): based on the social media paradigm, supporting dynamic social networks of up to million users, diverse action spaces and recommendation systems; reproduced three phenomena—information diffusion, group polarization, herding. Its evaluation nature is **phenomenon reproduction**.
- **AgentSociety** (arXiv:2502.08691): 10,000+ Agents, about 5 million inter-Agent/Agent-environment interactions, studying four social issues such as polarization, diffusion.
- **MegaAgent** (ACL 2025 Findings, 2025.findings-acl.259): no predefined SOP, dynamically generating Agents and communication structures, expanding to 590 Agents in national-level policy simulation, and independently making the Gomoku game in 800 seconds.

Additionally, AgentTorch (AAMAS 2025) proposes the Large Population Models (LPMs) paradigm, On the Limits of Agency (AAMAS 2025 Oral) studies the limits of Agent autonomy—this volume cites only by direction, not verbatim citing their scale numbers.

### 2.2 Their Common Evidence Nature

The common point of these three works is: they show "LLM-driven Agent clusters at large scale can run out presentable social dynamics." This is important **exploratory** capability—previously ABM could only do a few hundred Agents with hand-written rules, now using LLM behavior can do tens of thousands. But their answer to "why this phenomenon appears" usually stays at "because our Agents interact by these rules," rather than "because variable X was proven by controlled comparison to cause Y."

---

## 3 The Three-Level Validity Ladder

![Figure 1 The three-level ABM validity ladder](figures/P27_fig1_validity_ladder.png)

*Figure 1 Exploratory level (discovering emergence) → phenomenon reproduction level (reproducing known statistical laws) → hypothesis testing level (controlled comparison identifying causality). OASIS/AgentSociety sit at the first two, UDOS P3 RCT sits at the third. Conceptual illustration.*

This paper explicitly divides ABM validity into three levels:

**Table 1 Three levels of ABM validity**

| Level | Question asked | Representative work | Can answer "why" |
|---|---|---|---|
| L1 Exploratory | What will this set of Agents emerge? | MegaAgent policy simulation | no, only describes emergence |
| L2 Phenomenon reproduction | Can it reproduce known phenomena? | OASIS reproducing three phenomena, AgentSociety | partially, proves fitting looks like |
| L3 Hypothesis testing | Does changing X causally cause Y? | UDOS P3 fault-injection RCT | yes, controlled comparison identifies the effect |

### 3.1 L1→L2: From "Moves" to "Looks Like"

L1 only asks what emergent behavior the system ran out. MegaAgent under no SOP generating 590 Agents and doing policy inference, writing Gomoku in 800 seconds, this itself is proof of emergent capability, but it is not responsible for "whether this policy inference is right."

L2 requires output matching the statistical shape of real data. OASIS compares simulated output with real social media data, reproducing the distribution features of three phenomena—this is stronger than L1, but it answers "looks like or not," not "why."

### 3.2 L2→L3: From "Looks Like" to "Causal"—This Is the Hardest Leap

L3 requires controlled intervention: holding other conditions fixed, changing only one variable, observing whether the outcome systematically changes, and giving confidence intervals. This is the basic requirement of causal inference, also the weakest link of LLM-ABM—because ABM's "experiment" itself is done in a world where you can tune parameters arbitrarily, you can fully make results go your desired direction by tuning.

**The key risk is circular reasoning**: if when calibrating Agent rules you already referenced the real-world polarization distribution, then "the simulator reproducing polarization" is not an independent discovery, but you writing the answer into parameters. This is not saying OASIS-like works are cheating, but its evidence strength cannot exceed "a well-fitted phenomenon reproducer."

### 3.2.1 Three "Looks Like an Experiment" Pseudo-L3

In practice, three practices are often mistaken for causal identification, must be exposed:

1. **Post-tuning fitting = causality.** Tuning Agent behavior rules until they match the real data distribution, then claiming "the model explains polarization." This is only fitting, the rules themselves may not be polarization's real mechanism at all.
2. **Single-arm demonstration = effect.** Running one large-scale simulation, showing a dramatic emergence (such as group split), then claiming "some variable causes split." No comparison group, cannot exclude randomness or initial-condition sensitivity.
3. **Sensitivity analysis = robustness.** Sweeping a few parameters to see whether emergence still appears, then claiming the conclusion robust. Sensitivity analysis only shows "changing parameters the phenomenon remains," not "changing some variable indeed caused the effect."

Real L3 requires **comparison design + repeatability + confidence intervals**, UDOS P3's fault-injection comparison arms precisely align with these three.

### 3.2.2 External Validation: Anchoring Simulation to the Real World

One way out of circular reasoning is externally validating simulation results against real-world **quasi-experiments** or historical policy comparisons. If after changing some variable the LLM-ABM's predicted effect direction and magnitude agree with a real-world natural experiment's estimate, it moves up from L2 to near L3. But 2026's mainstream million-scale social simulation mostly has not done this external anchoring—their "validation" still stays at "consistent with historical statistical shape," rather than "consistent with the real causal effect of an intervention." This is precisely why this paper overall places them at L2.

---

## 4 Why UDOS P3's RCT Sits at L3

### 4.1 The RCT's Comparison Group and Intervention

UDOS P3 (fault governance RCT)'s design deliberately aligns with L3 requirements: it is not "running a multi-Agent system once to see results," but **comparison arms under fault injection**—in an end-to-end task of 24 work orders, 9 specialists, 7 QA (including 2 Byzantine), 3 fault specialists, the layered hybrid topology with circuit breaker reassignment achieves 24/24 closing, all 3 fault nodes isolated, 5 reworks (cpu-proto). Here the "intervention" is injecting faults, the "comparison" is different topology/circuit breaker strategies, the "outcome" is completion rate. It wants to answer "whether circuit breaker reassignment **causes** completion rate to rise," not "whether the system running looks like."

### 4.2 Simulation vs RCT Division

Thereby obtaining UDOS's correct positioning of social simulation:

- **Large-scale social simulation (OASIS/AgentSociety) = hypothesis generator.** It cheaply tells you "in this region of parameter space, what interesting phenomena emerge," helping you propose hypotheses worth testing.
- **Controlled RCT (P3) = hypothesis verifier.** It expensively tells you "under strict comparison, whether this intervention really caused that outcome."

Taking simulation as a verifier is the most dangerous leap in policy inference—you will mistake "I saw X in the simulator" for "X will happen in the real world."

---

## 5 Echo with P20: Phenomenon Reproduction Does Not Identify Causality Between Individuals

P20 already pointed out, OASIS observes "larger Agent groups bring stronger group dynamics and more diverse opinions," but this is phenomenon reproduction, not answering "how much increasing N causes opinion diversity to change." This paper generalizes this: OASIS-like works can show "scale and diversity correlated," but to jump from correlation to "scale **causes** diversity" needs L3 controlled comparison—e.g. fixing other conditions, changing only the Agent count, measuring systematic changes in opinion distribution. OASIS did not do this, so its "larger more diverse" is observation, not causal effect estimation.

Worth adding the methodological why: ABM is inherently "forward simulation"—you input rules, it outputs behavior. Reverse-inferring from output "which rule caused which behavior" under multi-Agent nonlinear interaction is ill-posed, because rules entangle with each other, initial conditions are sensitive, emergence is nonlinear. To solve this ill-posed problem, the only clean way is forwardly doing controlled intervention: moving one rule at a time, seeing whether the outcome moves. This is precisely the fundamental division of RCT and ABM—ABM is responsible for forward exploration of "what happens," RCT is responsible for counterfactual verification of "was it this thing that caused it."

Pushing this division into policy inference practice: using OASIS-like simulation to scan out "under some policy parameter group split may appear," this is a valuable risk warning; but based on it pushing that policy parameter all the way in some direction and claiming "this avoids split," crosses the validity boundary—because the split in simulation may come from the Agent rules you wrote, not the policy itself. The safe practice is taking the direction simulation hints, going back to the real world to find a quasi-experiment or small-scale pilot for L3 validation.

---

## 6 Drawbacks, Failure Boundaries, and Falsifiable Conditions

**6.1 Drawbacks and circulating exaggerations**

1. **"Reproduced polarization" read as "proved the polarization mechanism."** This is ABM narrative's most common circular reasoning: the rules are written by you, the phenomenon is what you want to reproduce, reproduction success does not constitute independent evidence.
2. **"Million scale" read as "conclusion more credible."** Large scale only raises the richness of emergent phenomena, not the strength of causal identification; a million-scale well-fitted phenomenon reproducer's evidence strength remains lower than a few-dozen-node controlled RCT.
3. **Taking simulation output directly as policy advice.** Before doing L3 controlled comparison, social simulation can only "hint risk direction," cannot "give policy dosage."

**6.2 Failure boundaries**

- This paper does not deny large-scale social simulation's exploratory value; it is a sharp tool for hypothesis generation. This paper only draws its evidence applicability boundary for **causal policy conclusions**.
- If some future ABM work under controlled intervention gives causal effect estimates consistent with real-world quasi-experiments, then that work crosses into L3, this paper's "ABM mostly at L2" judgment needs upgrading.

**6.3 Falsifiable conditions**

- **C1**: if OASIS/AgentSociety-like works are proven their Agent behavior rules calibrated independently of the target phenomenon (i.e. not referencing polarization distribution for tuning), and output consistent with real data in held-out periods, then their evidence strength can be upgraded from L2; conversely if rules are found repeatedly tuned to reproduce phenomena, then circular reasoning risk is confirmed.
- **C2**: if a purely L1/L2 social simulation is used to give policy-binding "dose-effect" conclusions (rather than directional hints), and afterward falsified by real policy implementation, then this paper's "simulation ≠ verification" demarcation is strengthened.

---

## 7 Hard Predictions (2026–2031)

- **F1**: in the next five years, the LLM-ABM field will appear review standards specifically distinguishing "phenomenon reproduction" and "causal identification"; papers still taking reproduced phenomena directly as policy evidence will be systematically downgraded.
- **F2**: the first LLM-ABM conclusion accepted by the policy community must simultaneously attach an L3-level controlled comparison or external validation with a real-world quasi-experiment; purely phenomenon-reproducing "million-Agent policy inference" will no longer alone constitute a decision basis.

---

## 8 Conclusion

OASIS, AgentSociety, MegaAgent pushed LLM social simulation from a few hundred rule Agents to tens of thousands of LLM Agents, this is real progress. But progress happens at "how large a scale can be simulated," not at "how hard a causality can be identified." Reproducing a phenomenon only shows the simulator tuned to look like, not that you found the mechanism behind the phenomenon—the latter needs controlled intervention, comparison arms, confidence intervals, i.e. UDOS P3-style RCT.

For engineering, this means large-scale social simulation's correct position in the UDOS system is **hypothesis generation**: it cheaply scans parameter space, hints directions worth testing; upgrading directional hints into decidable causal conclusions must return to RCT. For research, it gives a killable standard (C1–C2): any ABM claiming mechanism discovery must prove its rules were not reverse-calibrated by that mechanism's target phenomenon.

**The simulator can run out the world's appearance, which does not mean the simulator found the world's cause. Taking the former as the latter is replacing experiment with fitting.**

---

## References

**2026 frontier material (existence verified per EVIDENCE_LEDGER_v76.md 2026-09-21; numbers all paper reports)**

1. Yang, et al. (2024). OASIS: Open Agent Social Interaction Simulations with One Million Agents. arXiv:2411.11581, NeurIPS 2024.
2. Piao, et al. (2025). AgentSociety: Large-Scale Simulation of LLM-Driven Generative Agents. arXiv:2502.08691.
3. (MegaAgent Team) (2025). MegaAgent: A Large-Scale Autonomous LLM-based Multi-Agent System Without Predefined SOPs. ACL 2025 Findings, 2025.findings-acl.259.
4. AgentTorch / Large Population Models. AAMAS 2025. (direction citation, scale numbers not verbatim verified)
5. On the Limits of Agency in Agent-based Models. AAMAS 2025 Oral. (direction citation)

**UDOS paper volumes (this series, evidence grades marked in text)**

6. UDOS v7.5.0: P3 "Circuit breaker reassignment RCT for multi-agent fault governance," P9 "Quality saturation law of the Agent legion."
7. UDOS v7.6.0: P20 "Heterogeneity dividend and homogeneity saturation," P21 "Million-level coordination end-to-end evidence and extrapolation boundaries."

---

## Evidence Discipline and Reproduction Notes

- This paper is a methodology/validity analysis paper, no new experiments; UDOS numbers go back to P3/P9 and `reports7/`, evidence grades subject to each paper's marking.
- OASIS/AgentSociety/MegaAgent existence and scale numbers are verified per EVIDENCE_LEDGER_v76.md; AgentTorch and On the Limits of Agency are cited only by direction, not verbatim citing scale numbers. All external numbers keep the "paper report" caliber.
- This paper's predictions (F1–F2) and falsification conditions (C1–C2) can be post-hoc scored with public evidence during 2026–2031.


---

<p align="center"><img src="assets/logo.png" width="180" alt="TwinsEarth"/></p>

# The v7.6.0 Master Architecture: Unified Specifications and Backward-Compatible Rollout of Eight Structural Changes to the UDOS Engine

> Whole-volume closing master outline P33 · consolidating the eight structural upgrades across the seventeen papers P16–P32 into one unified change specification · compatible with the v7.5.0 master · UDOS Reasoning Engine v7.6.0 · Fang Wenxin · 2026-09-21
>
> **One sentence first**: v7.6.0 is not "yet another system swap," but **eight structured incremental changes** on the v7.5.0 engine — upgrading stop decisions from binary to three-level, adding a least-privilege scope to the handoff contract, changing the data flywheel into a breadth–depth two-stage process, adding a test-assertion gate to structural changes, and uniformly registering the four external evidences of external literature plus one "rules-first" red line into the change ledger. This paper writes these eight as landable field/state-machine/gate specifications and **repeatedly declares: they are design proposals · to be implemented, not measured returns already run**.
>
> **Evidence scope (stated only once in the whole paper)**: this paper is an architecture-specification document and **contains no new UDOS measured figures**. The motivation for the eight changes points item by item back to external papers (existence verified on 2026-09-21 per `EVIDENCE_LEDGER_v76.md`), where the performance figures of external papers are uniformly "paper-reported/company-disclosed, not independently reviewed"; existing figures on the UDOS side only point back to the v7.5.0 papers and `udos-engine/reports7/*.json` (verified / cpu-proto / cpu-proxy / unverified). All fields, state machines, gates, and thresholds this paper proposes are **design proposals · to be implemented**, and any expected return without experimental support is marked `[RESULT NEEDED]`, forbidden to be written as a measured conclusion. Verification date 2026-09-21.

---

## Abstract

The v7.5.0 engine has answered the master proposition "structural priors determine the capability ceiling" (P0–P15), but its engineering interfaces remain **binary, single-point, ungated**: stop decisions have only the two states STOP/CONTINUE (P2), the handoff contract has only the owner field (P10), the data flywheel only asks "where has not yet been seen" (P4), and structural changes rely on people remembering to write tests (the P12 version-number incident is the counterexample). A batch of external works in the first half of 2026 (H-CSC, ClawArena-Team, RSIAgent, Ouroboros, OASIS, MAPF-GPT-DDG) exposed the roughness of these interfaces from different directions and also gave borrowable structural solutions. v7.6.0 consolidates these lessons into eight **backward-compatible structural changes**: ① three-level typed finality (P16), ② TransferBundle adds a scope field (P17), ③ BRS+DRS two-stage memory flywheel (P18), ④ structural-change review gate (P19), ⑤ external-verification registration of the heterogeneity dividend (P20, citing OASIS), ⑥ downgraded citation of MAPF million-level conclusions (P21, downgraded per ledger A8), ⑦ explicit marking of RSI lineage levels (P24, none called L5), ⑧ the design red line of rules over learning (P32). This paper gives for each: motivation source, pseudocode-level specifications of fields/state machines/gates, the compatibility path with v7.5.0, and the to-be-implemented measurement items `[RESULT NEEDED]`. The whole paper creates no new measured figures; the eight changes form a unified change set of "design proposals · to be implemented," whose success or failure is scored afterward by the five falsifiable conditions F6–F10 registered in P34.

**Keywords**: architecture specification; typed finality; least privilege; data flywheel; review gate; backward compatibility; design proposal; falsifiability

---

## Structured abstract (background problem → method/thesis → evidence → contribution)

- **Background problem**: v7.5.0 proved "structure determines the ceiling," but its own interfaces remain coarse-grained, without least privilege, without change gates; the 2026 external papers both verified UDOS's core proposition and exposed the cost of making interfaces coarse.
- **Core thesis**: v7.6.0 does not rewrite the engine, only makes **eight backward-compatible structural increments**; each increment traces to a specific finding of an external paper and has a clear old-version default backstop (default falls back to v7.5.0 semantics).
- **Evidence clues**: the eight motivations come respectively from H-CSC v2's negative result (lexical predicates beat learned encoders), ClawArena-Team's permission bottleneck, RSIAgent's broad-then-deep strategy, Ouroboros's reviewed commits, OASIS/P9/TUMIX's heterogeneity comparison, MAPF-GPT-DDG's extrapolation boundary, and Theseus's RSI discrimination criteria; all marked per the ledger's three states.
- **Contribution**: a unified change specification at field/state-machine/gate level; a v7.5.0→v7.6.0 change-comparison table; a discipline of "new-architecture returns uniformly marked to-be-implemented"; and the interface with P34's forecast updates (F6–F10).

---

## 1 Why v7.6.0 is "incremental," not a "rewrite"

Reading the seventeen v7.5.0 papers (P0–P15) yields a counter-intuitive but important conclusion: the **theoretical skeleton** of the UDOS engine does not need changing. Structural priors determine the capability ceiling (P0b), hierarchical hybrid topology presses fan-in from $O(N)$ to $O(\log N)$ (P1), equal-weight $2f+1$ BFT-lite stop decisions (P2), homogeneous-expansion saturation (P9), and six-field handoff-contract validation (P10) — these were supported in v7.5.0 by controlled comparisons or property tests.

What really needs upgrading is **interface granularity and governance**. The 2026 external works forced this judgment from five directions:

1. **Stop decisions too coarse**: H-CSC v2 (arXiv:2606.07316) points out that LLMs produce random natural language each round, and binary STOP/CONTINUE compresses "how semantically consistent" into one switch, losing gradient information.
2. **Handoff permissions too loose**: ClawArena-Team (arXiv:2606.31174) finds the first bottleneck of multi-agent orchestration is not "cannot understand" but "over-authorization."
3. **Data collection asks only breadth**: RSIAgent (arXiv:2609.15364) uses "broad-then-deep" to prove that only covering blind spots without focusing on hard cases means memory compound interest cannot roll up.
4. **Structural changes ungated**: Ouroboros (arXiv:2608.08311) proves "reviewed commits" must become the foundation of subsequent runtime; conversely the P12 version-number incident is precisely the counterexample of "no review gate."
5. **External narratives need discipline**: the "million-level" narratives of OASIS, MAPF, Warp-Cortex must distinguish "measured" from "theoretical extrapolation/media retelling" per the evidence ledger, otherwise engineering ambition is disguised as engineering fact.

Therefore v7.6.0's positioning is: **retain all v7.5.0 behavior, overlay optional structures at eight interface points, and ensure default falls back to old behavior**. This is consistent with P32's "rules as base, learning as supplement" orientation — not overturning verified old paths, only adding guardrails on them.

![Figure 1: v7.5.0 and v7.6.0 eight-change comparison](figures/P33_fig1_change_map.png)

*Figure 1 From v7.5.0's coarse-grained interfaces (left column) to v7.6.0's structured increments (right column), the eight changes correspond one to one. Conceptual schematic, not measured data.*

---

## 2 Specifications of the eight structural changes

The eight below are uniformly given in a four-part set: **motivation (which finding of which external paper) → structural-change specification (pseudocode-level description of fields/state machines/gates) → compatibility path (how default falls back to v7.5.0) → `[RESULT NEEDED]` to-be-implemented items**. The full argument for each is in the corresponding dedicated paper (P16–P32); this paper only does unified registration and specification closure.

### 2.1 Change ①: three-level typed finality (P16)

**Motivation**: H-CSC v2 (arXiv:2606.07316, retitled *Certifiable Semantic Agreement... What the Admissibility Instrument Decides*) uses typed commit objects to solve "LLMs produce random natural language, unable to agree at byte level"; its v2 also **actively retracts** v1's "honest-retention advantage" claim (aggregate distance 5.32°/5.48° vs the steelman baseline 4.27°/4.32°, honest retention 0.6475 vs 0.7325; all paper-reported, not independently reviewed) and finds deterministic lexical predicates in adversarial scenarios at AUROC 0.865–0.982 outperform learned encoders at 0.621–0.744.

**Specification (stop-decision state machine)**: upgrade v7.5.0 P2's binary `{STOP, CONTINUE}` to the three states `{strong_stop, weak_stop, abort}`:

- `strong_stop`: ≥$2f+1$ members agree on the **semantic core** (judged by deterministic lexical/rule predicates, not learned encoders);
- `weak_stop`: ≥$2f+1$ members agree only on the **verdict label** (pass/fail);
- `abort`: neither level reached, triggers view change (using P2's existing mechanism).

Pseudo state transitions:

```
round r vote -> rule predicate judges semantic-core agreement?
  yes (≥2f+1)            -> strong_stop  commit semantic summary
  no but label agree (≥2f+1) -> weak_stop  commit only verdict
  no                     -> abort         view change / hand to human
```

**Compatibility path**: old work orders and old tests default to mapping "recognize only strong_stop, weak_stop treated as CONTINUE, abort treated as failure," i.e. behavior degenerates to v7.5.0 binary semantics; new work orders explicitly declare `finality_mode=typed` to enable three states.

**To implement**: `[RESULT NEEDED: H3-new monotonicity, weak_stop unbounded-loop risk, honest-retention difference under steelman comparison]`.

### 2.2 Change ②: TransferBundle adds a scope field (P17)

**Motivation**: ClawArena-Team (arXiv:2606.31174, existence verified) reports the management bottleneck of multi-agent orchestration concentrates on permission granting, with cost approximately decoupled from management quality; its precise thresholds ("no LLM exceeds 50% permission accuracy," "cost hundred-fold/total score four-fold," "SMS three-term product formula") were not hit verbatim at abstract level, and this paper only borrows the direction, not hardcoding figures.

**Specification (handoff-contract new field)**: alongside v7.5.0 P10's six-field TransferBundle, add `scope`:

- `owner`: who is responsible (old field, retained);
- `scope`: the **set of stages this owner is allowed to operate** (new field).

Two invariants:

- On `advance`: when the current stage completes, `scope` automatically tightens to the singleton set of the next stage;
- On `handoff`: `scope` is re-granted by the new owner's authorized range; calls exceeding `scope` are directly rejected by rule predicates (`scope_violation`).

**Compatibility path**: default `scope=all` (i.e. no tightening), behavior equivalent to v7.5.0 with only owner; explicit `scope=stageset` enables least privilege. All 26 existing contract tests continue to pass.

**To implement**: `[RESULT NEEDED: scope factorial experiment — boundary-violation rejection rate, usability loss, false-tightening rate]`.

### 2.3 Change ③: BRS+DRS two-stage memory flywheel (P18)

**Motivation**: RSIAgent (arXiv:2609.15364) achieves training-free RSI with "broad-then-deep" memory construction, reported to make open-source Kimi-K3/GLM-5.3 surpass GPT-6-class closed-source models on specified benchmarks (paper/media reported, not independently reviewed); its "in New Environments" means learning in new environments, not cross-environment transfer.

**Specification (two-stage collection strategy)**: upgrade v7.5.0 P4's coverage-aware single-stage flywheel to two stages sharing the same memory body:

- **BRS (breadth range scan)**: use submodular greedy $(1-1/e)$ approximation to maximize state-space coverage;
- **DRS (depth range scan)**: focus on identifiability blind spots — specifically pointing to hard-to-identify parameters like the "acceleration scalar skill ≈ −0.508" already identified in P6 (single seed, cpu-proxy, not recomputed here).

Scheduling rule (design parameter): initial about 60/40 (BRS/DRS compute ratio), tilting toward DRS after coverage crosses threshold $\tau_{cov}$.

**Compatibility path**: the old flywheel remains "full coverage-aware"; two stages are a new switch `flywheel=two_stage`, default off.

**To implement**: `[RESULT NEEDED: $\tau_{cov}$ calibration, 60/40 ratio, blind-spot coverage improvement]`.

### 2.4 Change ④: structural-change review gate (P19)

**Motivation**: Ouroboros (arXiv:2608.08311) makes "reviewed commits" the subsequent runtime foundation, self-reporting Terminal-Bench 2.1 at 86.74% (original 86.97%, self-reported SOTA, not independently reviewed); the UDOS-side counterexample is the "version-number incident" recorded in P12 — structural changes merged without independent tests.

**Specification (merge gate)**: any commit changing `MatrixConfig` parameters must simultaneously meet three conditions to merge:

1. commit message tagged `[structural]`;
2. accompanied by **independent test assertions** (not reusing old assertions);
3. all CI green + new assertions cover the changed structure.

Any unmet condition blocks merge.

**Compatibility path**: the gate only blocks structural commits tagged `[structural]`; pure documentation/format changes are unrestricted, and old repository history is not retroactively traced.

**To implement**: `[RESULT NEEDED: the gate's interception rate for "silent failures/version-number-incident class," the cost to merge speed]`.

### 2.5 Change ⑤: external-verification registration of the heterogeneity dividend (P20, citing OASIS)

**Motivation**: OASIS (arXiv:2411.11581, NeurIPS 2024) observes with million-level **heterogeneous** social individuals "larger groups → stronger group dynamics/more diverse opinions"; this complements v7.5.0 P9's "homogeneous copies saturate no matter how many ($\text{mse}(k)=a+b/k$, cpu-proto)": P9 uses homogeneous copies to test saturation, OASIS uses heterogeneous individuals to test diversity.

**Specification**: this is a **citation registration and conclusion reinforcement**, not changing code interfaces. Register in the P9/P20 discussion: the quality variable is the independent-source count $D$, not headcount $N$; Warp-Cortex's Singleton Weight Sharing is in essence forced homogenization, solving only the memory wall, not the quality wall (see P22).

**Compatibility path**: pure documentation/argument-layer increment, no interface change, zero compatibility risk.

**To implement**: `[RESULT NEEDED: retest the P9 saturation law inside UDOS with homogeneous vs heterogeneous copy groups]`.

### 2.6 Change ⑥: downgraded citation of MAPF million-level conclusions (P21)

**Motivation**: the circulated "MAPF-GPT-DDG coordinates 1M agents on a 2048×2048 map, 99.9%, 163μs/agent" was verified per ledger A8 as **only seen in Russian media retelling**; the real paper *Advancing Learnable MAPF Solvers with Active Fine-Tuning* (arXiv:2506.23793, IROS 2025) only verified DDG active fine-tuning and predecessor small maps, 16–32 agents measured.

**Specification**: v7.6.0 whole volume **uniformly downgrades the scope** — only cite "DDG active fine-tuning + large-scale coordination direction," mark "million success rate/163μs" as media claim or deprecated; and clarify the extrapolation boundary: MAPF is geometric, fully observable, task-structured, while UDOS work orders are semantic, contain hidden ground truth and Byzantine faults, and the two cannot directly transfer (see P21).

**Compatibility path**: pure citation discipline; old P1 protocol-layer conclusions ($O(N)\to O(\log N)$, verified extrapolation) unchanged, only adding an "end-to-end evidence not extrapolatable" disclaimer.

**To implement**: `[RESULT NEEDED: if the MAPF paper's body later discloses million-level measurements, re-evaluate the extrapolation boundary]`.

### 2.7 Change ⑦: explicit marking of RSI lineage levels (P24)

**Motivation**: Theseus Labs (arXiv:2609.11873) gives seven RSI discrimination criteria and HCI; v7.5.0 P14/P15 already have the L1–L5 ladder. Externally any "gets better with use" is often called RSI, and a strict ruler is needed to place coordinates.

**Specification**: v7.6.0 in RSI discussion forces explicit level marking: RSIAgent=L2 (changes memory/Harness), Ouroboros=L2 (review-driven Harness change), Dream-RSI=L2–L3 (optimizes exploration-strategy structure, not weights), Gödel Agent≈L4 (changes self-referential source, secondary-material scope), HyperAgents closest to "recursive meta-improvement" but **did not reach L5**, RSIAI0 (correctly recording Zenodo 15644670)=L4 candidate. **None called L5**.

**Compatibility path**: pure terminology discipline; does not affect engine behavior.

**To implement**: `[RESULT NEEDED: if a system later shows controlled public evidence of "successor re-entering the loop," upgrade the level per F24-b]`.

### 2.8 Change ⑧: the design red line of rules over learning (P32)

**Motivation**: H-CSC v2's negative result (lexical predicates AUROC 0.865–0.982 beat learned encoders 0.621–0.744) holds in adversarial scenarios, suggesting "using learned semantic-judgment modules for safety verification" may be the wrong tool.

**Specification**: uniformly establish rules at the three safety-critical verification points P2/P10/P12 — **rules as base, learning as supplement**: first hold the bottom line with explainable, reproducible, CI-able deterministic rule predicates; learned models can only supplement after rules have held, and must pass steelman comparison to enter the safety path. P10's six-field mechanical validation and P2's $2f+1$ quorum judgment both remain pure rules.

**Compatibility path**: existing rule predicates untouched; "learning as supplement" is a new optional layer, default off.

**To implement**: `[RESULT NEEDED: the AUROC difference between rule predicates and learned predicates at each verification point under adversarial attack (see P34 F10)]`.

---

## 3 v7.5.0 → v7.6.0 change-comparison table

**Table 1 Quick reference for the eight structural changes (all design proposals · to implement)**

| # | Change point | v7.5.0 status | v7.6.0 specification | Motivation source (external paper) | Compatibility default | To-implement measurement |
|---|---|---|---|---|---|---|
| ① | Stop decision | Binary STOP/CONTINUE | strong_stop / weak_stop / abort three states | H-CSC v2 (arXiv:2606.07316) | Recognize only strong_stop, degenerates binary | H3-new monotonicity, unbounded loop `[RESULT NEEDED]` |
| ② | Handoff contract | Only owner | owner + scope (advance tightens, handoff re-grants) | ClawArena-Team (arXiv:2606.31174) | scope=all, equivalent old behavior | Boundary rejection rate, usability loss `[RESULT NEEDED]` |
| ③ | Data flywheel | Single-stage coverage-aware | BRS breadth + DRS depth two stages | RSIAgent (arXiv:2609.15364) | flywheel default single stage | Blind-spot coverage, ratio calibration `[RESULT NEEDED]` |
| ④ | Structural-change governance | People remember tests | [structural] gate + independent assertions | Ouroboros (arXiv:2608.08311) | Only blocks [structural] commits | Silent-failure interception rate `[RESULT NEEDED]` |
| ⑤ | Heterogeneity argument | P9 homogeneous saturation (internal) | Register OASIS heterogeneous diversity as external corroboration | OASIS (arXiv:2411.11581) | Pure argument layer | Homogeneous vs heterogeneous group comparison `[RESULT NEEDED]` |
| ⑥ | Million-level citation | Directly cite MAPF million | Downgrade to DDG direction + extrapolation boundary | MAPF-GPT-DDG (arXiv:2506.23793) | Only changes citation scope | Await paper body's million measurements |
| ⑦ | RSI terminology | L1–L5 ladder (P14/P15) | Six systems placed one by one on L1–L5, none L5 | Theseus (arXiv:2609.11873) | Pure terminology discipline | Await new "successor re-entry" evidence |
| ⑧ | Safety verification | P2/P10/P12 rules | Rules as base, learning as supplement, must pass steelman comparison | H-CSC v2 negative result | Existing rules untouched | Rules vs learning AUROC difference `[RESULT NEEDED]` |

**Table 2 Dependencies and orthogonal relations among changes**

| Change | Depends on | Orthogonal to | Note |
|---|---|---|---|
| ① Three-level finality | Inherits P2 | ⑧ rules-first (uses predicates to judge semantic core) | ① and ⑧ must be introduced in the same batch, otherwise strong_stop may misuse learned encoders |
| ② scope | Inherits P10 | ⑧ (scope_violation itself is a rule predicate) | Independently launchable |
| ③ Two-stage flywheel | Inherits P4/P6 | ①②④ | Data layer, orthogonal to consensus layer |
| ④ Review gate | Inherits P12 | All | Governance layer, independently launchable first |
| ⑤⑥⑦⑧ | Pure argument/discipline | — | No runtime dependency, documentation layer |

There is only one key constraint on dependencies: **① and ⑧ must be in the same batch**. If three-level finality is launched first without establishing the "rules as base" red line, engineers may well use learned encoders to judge strong_stop, precisely stepping on H-CSC v2's retraction lesson. This is the only strong coupling among the eight changes.

---

## 4 Compatibility-path overview

v7.6.0's compatibility strategy can be summarized in three sentences:

1. **Default is the old version**: all new interfaces have a default value falling back to v7.5.0 semantics (finality_mode=binary, scope=all, flywheel=single, no [structural] marking, rule predicates unchanged). Without opening new switches, behavior is byte-for-byte identical to v7.5.0.
2. **Switches made explicit**: new behavior must be opened by explicit configuration, producing no new assertions, branches, or attack surfaces before opening.
3. **Old tests do not regress**: v7.5.0's verified tests (P2's ten property tests, P10's twenty-six contract tests) fully regress in v7.6.0, ensuring increments do not break existing stock.

This interlocks with P19's review gate: any commit "changing default behavior" itself must be tagged `[structural]` and accompanied by assertions. That is, **v7.6.0 uses the gate it designed to constrain its own release** — a direct response to the P12 version-number incident.

---

## 5 Defects, failure boundaries, and falsifiable conditions

### 5.1 This master outline's own defects and boundaries

- **Not one of the eight changes has been measured effective**. This paper is a specification, not a scorecard. Reading "design proposals · to implement" as "already upgraded" is this volume's greatest misuse risk.
- **Backward compatibility ≠ zero risk**. The default backstop only guarantees "no change without opening," not "definitely better once opened"; weak_stop's unbounded loop, scope false-tightening hurting usability, and gates lowering iteration speed are all real costs (see dedicated chapters).
- **External motivation is only motivation, not evidence**. The figures of H-CSC, ClawArena, RSIAgent are mostly paper-reported/not independently reviewed; they can only show "others met problems in this direction," not substitute for UDOS's own steelman comparison.

### 5.2 Falsifiable conditions (aligned with P34's F6–F10)

This paper's eight specifications should be overturned or rolled back if the following occur:

- **C1 (three-level finality ineffective)**: if under fault-injection comparison, the three states show no statistically measurable improvement in closure rate/false-stop rate versus binary stopping, then ① should roll back to binary.
- **C2 (scope instead worse)**: if scope tightening makes normal-work-order usability loss exceed the boundary-violation rejection return, then ② should change to "on-demand authorization" rather than "default tightening."
- **C3 (two-stage flywheel no gain)**: if DRS depth probing shows no improvement over BRS-only in identifiability blind-spot coverage, then ③ should fall back to single stage.
- **C4 (gate idling)**: if the review gate neither blocks silent failures nor slows merge unacceptably, then ④ should be downgraded to "prompt" rather than "block."
- **C5 (rules red line pierced by counterexamples)**: if on some attack family learned predicates are stable and substantially better than deterministic predicates, then ⑧ "rules as base" needs weakening to "route by attack family."

---

## 6 Conclusion

All v7.6.0 increments are in essence making **one honest remedial lesson in interface granularity and governance** on the proposition "structural priors determine the ceiling" already verified in v7.5.0: stopping must be subdivided finer, permissions tightened more, data collected deeper, changes reviewed stricter, external citations marked clearer, and safety paths held harder. Not one of the eight changes overturns v7.5.0; all are backward-compatible with default backstops; the only strong coupling is that "three-level finality" must be introduced in the same batch as "rules as base," otherwise one falls a second time in the same pit.

**This is not a blueprint already redeemed, but a construction checklist with falsifiable conditions — the success or failure of each item is left to F6–F10 registered in P34 to score.**

---

## References

**UDOS v7.5.0 master paper volume (this series, evidence grades marked per paper)**

1. UDOS v7.5.0: P0 "Spatial Structure Itself Is Intelligence," P0b "Structure Plus Scale, Everything Obeys Scaling Laws," P0c "Metacognition and Boundary Calibration," P1 "Million-level Extrapolation of Coordination Complexity for Hierarchical Hybrid Topology," P2 "BFT-lite for Multi-Agent Stop Decisions," P4 "Coverage-aware Data Flywheel," P6 "Empirical Observability of Hidden Dynamic Parameters," P9 "The Quality Saturation Law of Agent Legions and the Failure Boundary of Homogeneous Expansion," P10 "Making TransferBundle Handoff Information Loss Measurable," P11 "Settlement Conservation in the Internal Task Market," P12 "Evidence-grade-driven Reproducible Systems Engineering," P14 "The Convergence of RSI," P15 "Life Originates in Structural Organization."

**UDOS v7.6.0 dedicated papers (full argument for the eight changes)**

2. P16 "Typed Finality: Three-level Semantic Commits and BFT-lite Stop-Decision Upgrade"; P17 "Least-privilege Orchestration: TransferBundle's scope Tightening and Re-granting"; P18 "Breadth–Depth Two-stage Memory Flywheel: BRS and DRS Submodular Collection"; P19 "Structural-change Review Gate: Test-assertion Enforcement for Versioned Repositories"; P20 "Heterogeneity Dividend and Homogeneity Saturation"; P21 "Million-level Coordination End-to-end Evidence and Extrapolation Boundary"; P22 "The Memory Wall Is Not the Coordination Wall"; P24 "Measured Positioning of the RSI Lineage"; P32 "Rules Over Learning"; P34 "v7.6.0 Literature Evidence Ledger and Forecast Updates."

**External dialogue counterparts (existence verified on 2026-09-21 per EVIDENCE_LEDGER_v76.md; figures paper-reported/company-disclosed, not independently reviewed)**

3. H-CSC Team (2026). *Certifiable Semantic Agreement Among LLM Agents: What the Admissibility Instrument Decides* (v2). arXiv:2606.07316.
4. ClawArena-Team (2026). *Benchmarking Subagent Orchestration and Dynamic Workflows in Language-Model Agents*. arXiv:2606.31174.
5. Zhu, S., et al. (2026). *RSIAgent: Autonomous Exploration for Recursive Self-improvement in New Environments*. arXiv:2609.15364.
6. Ouroboros (2026). *A Self-Developing Frontier Coding Agent with Reviewed Core Evolution*. arXiv:2608.08311.
7. Yang, K., et al. (2024). *OASIS: Open Agent Social Interaction Simulations with One Million Agents*. arXiv:2411.11581, NeurIPS 2024.
8. MAPF Team (2025). *Advancing Learnable Multi-Agent Pathfinding Solvers with Active Fine-Tuning*. arXiv:2506.23793, IROS 2025. ("Million success rate" downgraded per ledger A8 to media claim.)
9. Theseus Labs (2026). *Toward Genuine Recursive Self-Improvement*. arXiv:2609.11873.

---

## Evidence discipline and reproduction notes

- This paper is an **architecture-specification document**, with no new experiments; all eight changes are **design proposals · to implement**, all return positions `[RESULT NEEDED]`, forbidden to be written as measured.
- The existence, titles, and arXiv numbers of external papers were verified on 2026-09-21 per `EVIDENCE_LEDGER_v76.md`; their performance figures (H-CSC aggregate distance/AUROC, Ouroboros Terminal-Bench, RSIAgent open-source surpassing closed-source, MAPF million success rate, etc.) uniformly keep the scope "paper-reported/company-disclosed, not independently reviewed," and MAPF million figures are downgraded per A8 to media claims.
- UDOS existing figures only point back to v7.5.0 papers and `reports7/*.json`, with evidence grades per paper (verified / cpu-proto / cpu-proxy / unverified); single-seed pilots need ≥30 seeds and 95% CI before submission.
- This paper's falsifiable conditions C1–C5 correspond one to one with P34's F6–F10, all testable afterward with controlled comparisons within the 2027 scoring window.


---

<p align="center"><img src="assets/logo.png" width="180" alt="TwinsEarth"/></p>

# The v7.6.0 Evidence Ledger and Prediction Update: Three-State Audit, Downgraded Claims, and F6–F10

**Working Title (EN):** *The v7.6.0 Evidence Ledger and Prediction Update: Three-State Literature Audit, Downgraded Claims, and Falsifiable Forecasts F6–F10*

> Whole-volume closing paper P34 · upgrading the pre-verification shard's `EVIDENCE_LEDGER_v76.md` into the formal paper-version evidence ledger · updating the hard predictions of P15/P33 · UDOS Reasoning Engine v7.6.0 · Fang Wenxin · 2026-09-21
>
> **One sentence first**: not one external reference cited in the whole v7.6.0 volume is "heard about"—each was verified online, and pinned to one of three states: verified verified, partially verified partially verified (giving actual values), no traceable evidence no traceable evidence (marking [CITATION NEEDED]). This paper makes this ledger public, and makes public **which widely circulating numbers must be downgraded or removed** (MAPF million success rate, five attributes of settlement neutrality, Hybrid 96.1%, etc.), finally retaining P15's F1–F5 and adding five hard predictions F6–F10 scorable in 2027.
>
> **Evidence caliber (stated only once in the whole paper)**: this paper is an **evidence ledger and prediction registration** paper, containing no new experiments, no new UDOS measured numbers. The three-state verification was completed **2026-09-21** (Asia/Shanghai), the method being independently reviewing each clue with arXiv abs/html, ACL Anthology, IEEE Xplore, OpenReview, Zenodo, official press releases; the arXiv numbers, figures, institutional attributions the user pasted are all treated as "clues" rather than facts. A verified mark only represents "the reference truly exists, title/year/source consistent," its in-paper numbers still marked "paper report · not independently reviewed"; a partially verified mark is subject to the **actual values** this paper gives; a no traceable evidence mark is always written `[CITATION NEEDED]`, forbidden to write as fact.

---

## Abstract

The v7.6.0 paper volume (P16–P34) cites about 50 external clues. This paper does not do survey stacking, but runs all clues through **three-state verification**: verified verified (exists and bibliographic consistent), partially verified partially verified (exists but some number/record number inconsistent with the clue, giving actual values), no traceable evidence no traceable evidence (marking `[CITATION NEEDED]`). The verification conclusion is optimistic but restrained: the core new papers (H-CSC, EPOB, ClawArena-Team, Warp-Cortex, RSIAgent, Ouroboros, OASIS) **the vast majority truly exist and core propositions hold**; what really needs downgrading is four classes—(a) MAPF's "million-scale 99.9%/163μs" only seen in Russian media relaying; (b) "five attributes of settlement neutrality" not located to any original source; (c) Hybrid LangGraph-CrewAI's 96.1%/−76.2% token not verified to original text; (d) ISONOMIA/RSIAI0/Adaptive Bitmask's record numbers and simulation numbers need correction. This paper also registers a "removed/downgraded list," and on the basis of retaining P15 F1–F5 adds five falsifiable predictions F6–F10 (three-level finality, scope, BRS+DRS, review gate, rule predicate vs learned predicate), all marked "pending 2027 scoring."

**Keywords**: evidence ledger; three-state verification; citation audit; downgrade list; falsifiable prediction; literature credibility

---

## Structured Abstract (background problem → method/argument → evidence → contribution)

- **Background problem**: the easiest place an AI paper volume fails is not argument but **citation**—a widely circulating arXiv number or "million-scale" figure, if written into text without verification, builds the whole argument on sand.
- **Core argument**: an honest paper volume must pin each external material it cites to one of three states, and make public "which I believed, which I discounted, which I did not find."
- **Evidence clues**: about 50 clues verified online one by one; core new papers verified in majority, partially verified concentrated on record numbers and media-relayed figures, no traceable evidence concentrated on sourceless "framework five attributes" classes.
- **Contribution**: a formal paper-version three-state verification ledger; a removed/downgraded list; P15 F1–F5 retained + F6–F10 added prediction update; and the whole v7.6.0 volume's 19-paper index.

---

## 1 Why a Public Ledger Is Needed

AI writing's default failure mode is: the model taking "impressions" seen in training as "facts," taking second-hand relayed numbers as paper original text, taking an unfindable framework as "existing consensus." Since v7.6.0 calls itself "measurement and governance," it must execute the same governance on its own **citations**. Therefore this paper upgrades the pre-verification shard's working ledger into a formal paper, making public the three-state results and downgrade list—this is both responsible to readers and the extension at the literature layer of P12's evidence grading methodology.

The three-state definitions stated only once in the whole paper:

- **Verified verified**: the reference truly exists, title/author/year/source consistent; in-paper numbers still marked "paper report · not independently reviewed."
- **Partially verified partially verified**: the reference exists, but some number/record number/title inconsistent with the clue, this paper writes the **actual value**.
- **No traceable evidence no traceable evidence**: cannot find or seriously inconsistent with the given number, marking `[CITATION NEEDED]` when writing.

![Figure 2 Three-state distribution of the whole volume's external references](figures/P34_fig1_ledger_states.png)

*Figure 2 Distribution of v7.6.0 whole-volume external clues classified by three states (conceptual illustration; counts subject to this paper's ledger). Conceptual illustration, not measured data.*

---

## 2 Core New Paper Ledger (Group A, v7.6 main dialogue objects)

**Table 1 Group A: the eight core papers directly in dialogue with v7.6.0**

| No. | Reference | Status | Actual information verified | Safe citation caliber |
|---|---|---|---|---|
| A1 | H-CSC (semantic commit BFT) | verified (v2 retitled) | v2 retitled *Certifiable Semantic Agreement Among LLM Agents: What the Admissibility Instrument Decides* (arXiv:2606.07316v2); typed commit objects and 2f+1 certificate envelopes are true. Negative results verbatim hit: aggregation distance 5.32°/5.48° vs steelman 4.27°/4.32°; honestly retaining the author's explicit "we withdraw it"; lexical predicate AUROC 0.865–0.982 beats learned encoders 0.621–0.744. | Use v2 caliber: withdrawal + lexical predicate beats learned; numbers paper report · not independently reviewed. |
| A2 | EPOB (project orchestration benchmark) | verified | formally recorded as OpenReview `NtPIgzPtOU` (2026-05) + epob.us leaderboard; five dimensions Plan/Assignment/Coordination/Deliverable/Efficiency are true. | Five dimension names true; failure taxonomy paper report caliber. |
| A3 | ClawArena-Team | verified | *Benchmarking Subagent Orchestration...* (arXiv:2606.31174); 41 scenarios/258 rounds/pure execution-style scoring without LLM judging is true. | Direction true; "no LLM exceeds 50% permission precision," "hundredfold cost/fourfold total score," "SMS three-term product" not verbatim hit, write "paper report · not independently reviewed," not writing fixed percentages. |
| A4 | Warp-Cortex | verified | Singleton weight sharing + topological synapse, memory complexity O(N·L)→O(1)+O(N·k) is true. | "100 Agents/2.2GB" paper report; "million" paper self-described theoretical extrapolation, **actual measured only about hundred scale**. |
| A5 | RSIAgent | verified | curricula/actor/verifier three roles, broad-then-deep strategy true (arXiv:2609.15364). | "open-source Kimi-K3/GLM-5.3 beats GPT-6-class closed" paper/media report; title "New Environments" means learning in new environments, not cross-environment transfer. |
| A6 | Ouroboros | verified | reviewed commits becoming runtime is true (arXiv:2608.08311, v3). | Terminal-Bench 2.1 original 86.97%/after removing reward-hack 86.74% self-reported SOTA, not independently reviewed. |
| A7 | OASIS | verified | million-user social simulation, reproducing information diffusion/polarization/herding is true (arXiv:2411.11581, NeurIPS 2024). | Evidence nature is **phenomenon reproduction, not causal identification**. |
| A8 | MAPF-GPT-DDG | partially verified | true title *Advancing Learnable MAPF Solvers with Active Fine-Tuning* (arXiv:2506.23793, IROS 2025), DDG active fine-tuning is true. | "2048×2048 / 524k 100% / 1M 99.9% / 163μs" only seen in Russian media relaying, **not accepted as paper fact**; predecessor only small maps, 16–32 Agents. |

---

## 3 RSI Lineage Ledger (Group B) and Market Governance Ledger (Group C)

**Table 2 Group B: RSI systems used for P24 level positioning**

| No. | Reference | Status | Actual information |
|---|---|---|---|
| B1 | The Last AI Built by Humans | verified | arXiv:2609.11873; HCI + seven RSI discrimination criteria, as P24 yardstick. |
| B2 | Dream-RSI | verified | accurate title *Recursive Self-Improvement through Evolving Worlds* (arXiv:2609.14858); "317 calls ≈ 51,200 generations" paper/official report. |
| B3 | Gödel Agent | verified | ACL 2025, 2025.acl-long.1354 (arXiv:2410.04444); self-referential code-modification self-improvement, second-hand material marks L4, not calling L5. |
| B4 | HyperAgents | verified | arXiv:2603.19461 (Meta FAIR); the meta-level modification process itself editable, closest to "recursive meta-improvement" but not reaching L5. |
| B5 | RSIAI0 | partially verified | correctly recorded as Zenodo **15644670** (not user-given 15646184); VM controlled execution + Darwinian Mode. |

**Table 3 Group C: Markets and governance**

| No. | Reference | Status | Actual information |
|---|---|---|---|
| C1 | AgentLance | verified | arXiv:2608.23867; private-cost bidding + self-maintained strategy notes + VCG-style payment. |
| C2 | Agent Exchange | verified | arXiv:2507.03904; multi-attribute auctions + generalized second price, greedy/random degradation when liquidity insufficient. |
| C3 | ISONOMIA | partially verified | correctly recorded Zenodo **21338480** (not 21287289); mutual-trust bookkeeping unit ergs + bicameral governance true; "45,000 simulations/300 parameter points" not verified, mark `[RESULT NEEDED]`. |
| C4 | When Agent Markets Arrive | verified | arXiv:2604.06688; first-price sealed-bid auction. |
| C5 | Five attributes of settlement neutrality | no traceable evidence | not located to any original paper/Zenodo record; **forbidden** to cite as existing framework, rewrite as UDOS P11/P25-authored design goal or mark `[CITATION NEEDED]`. |

---

## 4 Communication Scale, Evaluation Frameworks, and Classic/Industry Ledgers (Groups D/E/F/G summary)

**Table 4 Group D Communication and scale**

| No. | Reference | Status | Actual information |
|---|---|---|---|
| D1 | RocketMQ-A2A | verified | ACM FSE 2026 Industry (Alibaba Cloud); session-level replayable event streams, LiteTopic session isolation, resume from breakpoint. |
| D2 | Adaptive Bitmask | partially verified | npm product real, sub-10ms direction credible; "85× compression/8.2ms p99/N=5000" not independently reviewed, mark author claim. |
| D3 | AI Agent Protocols survey | verified | arXiv:2504.16736; CNS=(N_success/N_attempts)/T_avg verbatim hit. |

**Table 5 Group E Evaluation and frameworks**

| No. | Reference | Status | Actual information |
|---|---|---|---|
| E1 | TUMIX | verified | arXiv:2510.01279; highest +3.55% on Gemini-2.5-Pro verbatim hit. |
| E2 | MAKER | verified | arXiv:2511.09030; extreme decomposition + k=3 majority voting, first zero-error completion of over-million-step tasks. |
| E3 | Thinking vs Doing | verified | arXiv:2506.07976, NeurIPS 2025 WS; Gemma 3 12B reaches open-source SOTA on WebVoyager/WebArena. |
| E4 | Hybrid LangGraph-CrewAI | partially verified | hybrid direction truly exists, but IEEE Access 11481053, "96.1%/−76.2% token/14.5× latency" not located to original text, mark pending. |
| E5 | MultiAgentBench | verified | ACL 2025, 2025.acl-long.421; systematically comparing star/chain/tree/graph topologies; precise "graph best" ranking not verbatim hit, write "topology strongly task-related." |
| E6 | AgentSociety | verified | arXiv:2502.08691; 10,000+ Agents, about 5 million interactions verbatim hit. |
| E7 | MegaAgent | verified | ACL 2025 Findings, 2025.findings-acl.259; no predefined SOP, national-level policy simulation expanding to 590 Agents. |

**Group F Classic background** (F1–F10, all verified numbers correct): Kaplan 2020 (2001.08361), Chinchilla 2022 (2203.15556), Attention 2017 (1706.03762), AutoGen 2023 (2308.08155), Practical BFT 1999, HotStuff 2019 (1803.05069), Switch Transformers 2021 (2101.03961), Krogh & Vedelsby 1995, Emergence Mirage 2023 (2304.15004), Conformal Prediction 2021 (2107.07511).

**Group G Industry rumors** (key items verified one by one, all verified, individual numbers partially verified): Kimi K2.6 Agent Swarm (officially says up to 300 sub-Agents parallel), Kimi K3 (arXiv:2607.24653, 2.8T parameter open-source MoE), Ant Agentar 2.0 (WAIC 2026, nearly 200 job templates), Inspur single rack 384 OCM CPUs supporting 40,000+ Agents, Huawei openJiuwen (dual-dimension RSI, arXiv:2608.27969), Huawei Tao(τ) Law (ISCAS 2026-05-25, six years mass production 381 chips, 2031 equivalent 1.4nm), Anthropic R&D automation (about 26% R&D AI-led, about 30,000 concurrent Agents; "1 billion decisions" partially verified not found, discarded), OpenAI automated research intern → 2028-03 researcher (about 3.1 Agent workdays/person-day), Pillar 3 Fail-Safe & Recovery (Swarm legal abort/drift rollback/cascade containment).

---

## 5 The Removed / Downgraded List (writers must comply)

The following nine are circulating claims this verification **actively abandoned or downgraded**. They are not "outrageously wrong," but "writing into text misleads":

1. **C5 Five attributes of settlement neutrality (no traceable evidence)**: no original source. Can only rewrite as UDOS P11/P25-authored design goal, or mark `[CITATION NEEDED]`.
2. **A8 MAPF million numbers (partially verified downgrade)**: DDG active fine-tuning true; "524k 100% / 1M 99.9% / 163μs" only Russian media relaying, P21 can only say direction, million numbers mark media claim.
3. **B5 RSIAI0 record number (partially verified)**: correct is Zenodo **15644670**, not 15646184.
4. **C3 ISONOMIA record number and simulation numbers (partially verified)**: correct is Zenodo **21338480**; "45,000 simulations/300-300 pass" not verified, mark `[RESULT NEEDED]`.
5. **D2 Adaptive Bitmask (partially verified)**: sub-10ms direction credible, "85× compression/8.2ms p99" mark author claim.
6. **E4 Hybrid LangGraph-CrewAI (partially verified)**: "96.1%/−76.2%/14.5×" not located to original text, downgraded to pending.
7. **A3 ClawArena precise thresholds (partially verified)**: direction true, "50% permission precision/hundredfold cost/SMS formula" not verbatim hit, mark paper report · not independently reviewed.
8. **G6 Anthropic "1 billion decisions" (partially verified)**: 26%/30,000 concurrent verified; "1 billion" not found, discarded.
9. **A1 H-CSC version (partially verified)**: must use v2 caliber—has withdrawn "honest retention advantage," lexical predicate beats learned encoders; must not continue v1 positive conclusions.

---

## 6 Prediction Update: Retain F1–F5, Add F6–F10

P15's proposed F1–F5 (structural change share, changing structure changes curve, architecture search boundary, calibration-class gain, cross-substrate master curve collapse) are **all retained**, continuing as 2026–2031 scoring objects. This section adds F6–F10, specifically scoring the success/failure of v7.6.0's eight design proposals—they are all written as **falsifiable conditions**, not established conclusions.

![Figure 3 Scoring timeline of the added predictions F6–F10](figures/P34_fig2_forecast_timeline.png)

*Figure 3 The proposal time (2026-09) and expected scoring window (2027) of the five design predictions F6–F10. Conceptual illustration, not measured data.*

- **F6 (three-level finality vs binary stopping)**: under the same fault-injection suite, three-level typed finality relative to v7.5.0 binary STOP/CONTINUE should have significantly higher completion rate, false-stop rate should not rise; if no measurable difference, then ① roll back binary. **Criterion**: ≥30 seeds, 95% CI not crossing zero. Pending 2027 scoring.
- **F7 (scope tightening boundary-crossing rejection)**: after TransferBundle adds scope, structural boundary-crossing call rejection rate should significantly rise, while normal work-order availability loss below preset threshold; if availability loss exceeds boundary-crossing benefit, then ② change to on-demand authorization. Pending scoring.
- **F8 (BRS+DRS coverage of blind zones)**: the two-stage flywheel relative to single-stage coverage-aware should measurably raise coverage of the identifiability blind zones P6 identified (acceleration scalar class); if no improvement then ③ fall back single stage. Pending scoring.
- **F9 (review gate intercepting silent failures)**: the [structural] gate's interception rate of "version-number accident class" silent failures should be significantly higher than the no-gate baseline, while the cost to merge speed acceptable; if idle then ④ downgrade to a hint. Pending scoring.
- **F10 (rule predicate vs learned predicate AUROC difference)**: under the same adversarial attack family, deterministic rule predicates' safety discrimination AUROC should be no lower than learned predicates (continuing H-CSC v2 direction); if learned predicates stably and substantially dominate, then ⑧ "rules as base" needs weakening to attack-family-based splitting. Pending scoring.

---

## 7 The v7.6.0 Whole Volume 19-Paper Index

**Table 6 P16–P34 quick reference**

| Paper | Title | One-sentence theme | External dialogue object |
|---|---|---|---|
| P16 | Typed finality | binary stopping → three-level semantic commit | H-CSC v2 |
| P17 | Least-privilege orchestration | TransferBundle adds scope | ClawArena-Team |
| P18 | Broad-deep two-stage flywheel | BRS+DRS submodular collection | RSIAgent |
| P19 | Structural change review gate | MatrixConfig submission must accompany assertions | Ouroboros |
| P20 | Heterogeneity dividend and homogeneity saturation | independent source count ≠ head count | OASIS/TUMIX/P9 |
| P21 | Million-level coordination end-to-end evidence | MAPF success cannot extrapolate semantic tasks | MAPF-GPT-DDG |
| P22 | Memory wall is not coordination wall | capacity ≠ quality ≠ coordination | Warp-Cortex |
| P23 | Project-level five-dimensional evaluation | work order → project failure taxonomy | EPOB |
| P24 | RSI lineage measured positioning | six systems land L1–L5, none L5 | Theseus/Dream-RSI/Gödel/HyperAgents |
| P25 | Agent task markets | VCG private information and settlement | AgentLance/Exchange/ISONOMIA |
| P26 | Weighted BFT and reputation | weight direction of equal-weight 2f+1 | WBFT/COALITION-VAST/BlockAgents |
| P27 | Large-scale social simulation causal validity | phenomenon reproduction ≠ mechanism verification | OASIS/AgentSociety/MegaAgent |
| P28 | Million-level communication protocol | replayable event streams/sub-10ms | RocketMQ-A2A/Bitmask |
| P29 | Test-time scaled collaboration | tool integration and extreme decomposition voting | TUMIX/MAKER/TTI |
| P30 | Hybrid orchestration and framework-task matching | topology selection decision tree calibration | MultiAgentBench/AutoGen |
| P31 | Fail-safe and recovery | legal abort/drift rollback/cascade containment | Pillar 3 |
| P32 | Rules before learning | adversarial scenario predicate robustness boundary | H-CSC v2 lesson generalized |
| P33 | v7.6.0 master architecture | eight structural changes unified spec | whole volume convergence |
| P34 | Evidence ledger and prediction update | three-state verification + F6–F10 | this paper |

---

## 8 Drawbacks, Failure Boundaries, and Falsifiable Conditions

### 8.1 This Ledger's Own Boundaries

- **Three-state verification is "existence verification," not "correctness verification."** A verified mark only represents "this paper truly exists, bibliographic matches," not that its conclusions are right; in-paper numbers always "paper report · not independently reviewed."
- **Verification completed at abstract level.** A3's SMS formula, E4's 96.1% etc. "not verbatim hit" only means this verification did not read at abstract level, not that the paper necessarily lacks them—they should be opened paper by paper to full text for review before submission, rather than killed with one blow.
- **Industry rumors have short timeliness.** Group G (Kimi/Ant/Inspur/Huawei/OpenAI/Anthropic) comes from official pages and mainstream media, may still change after 2026-09-21, cite with dates.

### 8.2 Falsifiable Conditions

- **C-ledger-1**: if readers open any verified reference's original text and find title/number/core mechanism inconsistent with this paper's ledger, then that item should immediately be re-marked partially verified/no traceable evidence and the safe citation sentence corrected.
- **C-ledger-2**: if F6–F10 are falsified in the 2027 scoring window, then the corresponding design proposals (P16–P19, P32) should roll back per P33's C1–C5, rather than persisting by changing wording.

---

## 9 Conclusion

A paper volume's credibility ultimately depends not on how many papers it cites, but on whether it **dares to publicly admit what was not verified, what was discounted**. v7.6.0's honest position is: the core new papers the vast majority true, core propositions hold; but all widely circulating yet unsound numbers (MAPF million success rate, five attributes of settlement neutrality, Hybrid 96.1%, Anthropic 1 billion decisions), this paper all downgrades or removes, and writes the reasons. F6–F10 turn the eight design proposals from "self-feeling good" into "falsifiable propositions waiting to be slapped."

**The ledger is not decoration, it is this volume's disclaimer and scoreboard—what is written in must be answered for, what cannot be verified is never taken as fact.**

---

## References

**UDOS paper volumes (evidence grades marked in text)**

1. UDOS v7.5.0: P9 "Quality saturation law," P12 "Reproducible systems engineering driven by evidence grading," P14 "Convergence of RSI," P15 "Life originates from structural organization" (F1–F5).
2. UDOS v7.6.0: P16–P33 all dedicated papers (papers see Table 6).

**External references (existence verified per EVIDENCE_LEDGER_v76.md 2026-09-21; numbers paper reports/company disclosures, not independently reviewed)**

3. H-CSC v2 (arXiv:2606.07316); EPOB (OpenReview NtPIgzPtOU, 2026); ClawArena-Team (arXiv:2606.31174); Warp-Cortex (arXiv:2601.01298); RSIAgent (arXiv:2609.15364); Ouroboros (arXiv:2608.08311); OASIS (arXiv:2411.11581, NeurIPS 2024); MAPF-GPT-DDG (arXiv:2506.23793, IROS 2025).
4. Theseus Labs (arXiv:2609.11873); Dream-RSI (arXiv:2609.14858); Gödel Agent (ACL 2025, long.1354); HyperAgents (arXiv:2603.19461); RSIAI0 (Zenodo 15644670).
5. AgentLance (arXiv:2608.23867); Agent Exchange (arXiv:2507.03904); ISONOMIA (Zenodo 21338480); When Agent Markets Arrive (arXiv:2604.06688).
6. RocketMQ-A2A (ACM FSE 2026 Industry); A Survey of AI Agent Protocols (arXiv:2504.16736).
7. TUMIX (arXiv:2510.01279); MAKER (arXiv:2511.09030); Thinking vs Doing (arXiv:2506.07976); MultiAgentBench (ACL 2025, long.421); AgentSociety (arXiv:2502.08691); MegaAgent (ACL 2025 Findings, 259).
8. Classic background: Kaplan 2020 (2001.08361); Chinchilla 2022 (2203.15556); Attention 2017 (1706.03762); AutoGen 2023 (2308.08155); Castro & Liskov 1999; HotStuff (1803.05069); Switch Transformers (2101.03961); Krogh & Vedelsby 1995; Emergence Mirage (2304.15004); Conformal Prediction (2107.07511).
9. Industry disclosures (Group G): Kimi K2.6/K3 (arXiv:2607.24653); Ant Agentar 2.0 (WAIC 2026); Inspur whole rack (2026-07); Huawei openJiuwen (arXiv:2608.27969) and Tao(τ) Law (ISCAS 2026-05-25); Anthropic R&D automation data (2026-09); OpenAI automated research intern (2026-09-06); Cyber Strategy Institute AI SAFE² Pillar 3 (2026-05).

---

## Evidence Discipline and Reproduction Notes

- This paper is an **evidence ledger and prediction registration** paper, no new experiments; three-state verification completed 2026-09-21, the method being independently reviewing each clue online.
- Verified only represents reference existence and bibliographic consistency, in-paper numbers still "not independently reviewed"; partially verified always subject to this paper's actual values; no traceable evidence always `[CITATION NEEDED]`.
- The removed/downgraded nine (Section 5) are this volume's hard discipline, when dedicated papers cite related numbers they must not fall back to the user's original clue values.
- F6–F10 are falsifiable scoring conditions for design proposals, marked "pending 2027 scoring"; before submission partially verified items should be opened paper by paper to full text for secondary review.


---
