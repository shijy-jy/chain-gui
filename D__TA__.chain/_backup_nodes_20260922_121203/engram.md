---
id: engram
type: note
title: 项目 · Engram（知识图谱工具）
parent: 项目深挖
status: none
tags:
- Engram
- Tauri
- MCP
- 知识图谱
- 工具链
evidence:
- artifacts/engram/engram-侦察报告-2026-09-21.md
created: 2026-09-21T21:00:00+08:00
updated: 2026-09-21T21:00:00+08:00
revision: 1
---

> 触发：Engram；知识图谱工具；MCP 服务；.chain 是什么；Tauri 桌面应用

# 项目 · Engram（知识图谱工具）

> **归属（2026-09-21 本人确认）：本项目由我独立开发，从 0 写。** 素材来自 `G:\engram\.chain`（你自己的开发记录）+ `D:\AIworkspace\Engram\` 二进制元数据 + `G:\test1.x` 源码仓交叉核对。面试可以按"我独立设计并实现了这套工具"来讲——但**下面「风险与缺口」里两条会被追问穿**（2.10 快照的过期数值、缺规模化使用数据），讲之前先按现仓核对数字。

**一句话**：**开发者与 AI 共用的工程记忆图谱**——Tauri 2 桌面应用 + MCP stdio 服务，把 `.chain/nodes/*.md` 纯文本渲染成交互式图谱；"人看图、AI 读文件"，两者共享同一份事实源。配套 `engram-cli` 做迁移/索引/骨架。

**技术形态**：Rust 四 crate（core / mcp / gui / cli）+ Svelte 5 前端 + Tauri 2；NSIS 安装形态（`uninstall.exe`），语义模型随包内置（`bge-small-zh-v1.5` ONNX 94.8 MB）；`D:\AIworkspace\Engram\` 内有 24 个 `.old*` 备份可还原发布轨迹；`app.exe` ProductVersion **2.17.0**（Product=Engram, Company=chaingui）。

**源码仓**：节点里 `code_map: ../test1.x/crates` 指向 `G:\test1.x`，实测存在（`Cargo.toml` 四 crate、`docs/adr` 15 篇、`.github/workflows` 有 `ci.yml` + `release.yml`、7 组打包脚本、`CHANGELOG` 最新条目 **2.17.0 - 2026-09-21**，与 app.exe 版本互相印证）。

## 架构与设计（面试讲"我懂系统设计"的素材）

- **唯一事实源**：节点 `.md` 是唯一事实源；索引 / 统计 / 审计 / 骨架均为**可重建、可删的派生物**，绝不写回 YAML。
- **三层单向依赖**：Svelte 5 →`invoke`→ engram-gui；外部 AI →stdio→ engram-mcp；两者都只调 `engram_core::ops` 这一个写路径。
- **双模式 profile**：`.chain/.mode` 决定分析模式（strict：单根树/无环/无悬空、拒 note）或开发模式（dev：多根/孤立/环均可）。
- **生命周期字段**：`archived` + `archived_reason`、`derived`（检索降权 ×0.85）、`frozen`、`folded`、`code_map`、`content_hash`——全部 serde skip-if-false，**零破坏存量**。
- **检索阶梯**：向量召回（余弦，两档阈值 0.35/0.2）→ 冷启动兜底（创建时间 + 图谱度数）→ 关键词降级并**显式声明 `degraded`**；叠加 ACT-R 使用强度 $S=\ln\sum(\text{now}-t)^{-d}$（$d=0.5$）。
- **代码骨架**：tree-sitter 提取公开接口 + 签名 + `文件:行:列` + 调用边 + Mermaid 图 → `.chain/code_map/<id>.md`，带 `stale` 标记兜底。
- **接口契约**：MCP 工具从 v1 的 9 个演进到 v4 的 13 个（读 6 + 写 6 + 引导），写入全走串行 `write_lock` + 乐观锁；稳定错误码 `CONFLICT:` / `DUPLICATE_TITLE:` / `SCHEMA_TOO_NEW:` / `WORKSPACE_MODE_MISMATCH:` 等。
- **CLI 契约**：`migrate`（五相 detect→backup→transform→verify→write，失败回滚）、`reindex`、`sync-code-map`；退出码 0 成功 / 2 dry-run 将变更 / 3 校验失败 / 4 SCHEMA_TOO_NEW / 5 非工作区。

## 你在其中踩的坑与主张（工作区里的一手记录）

| 内容 | 说明 | 证据 |
|---|---|---|
| **BOM 导致静默跳过** | PowerShell `Set-Content` 写出的 BOM 让 reindex 静默跳过节点 → frontmatter 解析显式剥 BOM | `scanner-ops` |
| **PowerShell 5.1 调 MCP 必须 UTF-8 字节直写 stdin** | GBK 会把中文查询写坏，且**传输静默死亡** | `mcp-contract` |
| **`is_safe_id` 的边界与后果** | 只允许 ASCII 字母数字 `_-`（≤64，防路径穿越）→ 中文 id 节点无法经 MCP 读写链接，于是定下"**ASCII id + 中文 title**"约定，13 个节点无一例外 | `scanner-ops` |
| **前端卡顿归因** | 定位"全图每帧写 width/height"是主因 → 位置缓存 + 样式旁路收窄 + 波前 class 增量点亮 + 30 fps 节流 | `frontend`，提交 `07014c0` |
| **治理主张** | 迭代循环"只采集不自动调参，参数变更需人确认（治理权在人）"；ACT-R 由墙钟秒改为记忆时钟序数，修掉"昨天用过 < 从未用过"的硬伤 | `memory-l2` 补丁 1 |
| **工具链** | NSIS 静默安装 + 模型随包内置 + MCP 进程按需重生（替换 DSH 内置 MCP） | `engineering` |
| **watcher 边界** | `nodes/` 非递归、`archive/` 递归双监听 | `gui-cli` |

## 风险与缺口

- **工作区是 2.10.0 快照**：写于 2026-09-09，此后软件已到 **2.17.0**（12 天 2.10→2.17）。节点里的数值已过期：契约 v4→实际 v5、单测 186→197、docs "20+ 篇"→实际 34 篇、dev 指南副本 v7→出品 v8。→ **别直接引用节点里的数字**，要用先去 `G:\test1.x` 核对。
- **缺规模化使用数据**：`stats.json` 只有 3 个节点有触达记录、`audit.jsonl` 只有 1 条 recall（mode=vector, degraded=false, score 0.575→0.505）。→ 讲"检索效果好"没有数据支撑，只能讲机制。
- **弱证据**：记忆层理论（ACT-R 强度、双时钟、M-Code、蒸馏模板）多来自设计稿与补丁；"186/197 单测全绿"是 CHANGELOG 自述，未独立验证。
- 本机 `D:\AIworkspace\Engram\` 内**无任何文档/元数据文件**，`engram-mcp.exe` / `engram-cli.exe` 无嵌入版本资源（版本只能从 `app.exe` 与 CHANGELOG 交叉印证）。

## 可能被追问的三层

1. **原理层**：为什么"事实源与派生物"必须严格分离？向量召回为什么需要降级路径，降级为什么要显式声明？乐观锁 + 串行写队列各自防的是什么？
2. **实现层**：tree-sitter 骨架的 stale 怎么判定（`content_hash`）？`is_safe_id` 为什么不能放中文（路径穿越之外还有编码问题）？BOM 为什么会导致**静默**跳过而不报错？
3. **边界层**：这套工具为什么不做数据库而坚持纯文本 md？十几万节点还跑得动吗？如果给团队用，权限与并发怎么设计？

## 证据索引

- 工作区（本机可直接打开）：`G:\engram\.chain\`（`nodes/engram-framework.md` 及其 10 个子节点、`node-1`/`node-2` 孤立验证节点、9 个 `code_map` 骨架、`audit.jsonl`、`stats.json`）
- 源码与发布：`G:\test1.x`（四 crate、`docs/adr` 15 篇、`CHANGELOG.md`、`.github/workflows/`）、`D:\AIworkspace\Engram\`（`app.exe` 2.17.0、`engram-mcp.exe`、`engram-cli.exe`、`resources/models/bge-small-zh-v1.5/`）
