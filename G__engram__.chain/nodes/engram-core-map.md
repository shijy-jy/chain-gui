---
id: engram-core-map
type: note
title: Engram · engram-core 模块地图
parent: engram-framework
status: none
tags:
- Engram
- engram-core
- Rust
- 模块
code_map: ../test1.x/crates/engram-core
revision: 1
updated: 2026-09-09T08:09:10+08:00
---

> 触发：engram-core 模块；core 代码结构；Engram Rust 模块

# Engram · engram-core 模块地图

`crates/engram-core/src/`，19 个公开模块（lib.rs）。唯一知道「规则」的纯库：节点模型 / YAML 解析 / 读写守门（D2/D3/D4）/ 乐观锁 / 原子写 / 检索工具 / watcher 回调 / 双模式 profile。

| 模块 | 职责 |
|---|---|
| `model/` | Node / ChainSnapshot / 更新模型 / 词表（node.rs、chain.rs、validation.rs） |
| `scanner/` | frontmatter 解析（BOM 剥离）、walker 目录扫描、validator 结构校验 |
| `ops/` | 读写守门 + 唯一写路径：node_edit（开发模式编辑原语）、chain（链级操作） |
| `profile` | 双模式配置包（词表 / 指南指针 / 校验开关）——模式是配置不是分支 |
| `guide` | AI 指南内嵌（分析 v8 / 开发 v3）与版本号 |
| `watch` | watcher 回调（nodes 非递归 + archive 递归双监听） |
| `workspace` | Workspace 上下文（根目录 / 模式检查 / 配置目录） |
| `embed` | 嵌入后端：fastembed + BGE-small-zh-v1.5，Embedder trait 可插拔 |
| `index` | `.chain/index/` 嵌入索引（meta.json + embeddings.bin，缓存失效指纹） |
| `stats` | `.chain/stats.json` 双时钟 + ACT-R 强度 + 参数区 + 反馈信号 |
| `retrieval` | recall 检索阶梯（向量 → 冷启动 → 关键词降级） |
| `consolidate` | BFS 连通分量蒸馏（[蒸馏] 骨架节点） |
| `audit` | audit.jsonl append-only 审计 |
| `code_map` | tree-sitter 代码骨架提取 + Mermaid |
| `schema` | .schema 版本读写 / adoption / 读者规则 |
| `migrate` | 五相幂等迁移（detect → backup → transform → verify → write） |
| `evidence` | 证据产物路径处理 |
| `version` | 四版本矩阵 + git 短哈希 |

依赖纪律：core 内部零入口 crate 依赖；186 单测全绿；自带 code_map 提取实战（en eng 提取自身 183 导出 / 437 调用边）。

