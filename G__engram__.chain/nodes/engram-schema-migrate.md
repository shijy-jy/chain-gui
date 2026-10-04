---
id: engram-schema-migrate
type: note
title: Engram · schema 版本与幂等迁移
parent: engram-framework
status: none
tags:
- Engram
- schema
- 迁移
- migrate
code_map: ../test1.x/crates/engram-core/src/migrate.rs
revision: 1
updated: 2026-09-09T08:15:28+08:00
---

> 触发：Engram schema 版本；幂等迁移；migrate

# Engram · schema 版本与幂等迁移

## `.chain/.schema`（宪法第 9 条，ADR 0013）

- 格式 `major.minor`；**缺失 = 隐式 1.0**；当前 `CURRENT_SCHEMA_STR = 1.1`
- 三位一体：frontmatter 字段集 / 目录结构 / 索引格式
- major = 破坏性事实源变更 → 必须走幂等迁移；minor = 加性字段 / 派生物格式 → 仅重建派生物
- 旧软件遇更高 major 一律拒绝打开（`SCHEMA_TOO_NEW:`）；扫描器除「忽略未知可选字段」外无任何版本 if 分支

## 幂等迁移 `engram-cli migrate --workspace <path>`

五相：**detect → backup → transform → verify → write**；失败回滚；登记 1.0→1.1 为 B 类一步（派生物落地，重建靠 reindex）。支持 `--dry-run / --no-backup / --json / --to`（当前仅支持迁到当前版本）。

## CLI 退出码契约（《schema v1 定义与迁移接口》§5.2）

`0` 成功或已是最新 · `2` dry-run 将变更（未落盘）· `3` 校验失败（已回滚）· `4` SCHEMA_TOO_NEW · `5` 非工作区/参数非法 · `1` 其他错误。退出码由 cli_contract.rs 测试固化。

