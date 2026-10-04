---
id: engram-gui-cli
type: note
title: Engram · engram-gui 与 engram-cli
parent: engram-framework
status: none
tags:
- Engram
- Tauri
- GUI
- CLI
code_map: ../test1.x/crates/engram-gui
revision: 1
updated: 2026-09-09T08:09:10+08:00
---

> 触发：Engram GUI 命令；Tauri 壳；engram-cli

# Engram · engram-gui 与 engram-cli

## engram-gui（Tauri 2 薄壳，`crates/engram-gui/`）

- `lib.rs`：Tauri Builder + 22 个 command 全部委托 core；dialog 插件；devtools false（发布）
- `commands.rs`：scan_chain / update_node / init_chain / get_ai_guide / get_guide_version / get_version_info / append_log / get_process_log / snapshot_chain / list_snapshots / read_snapshot / fold_chain / open_evidence / evidence_rel_path / create_node / delete_node / set_parent / list_workspaces / add_workspace / remove_workspace / get_code_map / reindex_embeddings
- `watcher.rs`：WatchState（watcher / mode / dir）——**nodes/ 非递归 + archive/ 递归双监听**，外部编辑或 AI 写入自动刷新
- tauri.conf.json：version 2.10.0 / identifier com.chaingui.desktop / frontendDist ../../dist / devUrl 1420 / bundle nsis + resources（engram-mcp.exe + models 随包内置）

## engram-cli（工作区维护工具，`crates/engram-cli/`）

- `migrate --workspace`（幂等迁移）· `reindex --workspace`（全库重嵌）· `sync-code-map --workspace [--lang rust] [--node <id>]`（tree-sitter 代码骨架提取）· `--version`（四版本矩阵 + git 短哈希）
- 退出码 0/2/3/4/5/1（见 schema 节点）

## 四版本矩阵（ADR 0011）

四个 crate 版本 + git 短哈希同步递增，MCP/CLI/GUI 三处 `--version` 同源（`core::version::VersionInfo`）；2.10.0 起安装包内置 engram-mcp.exe + 嵌入模型。

