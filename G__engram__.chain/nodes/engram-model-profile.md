---
id: engram-model-profile
type: note
title: Engram · 数据模型与双模式 profile
parent: engram-framework
status: none
tags:
- Engram
- 数据模型
- profile
- 词表
code_map: ../test1.x/crates/engram-core/src/model
revision: 1
updated: 2026-09-09T08:09:11+08:00
---

> 触发：Engram 数据模型；Node 字段；双模式；词表

# Engram · 数据模型与双模式 profile

## Node（`model/node.rs`，序列化进 frontmatter）

- 基础：id / title / body / created / updated / revision
- 词表：`type`（goal/design/task/verification/note）、`status`（pending/in_progress/success/failed/blocked/none）
- 结构：parent + rel + rel_desc（rel 三类型：contains 实线 / solves 虚线 / alternative 点线）
- 元数据：tags / evidence（artifacts 分层归档）
- 生命周期扩展（v2.10+，均 serde skip-if-false 零破坏存量输出）：`archived`+`archived_reason`（归档）、`derived`（[蒸馏] 检索降权 ×0.85）、`frozen`+`freeze_reason`（[待裁决] 冲突冻结）、`folded`（子链折叠摘要）、`code_map`（代码骨架挂载）、`content_hash`（stale 比对，skip 序列化）

## ChainSnapshot（`model/chain.rs`）

nodes（活跃图）+ edges + archived（M7' 归档分离）+ manifest（node_count / edge_count / active_chain / chain_health / project_persona）+ validation 报告。

## 双模式 profile（`profile.rs`）

模式是**配置**不是分支：`.chain/.mode` 标签（analysis/dev）→ `profile_for(ScanMode)` 取 Profile 包：

| 维度 | ANALYSIS（链协议） | DEV（自由知识库） |
|---|---|---|
| strict | true（严格单根树 / 无环 / 无悬空） | false（多根 / 孤立 / 环均可） |
| type 词表 | 4 类（拒绝 note） | 5 类（含 note） |
| status 词表 | 5 态（拒绝 none） | 6 态（含 none） |
| 指南 | AI_GUIDE v8 | AI_GUIDE_DEV v3 |

词表是唯一数据源（profile 常量），生产校验、MCP 提示、前端类型三处同源。

