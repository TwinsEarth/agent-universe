//! 任务调度（v3.5.9 状态登记，DOC-05 / 自审 F-4）：**纯算法库（library-only）**。
//! `TaskRouter` / `LoadBalancer` 当前不在 `gsn-daemon` 的启动运行图中构造，仅由
//! `lib.rs` re-export 供库使用者与测试引用；官方插件需要的「按技能/负载分派」逻辑
//! 已在 `plugin/official` 内联实现（见该模块对 `assign_task` 的移植注释），不经过这里。
//! 状态：**未在守护进程生效，属待决项（接线启用 or 收敛删除）**，见
//! `docs/CAPABILITY-STATUS.md`。不要把本模块描述为节点实际运行的调度能力。

pub mod load_balancer;
pub mod router;

pub use load_balancer::{BalanceStrategy, LoadBalancer};
pub use router::TaskRouter;
