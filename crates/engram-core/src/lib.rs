//! Engram 领域核心（终版 §1.1 / 三层重构 P4）：
//! **记忆层 + API 层**——检索/强度/归档/蒸馏/审计语义、对话账本、工具函数。
//! 文件层已拆入 `engram-file`（事实源/协议词表/schema/迁移/原子写原语）并在此 re-export
//! 以保持 `engram_core::*` 旧路径兼容；依赖方向唯一：入口 crate → core → engram-file。

// 文件层 re-export（旧路径兼容：engram_core::model / scanner / schema / ... 不变）
pub use engram_file::{audit, dialogue, evidence, fsio, guide, migrate, model, profile, scanner, schema, watch, workspace};

pub mod code_map;
pub mod consolidate;
// 三层重构 P0：`dialogue` 已在文件层（只做字节与文件名，经 re-export 暴露），
// `dialogue_log` 属记忆层（JSONL 格式、覆盖区间、消费锚点的唯一定义处）。
pub mod dialogue_log;
pub mod embed;
pub mod index;
pub mod ops;
pub mod retrieval;
pub mod stats;
pub mod version;
