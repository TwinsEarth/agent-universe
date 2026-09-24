//! 群体记忆对照实验：可复现的最小验证（证据等级 cpu-proto）。
//!
//! 运行：`cargo test --test swarm_memory_experiment -- --nocapture`

use gsn_core::swarm::memory::{run_experiment, Rng, SharedMemory};

/* ------------------------------ 基础组件行为 ------------------------------ */

#[test]
fn rng_is_deterministic_and_in_range() {
    let mut a = Rng::new(12345);
    let mut b = Rng::new(12345);
    for _ in 0..100 {
        assert_eq!(a.next_u64(), b.next_u64());
    }
    let mut r = Rng::new(7);
    for _ in 0..1000 {
        let x = r.below(10);
        assert!(x < 10);
    }
}

#[test]
fn shared_memory_lookup_record_coverage() {
    let mut m = SharedMemory::new();
    // 未写入：miss。
    assert_eq!(m.lookup(1), None);
    assert_eq!(m.retrieval_misses, 1);
    assert_eq!(m.retrieval_hits, 0);

    m.record(1, 7);
    m.record(1, 7); // 幂等
    assert_eq!(m.solved_scenes(), 1);
    assert_eq!(m.lookup(1), Some(7));
    assert_eq!(m.retrieval_hits, 1);

    m.record(2, 3);
    assert!((m.coverage(10) - 0.2).abs() < 1e-9);
}

/* ------------------------------ 对照实验主测试 ----------------------------- */

#[test]
fn shared_memory_improves_hit_rate_and_reduces_attempts() {
    // 口径：200 场景，每个场景 10 个候选解，每任务最多 3 次尝试，20 个智能体依次处理。
    let r = run_experiment(200, 10, 3, 20, 20260924);

    // 打印实测汇总（--nocapture 可见），论文数字以此实测为准。
    println!("\n{}", r.summary());

    /* baseline 应贴近理论值：命中率≈B/K=0.30，首次≈1/K=0.10，尝试≈2.7。
       这一步同时验证了随机/盲猜实现本身正确。 */
    assert!((r.baseline.hit_rate() - 0.30).abs() < 0.03, "baseline hit {}", r.baseline.hit_rate());
    assert!((r.baseline.first_success_rate() - 0.10).abs() < 0.02);
    assert!((r.baseline.avg_attempts() - 2.7).abs() < 0.05);

    /* 核心主张：共享记忆组在三个指标上全面优于无记忆组。 */
    assert!(r.shared.hit_rate() > r.baseline.hit_rate() + 0.4, "shared hit {}", r.shared.hit_rate());
    assert!(r.shared.first_success_rate() > r.baseline.first_success_rate() + 0.5);
    assert!(r.shared.avg_attempts() < r.baseline.avg_attempts() - 1.0);

    /* 机制：记忆被大量检索命中，最终几乎覆盖全部场景。 */
    assert!(r.retrieval_hits > 0);
    assert!(r.final_coverage > 0.98, "coverage {}", r.final_coverage);

    /* 覆盖率随智能体单调不减（开垦只增不删）。 */
    for w in r.per_agent.windows(2) {
        assert!(w[1].coverage_in >= w[0].coverage_in);
    }

    /* 冷启动 -> 高命中：最后一个智能体命中率远高于开拓者。 */
    let first = &r.per_agent.first().unwrap();
    let last = &r.per_agent.last().unwrap();
    assert!(last.hit_rate > first.hit_rate + 0.5, "first {} last {}", first.hit_rate, last.hit_rate);
    assert!(last.avg_attempts < first.avg_attempts - 1.0);
}

#[test]
fn same_seed_is_reproducible() {
    let a = run_experiment(120, 8, 2, 10, 42);
    let b = run_experiment(120, 8, 2, 10, 42);
    assert_eq!(a.shared.hit_rate(), b.shared.hit_rate());
    assert_eq!(a.baseline.avg_attempts(), b.baseline.avg_attempts());
    assert_eq!(a.final_coverage, b.final_coverage);
}
