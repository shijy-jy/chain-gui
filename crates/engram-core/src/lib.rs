//! Engram 领域核心（终版 §1.1）：
//! 唯一知道「规则」的纯库——节点模型 / YAML 解析 / 读写守门（D2/D3/D4）/ 乐观锁 / 原子写 /
//! 检索工具 / watcher 回调 / 双模式 profile。
//! 依赖方向唯一：入口 crate → core；core 不得依赖任何入口 crate（宪法第 1 条）。

pub mod audit;
pub mod code_map;
pub mod consolidate;
pub mod embed;
pub mod evidence;
pub mod guide;
pub mod index;
pub mod migrate;
pub mod model;
pub mod ops;
pub mod profile;
pub mod retrieval;
pub mod scanner;
pub mod schema;
pub mod stats;
pub mod version;
pub mod watch;
pub mod workspace;
