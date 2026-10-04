//! 文件层轻量并行映射池（三层重构性能项：1500 节点冷启动扫描）。
//!
//! 只做一件事：把一个**纯计算**的映射函数跑在多线程上，并保证**输出严格保序**。
//! 之所以能安全用于扫描：worker 只负责「读文件 + 解析 + 构造节点」，绝不触碰共享状态；
//! 所有入表、去重、报错落位都由调用方在主线程按文件序完成——同一份输入下，
//! 并行与顺序两条驱动路径必须产出**逐字节一致**的快照（golden 契约测试依赖这一点）。
//!
//! 三条不变量：
//! 1. **保序**：`out[i] == f(items[i])`，与核数、调度、单条耗时无关（按索引回填，
//!    绝不让"谁先算完"决定次序）；
//! 2. **无共享写**：worker 结果先落在自己的局部 `Vec<(idx, R)>`，合并只发生在主线程；
//!    worker 内也不允许有 panic 逃逸——scoped 线程 panic 会在 `scope` 退出时重抛，
//!    因此 worker 体内自行 `catch_unwind`；
//! 3. **失败即回落**：条目太少（< [`MIN_PARALLEL_ITEMS`]）、单核、线程创建失败、
//!    任一 worker panic —— 一律返回 `None`，由调用方走 [`map_sequential`]；
//!    本模块**绝不把 worker 的 panic 传播给调用方**。
//!
//! 仅用标准库（`std::thread::scope` + `available_parallelism`），不引入任何依赖。

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::atomic::{AtomicUsize, Ordering};

/// 并行下限：少于该条目数时开线程的调度成本高于收益，直接走顺序路径。
pub const MIN_PARALLEL_ITEMS: usize = 8;

/// worker 线程名前缀（日志辨识用；测试据此判定"当前是否运行在 worker 线程内"）。
pub const WORKER_THREAD_PREFIX: &str = "engram-scan";

/// 该条目数走并行是否有意义：条目够多 + 真的是多核。
/// `None` = 不值得/不可用（调用方走顺序路径，语义完全一致）。
fn parallel_worker_count(len: usize) -> Option<usize> {
    if len < MIN_PARALLEL_ITEMS {
        return None;
    }
    let cores = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    if cores < 2 {
        return None;
    }
    Some(cores.min(len))
}

/// 顺序映射：并行路径的语义基准，也是所有回落场景的最终执行者。
pub fn map_sequential<T, R>(items: &[T], f: impl Fn(&T) -> R) -> Vec<R> {
    items.iter().map(f).collect()
}

