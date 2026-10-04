---
id: engram-mcp-contract
type: note
title: Engram · engram-mcp 工具契约
parent: engram-framework
status: none
tags:
- Engram
- MCP
- 工具契约
- golden
code_map: ../test1.x/crates/engram-mcp
revision: 1
updated: 2026-09-09T08:09:10+08:00
---

> 触发：Engram MCP 工具；工具契约；golden 契约测试

# Engram · engram-mcp 工具契约

`crates/engram-mcp/src/main.rs`（约 370 行薄壳）：rmcp 框架 stdio 传输，`--workspace <目录>` 指定工作区。**stdout 是 JSON-RPC 协议通道，日志只能走 eprintln!(stderr)**。逻辑全在 `engram_core::ops`，本 crate 只做参数 schema + 协议映射（Ok → JSON 文本 / Err → 协议级错误）。

## 13 工具（契约 v4，2.10.0）

- 读：`get_overview` / `search` / `read_node` / `expand` / `read_path` / `get_guide`
- 语义：`recall`（检索阶梯 + degraded 显式声明）
- 写（全部先拿 write_lock 串行队列）：`create_node` / `update_node` / `link_nodes` / `archive_node` / `unlink_nodes` / `consolidate`

## 工具即契约（宪法第 8 条）

- golden 契约测试：真实 engram-mcp 进程 18 条调用逐条比对（`tests/golden_contract.rs` + `tools/_collect_golden.ps1` 重固化）
- 工具增删改必须：更新 golden 契约 + CHANGELOG 记录 + 版本矩阵工具契约版本递增
- 契约版本轨迹：v1（9 工具，2.7.0）→ v2（+recall，2.9.0）→ v3（+archive/unlink，12 工具）→ v4（+consolidate，13 工具）

## 已知边界（写节点必读）

- id 安全校验：仅 `[A-Za-z0-9_-]` ≤64——中文 id 节点无法通过 MCP read/update/link（可文件直改，watcher 自动刷新）
- PowerShell 5.1 调 MCP 须 UTF-8 字节直写 stdin（GBK 会把中文查询写坏、传输静默死亡）；golden 全 ASCII 不受影响

