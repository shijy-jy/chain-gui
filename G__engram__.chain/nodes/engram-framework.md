---
id: engram-framework
type: note
title: Engram 工程框架
parent: null
status: none
tags:
- Engram
- 工程框架
- 代码地图
code_map: ../test1.x/crates
revision: 1
updated: 2026-09-09T08:15:27+08:00
---

> 触发：Engram 工程框架；Engram 代码结构；记忆系统架构；test1.x 项目框架

# Engram 工程框架

**Engram = 开发者与 AI 共用的工程记忆图谱**（Tauri 2 桌面应用 + MCP 服务）：把 `.chain/nodes/*.md` 纯文本渲染成交互式知识图谱。人看图、AI 读文件，双方共享同一份工程记忆；数据纯文本，git 可管、可迁移、任何编辑器可改。

- 源码仓库：`G:\test1.x`（origin/main，最新 0669fa0）
- 当前版本：**2.10.0**（2026-09-08）——记忆层 L2 全落地（契约 v4，13 工具，golden 18 条）
- 规模：cargo workspace 4 crate（core 19 模块）+ Svelte 5 前端（3 组件 + 7 lib）+ 15 篇 ADR + 20+ 篇设计文档
- 测试：core 186 单测 + CLI 契约 13 + golden 18 全绿；svelte-check 0 错
- 部署：D:\AIworkspace\Engram（NSIS 静默安装，模型随包内置）；DSH 会话通过 engram-mcp 读写本库（G:\water）

## 子节点目录（代码框架内化）

| 节点 | 一句话 |
|---|---|
| Engram · 分层宪法与依赖方向 | 9 条宪法 + 唯一写路径 + 事实源/派生物边界 |
| Engram · engram-core 模块地图 | 19 个模块逐句职责（唯一知道规则的地方） |
| Engram · 数据模型与双模式 profile | Node 字段集 / 词表 / rel 三类型 / 模式是配置不是分支 |
| Engram · 扫描层与写入守门 | frontmatter/walker/validator + D2/D3/D4 + 原子写 + 错误码 |
| Engram · 记忆层 L2 六件套 | embed/index/stats/retrieval/consolidate/audit |
| Engram · schema 版本与幂等迁移 | .schema 1.1 + migrate 五相 + 退出码 |
| Engram · engram-mcp 工具契约 | 13 工具 / 契约 v4 / golden / 串行队列 |
| Engram · engram-gui 与 engram-cli | Tauri 20+ 命令 + watcher + CLI 三子命令 |
| Engram · 前端图谱与涟漪交互 | App.svelte / ripple BFS / cytoscape / 侧栏 |
| Engram · 工程质量与发布体系 | 契约测试 / CI / 供应链 / ADR / 安装包 |

## 三层架构一句话

```text
Svelte 5 前端 ──invoke──▶ engram-gui（Tauri 命令薄壳）
外部 AI 会话 ──stdio───▶ engram-mcp（rmcp 薄壳）
                           │ 都只调 engram_core::ops（唯一写路径）
                           ▼
                     engram-core（规则/守门/扫描/检索）
                           ▼
              .chain/nodes/*.md（唯一事实源，纯文本）
```

维护提示：源码框架大改后回读对应子节点同步；新版本落地时更新本节点版本行并与 CHANGELOG 对照。