/// 并行映射（保序）。`None` = 并行不可用或失败，**请调用方自己走 [`map_sequential`]**；
/// 之所以把回落决定权交给调用方：只有它知道"重算一遍"是否安全（纯读扫描安全，
/// 带副作用的映射则应放弃而不是重跑）。
///
/// 调度：`AtomicUsize` 抢索引（动态负载均衡，单文件耗时差异大时不会出现静态分块的
/// 长尾）；每条结果先入 worker 私有 `Vec`，主线程 `join` 后按索引回填。
pub fn map_parallel<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Option<Vec<R>> {
    let threads = parallel_worker_count(items.len())?;
    let next = AtomicUsize::new(0);
    let (next, f) = (&next, &f);

    std::thread::scope(|scope| {
        let mut handles = Vec::with_capacity(threads);
        for tid in 0..threads {
            let worker = move || {
                let mut local: Vec<(usize, R)> = Vec::new();
                let mut panicked = false;
                loop {
                    let i = next.fetch_add(1, Ordering::Relaxed);
                    if i >= items.len() {
                        break;
                    }
                    // 捕获 worker 内 panic：既让本线程体面退出（否则 scope 退出时会重抛），
                    // 也让"这单没算全"这个事实以 panicked 标志回到主线程
                    match catch_unwind(AssertUnwindSafe(|| f(&items[i]))) {
                        Ok(r) => local.push((i, r)),
                        Err(_) => {
                            panicked = true;
                            break;
                        }
                    }
                }
                (local, panicked)
            };
            match std::thread::Builder::new()
                .name(format!("{WORKER_THREAD_PREFIX}-{tid}"))
                .spawn_scoped(scope, worker)
            {
                Ok(handle) => handles.push(handle),
                // 线程创建失败（资源耗尽）：已启动的 worker 会把队列排空自行结束
                // （worker 内已 catch panic，不 join 也不会让 scope 重抛），直接回报回落
                Err(_) => return None,
            }
        }

        let mut slots: Vec<Option<R>> = (0..items.len()).map(|_| None).collect();
        let mut complete = true;
        for handle in handles {
            // 兜底分支：worker 内已 catch_unwind，正常永远走 Ok
            match handle.join() {
                Ok((local, panicked)) => {
                    if panicked {
                        complete = false;
                    }
                    for (i, r) in local {
                        slots[i] = Some(r);
                    }
                }
                Err(_) => complete = false,
            }
        }
        // 有洞（panic / 未派发）→ 不拼接半成品，整单回落顺序路径
        if !complete || slots.iter().any(|s| s.is_none()) {
            return None;
        }
        Some(slots.into_iter().map(|s| s.expect("已确认填满")).collect())
    })
}

