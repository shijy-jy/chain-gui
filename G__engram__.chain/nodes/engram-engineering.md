---
id: engram-engineering
type: note
title: Engram · 工程质量与发布体系
parent: engram-framework
status: none
tags: [Engram, 工程质量, CI, 发布]
---

> 触发：Engram 测试体系；CI 发布；供应链；ADR

# Engram · 工程质量与发布体系

## 测试矩阵（2.10.0 全绿）

| 层 | 内容 | 数量 |
|---|---|---|
| core 单测 | 模型/扫描/守门/迁移/检索（embedder stub 注入，不依赖真实模型） | 186 |
| CLI 契约 | migrate/reindex 退出码用例 | 13 |
| golden 契约 | 真实 engram-mcp 进程逐条调用比对 | 18 |
| 真实模型回归 | BGE 端到端（#[ignore]，CI 无模型自动跳过） | 1 |
| 前端 | svelte-check 0 错 + vite build | — |

## CI（.github/workflows）

四版本矩阵 + git 短哈希注入 → NSIS 打包 → golden 契约 job → cargo-audit + cargo-deny 供应链门禁（deny.toml + docs/deps-justification.md 依赖理由清单）。

## ADR 决策记录（docs/adr/，15 篇）

file-as-truth / rel-three-types / conflict-freeze / derived-not-in-truth / single-instance-multi-workspace / retrieval-ladder / actr-strength / dual-clock / consolidate-community-summary / code-map-derived / version-matrix / gui-zero-break / schema-versioning / workspace-core-sink。**架构条款变更必须走 ADR**。

## 文档地图（docs/，20+ 篇）

架构设计终版 v1.0 / 安全模型 v1 / schema v1 定义与迁移接口 / 记忆层理论整理与评估（+补丁 1 参数迭代方法论）/ 记忆层与 M-Code 实现框架 v1 / 设计批判与深水区分析 v2.0 / 阶段性整理 / 未来规划阶段整理 / 模块详细设计 / 设计论文 / 架构建议。另有 `ai_workspace/`（多 AI 协作：ai-coordinator / ai-frontend / ai-rust / ai-qa + CODE_STATE 基线）、`front_docx/`（AI 加入协议）、`mem-log/`。

## 发布链路

四版本矩阵递增 → CHANGELOG 定版 → `_build_v2100.bat`（NSIS，模型随包内置 +92MB）→ 静默安装 `/S /D=` → 替换 DSH MCP（改名 + 紧环复制，进程按需重生）→ `git push`（网络不稳时留本地）。当前部署：D:\AIworkspace\Engram（2.10.0）。
