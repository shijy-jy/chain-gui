//! engram-file · 文件层（三层重构 P4，设计稿 v1 §2/§4.1）。
//!
//! 职责边界（严格）：
//! - 事实源（`.chain/nodes/*.md`）与原始输入（`.chain/dialogue/`）的**字节级**读写与扫描；
//! - 协议词表与 AI 指南文本（profile / guide —— 协议随事实源走）；
//! - 审计 append-only 落盘原语、schema 版本与幂等迁移、watcher、工作区标签与列表。
//!
//! **不变量（编译期强制）**：本 crate 不依赖 engram-core / engram-mcp / engram-gui /
//! engram-cli——文件层不认识"记忆"，也不认识任何入口程序。

pub mod audit;
pub mod chain_ops;
pub mod dialogue;
pub mod evidence;
pub mod fsio;
pub mod guide;
pub mod migrate;
pub mod model;
pub mod node_edit;
pub mod profile;
pub mod scan_pool;
pub mod scanner;
pub mod schema;
pub mod watch;
pub mod workspace;