/// 两条驱动合一：多核且条目够多走并行，否则（或并行失败）走顺序——输出恒保序、恒完整。
/// 扫描器的两条路径（nodes/ 与 archive/）都只调这一个入口。
pub fn map_parallel_or_seq<T: Sync, R: Send>(items: &[T], f: impl Fn(&T) -> R + Sync) -> Vec<R> {
    match map_parallel(items, &f) {
        Some(results) => results,
        None => map_sequential(items, &f),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 当前是否运行在 worker 线程内（用于构造"只在并行路径 panic"的注入闭包）
    fn in_worker_thread() -> bool {
        std::thread::current()
            .name()
            .map(|n| n.starts_with(WORKER_THREAD_PREFIX))
            .unwrap_or(false)
    }

    fn multi_core() -> bool {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            >= 2
    }

    #[test]
    fn map_parallel_preserves_input_order_for_thousands_of_items() {
        // (a) 数千条：输出必须与输入同序、同长、逐值相等
        let items: Vec<u64> = (0..4096).collect();
        let out = map_parallel_or_seq(&items, |i| i.wrapping_mul(2_654_435_761) ^ 0x9e37);
        assert_eq!(out.len(), items.len(), "条目数不得变化");
        let expected: Vec<u64> = items
            .iter()
            .map(|i| i.wrapping_mul(2_654_435_761) ^ 0x9e37)
            .collect();
        assert_eq!(out, expected, "并行输出必须与输入严格同序");
    }

    #[test]
    fn map_parallel_equals_map_sequential_on_multi_core() {
        // 并行路径真的被走到时：结果与顺序路径全等（含位置）
        if !multi_core() {
            return; // 单核环境无从验证并行分支
        }
        let items: Vec<String> = (0..3000).map(|i| format!("node-{i:04}")).collect();
        let weigh = |s: &String| s.len() + s.bytes().filter(|b| b.is_ascii_digit()).count();
        let par = map_parallel(&items, weigh).expect("多核 + 3000 条应走并行路径");
        assert_eq!(par, map_sequential(&items, weigh));
    }

    #[test]
    fn small_input_uses_sequential_fallback_with_same_values() {
        // (b) 小输入（< 阈值）：并行直接不可用，两驱动合一的结果与顺序路径全等
        let items: Vec<i32> = (0..MIN_PARALLEL_ITEMS as i32 - 1).collect();
        assert!(
            map_parallel(&items, |i| *i + 1).is_none(),
            "少于 {MIN_PARALLEL_ITEMS} 条不应开线程"
        );
        let out = map_parallel_or_seq(&items, |i| *i + 1);
        assert_eq!(out, map_sequential(&items, |i| *i + 1));
        assert_eq!(out, (1..MIN_PARALLEL_ITEMS as i32).collect::<Vec<_>>());

        // 阈值边界（恰好 8 条）：走哪条路径都必须给出一致结果
        let edge: Vec<i32> = (0..MIN_PARALLEL_ITEMS as i32).collect();
        assert_eq!(
            map_parallel_or_seq(&edge, |i| *i * 3),
            map_sequential(&edge, |i| *i * 3)
        );

        // 空输入
        let empty: Vec<i32> = Vec::new();
        assert_eq!(map_parallel_or_seq(&empty, |i| *i), Vec::<i32>::new());
        assert!(map_parallel(&empty, |i| *i).is_none());
    }

    #[test]
    fn worker_panic_does_not_panic_caller() {
        // (c) worker 内 panic：绝不传染调用方，只回报"并行失败"信号
        let items: Vec<usize> = (0..64).collect();
        let out = map_parallel(&items, |i| {
            if *i == 7 {
                panic!("注入 panic：模拟单文件解析路径炸了");
            }
            *i
        });
        assert!(
            out.is_none(),
            "worker panic 必须回报 None（回落信号），不得 panic 调用方"
        );
    }

    #[test]
    fn worker_panic_falls_back_to_sequential_with_complete_results() {
        // (c) 续：调用方按 None 走顺序路径后，必须拿到**完整**结果（不缺项、不重算错位）
        let items: Vec<usize> = (0..256).collect();
        let out = map_parallel_or_seq(&items, |i| {
            if in_worker_thread() {
                panic!("注入 panic：模拟 worker 线程内代码炸了");
            }
            *i * 5
        });
        let expected: Vec<usize> = (0..256).map(|i| i * 5).collect();
        assert_eq!(out, expected, "worker panic 后应整体回落顺序路径且结果完整");
    }

    #[test]
    fn each_item_is_mapped_exactly_once() {
        // 工作队列不得重复派发或漏派发（顺序即证据；计数为补充）
        let items: Vec<usize> = (0..2000).collect();
        let calls = AtomicUsize::new(0);
        let out = map_parallel_or_seq(&items, |i| {
            calls.fetch_add(1, Ordering::Relaxed);
            *i
        });
        assert_eq!(out, items, "输出应与输入逐项相等");
        assert_eq!(
            calls.load(Ordering::Relaxed),
            items.len(),
            "每条应恰好算一次"
        );
    }

    #[test]
    #[ignore = "标定用：sleep 会拖慢普通测试（冷启动 I/O 延迟形状的对照实验）"]
    fn bench_latency_bound_items_are_overlapped() {
        // 冷启动扫描的真实瓶颈是"每文件约 9 ms 的等 I/O"（首次打开 + 安全扫描），
        // 属于并发等待而非算力。这里用 5 ms 人工延迟复刻该形状：
        // 顺序路径 = N × 延迟，并行路径 ≈ N / 核数 × 延迟。
        if !multi_core() {
            return;
        }
        let cores = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        let items: Vec<usize> = (0..64).collect();
        let work = |_: &usize| std::thread::sleep(std::time::Duration::from_millis(5));

        let t = std::time::Instant::now();
        map_sequential(&items, work);
        let seq = t.elapsed();

        let t = std::time::Instant::now();
        map_parallel_or_seq(&items, work);
        let par = t.elapsed();

        println!("ENGRAM_BENCH 延迟型负载 64×5ms（核数 {cores}）：顺序 {seq:?} / 并行 {par:?}");
        if cores >= 4 {
            assert!(
                par * 2 < seq,
                "延迟型负载应被并行重叠：顺序 {seq:?} vs 并行 {par:?}"
            );
        }
    }
}
