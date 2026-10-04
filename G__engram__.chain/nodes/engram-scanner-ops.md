---
id: engram-scanner-ops
type: note
title: Engram · 扫描层与写入守门
parent: engram-framework
status: none
tags:
- Engram
- scanner
- ops
- 守门
code_map: ../test1.x/crates/engram-core/src/scanner
revision: 1
updated: 2026-09-09T08:09:10+08:00
---

> 触发：Engram 写入守门；扫描层；乐观锁；原子写

# Engram · 扫描层与写入守门

## 扫描层 `scanner/`

- `frontmatter`：YAML 头解析 + **显式剥 BOM**（PowerShell Set-Content UTF8 陷阱：不剥则 reindex 静默跳过节点）+ now_iso8601 手写 civil 算法（零依赖）
- `walker`：目录递归扫描、容错解析、归档分离（archive/ 递归只收 archived:true）、同 id 去重、双模式分流
- `validator`：字段级 9 条 + 结构级 5 条校验矩阵；校验器**反向生成规则文档**（指南与软件行为严格一致）

## 写入守门 `ops/`（唯一写路径，宪法第 4 条）

- **D2 词表守门**：type/status/rel 词表外拒绝或归一
- **D3 乐观锁 + 原子写**：expected_updated 不匹配 → `CONFLICT:`；M8' 冲突即冻结（ADR 0003）：CONFLICT → [待裁决] 前缀 + blocked + frozen，冻结期拒绝一切写入；落盘走 tmp+rename
- **D4 指南下发**：get_guide / 写后返回体提示
- `is_safe_id`：id 仅 ASCII 字母数字连字符下划线（≤64，防路径穿越）——**中文 id 节点 read/update/link 会被 MCP 拒绝**，故本库内化节点用 ASCII id + 中文 title
- 串行队列：MCP 侧 tokio Mutex 包裹全部写入工具，并发调用也串行落盘

## 稳定错误码（宪法第 7 条，文案可改码不可改）

`CONFLICT:` / `DUPLICATE_TITLE:`（同名拦截）/ `SCHEMA_TOO_NEW:` / `MIGRATE_FAILED:` / `VERIFY_FAILED:` / `INVALID_REL:` / `WORKSPACE_MODE_MISMATCH:` / `CONSOLIDATE_EMPTY:`——新增错误码须同步固化进 golden 契约。

