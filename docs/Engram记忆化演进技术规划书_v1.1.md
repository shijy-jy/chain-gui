# Engram 记忆化演进技术规划书

> **版本**：v1.1（修订版） · **日期**：2026-09-02 · **基线**：Engram v2.6.0（commit `e6d3539`） · **状态**：待执行
>
> **v1.1 修订说明**（相对 v1.0 的评审决议，均已在正文落实）：
> - **D1 · rel 数据格式**：保持现有单值格式（子节点 frontmatter `rel: contains | solves | alternative`），**不引入边列表数组**；「为什么连着」的说明用新增可选字段 `rel_desc`（阶段二），向后兼容。
> - **D2 · rel 词汇表**：阶段一只放行现有三值（与 GUI 线型渲染严格一致）；扩展词汇表由 AI_GUIDE v8 在阶段二收敛后同步放行，`get_guide` 返回的词汇表为唯一权威。
> - **D3 · 并发写保护提前至阶段一**：`update_node` 增加乐观锁（读-改-写冲突检测）+ tmp+rename 原子写 + server 串行队列；验收新增 CONFLICT 用例。
> - **D4 · 指南提醒内置**：`create_node` / `link_nodes` 的返回体附带 1–2 行关键规范提示；server 握手时下发当前指南版本号，不再依赖模型"自觉读指南"。

---

## 修订决议记录

| 编号 | 议题 | 决议 | 理由 |
|---|---|---|---|
| D1 | rel 存储格式 | 保持子节点 frontmatter 单值 `rel`；边说明用可选 `rel_desc` | 与 v2.4 实现、扫描器、GUI 线型渲染零冲突；数组化牵动全链路且无当前需求 |
| D2 | rel 词汇表 | P1 仅 `contains/solves/alternative`；扩展词表 P2 随指南 v8 收敛 | 词表即渲染契约，先收敛后放行，避免 AI 写出渲染不出来的关系 |
| D3 | 并发写保护 | 提前至 P1：乐观锁 + 原子写 + 串行队列 | 阶段一即存在"用户手改 vs AI 写入"竞态，成本低收益直接 |
| D4 | 指南执行保障 | 返回体内置提示 + 握手下发指南版本 | MCP 工具无法强制模型读文档，用返回体兜底 |

---

## 01 · 目标概述

让 AI 客户端把 Engram 工作区当作自己的外接记忆——**记忆独立于模型、透明可读、换大脑可携带**。

当前 Engram（v2.6.0）已完成图谱可视化、涟漪交互、双击聚焦、常驻信息栏等 GUI 能力，人机交互层成熟。本规划解决机器交互层：

- **可接入**：AI 客户端（Claude Desktop、Cursor、DeepSeek Harness 等）通过标准 MCP 协议发现并使用 Engram 的记忆能力；
- **可理解**：AI 读取时不只看到孤立文件，还能获得邻域、全局地图、路径等结构化关系叙述；
- **可维护**：AI 写入时遵守图谱健康规范（防重复、规范连边、冲突保护），记忆库长期不膨胀、不腐烂。

生态位不在「通用记忆 API」红海，而在开发者与研究者的人机共治工程记忆：透明（纯文本人可读）、可携带（文件夹即全部资产）、有生命周期（巩固与消退——竞品无人做「遗忘」）。

---

## 02 · 核心需求与技术约束

### 功能需求

- AI 客户端经 MCP 协议读写 Engram 工作区（stdio 本地传输）；
- 读取侧「关系叙述化」：节点邻域拼接、全局概览、涟漪式扩展、两点间路径；
- 写入侧图谱健康守门：创建前相似检查、结构化连边、**读-改-写冲突保护（D3）**、写前指南下发与返回体提示（D4）；
- GUI 与 MCP server 并行操作同一工作区，互不感知、互不阻塞。

### 非功能需求

- 本地优先：全部数据留本机，零云依赖；
- 可审计：任何 AI 写入动作有日志可查；
- 低侵入：GUI 现有功能零改动；
- 可分发：server 随 Engram 安装包一并交付，用户零额外安装。

### 刚性约束（不可突破）

- 节点文件保持纯文本 Markdown（YAML 头 + 自由正文），不引入任何二进制专有格式；
- **rel 保持现有单值字段格式（D1）**，本规划不得将其改为数组或边表；
- GUI（Tauri 应用）代码零改动或近零改动；
- 复用现有 Rust 代码库（src-tauri），不另起第二技术栈；
- server 写入后必须触发 GUI 既有文件监听自动刷新（P1 首个验证项）。

### 技术假设（待验证）

- MCP Rust SDK（rmcp）成熟度足以支撑 stdio server 开发——第 1 天 spike；
- DeepSeek Harness 当前为 v0.1 开发者预览版，官方声明存在破坏性变更——仅作为标准 MCP 客户端适配目标，不为其编写原生插件；
- 工作区规模在千级节点以内时，概览类统计现算性能可接受（当前实际规模远小于此）。

---

## 03 · 总体技术路线

**一个新编译目标，一堆旧文件，零 IPC。**

- AI 客户端通过 stdio 与 `engram-mcp` 进程通信（JSON-RPC）；
- `engram-mcp` 直接读写工作区 Markdown 文件；
- Engram GUI 通过既有文件监听自动刷新，与 server 无直接通信——**共享文件系统是唯一连接点**。

### 关键设计决策

1. **文件存事实，server 做叙述化**：邻域、概览、路径均由 server 读取时现算现拼，不回写文件；关系变更零同步成本。
2. **GUI 与 server 解耦**：不引入任何 IPC；server 写文件，GUI 靠既有 watcher 自动刷新，用户可实时「看着 AI 往记忆库里长东西」。
3. **工具设计 = GUI 交互翻译**：搜索框 → `search`；单击涟漪 → `expand(depth=1)`；双击聚焦 → `expand(depth=2)`；全图一览 → `get_overview`。
4. **写入守门（D3+D4）**：`create_node` 两阶段相似检查；`update_node` 乐观锁冲突检测；`get_guide` 随时下发最新读写规范，写入工具返回体附带关键提醒。
5. **rel 契约（D1+D2）**：写边即改写子节点 frontmatter 的单值 `rel`，取值限定在指南词汇表；阶段一只放行 `contains / solves / alternative`（与 GUI 线型渲染一致）。

### 替代方案对比

| 方案 | 描述 | 优势 | 代价 | 结论 |
|---|---|---|---|---|
| A · Rust bin target | src-tauri 内新增 engram-mcp 编译目标，图谱读写核心抽为共享 lib | 单一技术栈；随安装包分发；逻辑零重复 | 需做一次 lib 抽取重构 | **选用** |
| B · 独立 Node/Python server | 另起项目重新实现文件读写 | SDK 生态熟、起步快 | 第二运行时；读写逻辑两处维护；分发复杂 | 排除 |
| C · HTTP/SSE 远程 server | 网络服务形式暴露记忆能力 | 支持多设备、远程 AI | 本阶段无需求；引入认证与安全面 | 远期再议 |

---

## 04 · 分阶段实施计划

人天为单人全职技术估算（熟悉 Rust 与现有代码库为前提）。

### PHASE 1 · MCP 记忆服务（engram-mcp）——约 5–8 人天

**阶段目标**：Claude Desktop 接入 Engram，完成「读全局 → 搜节点 → 看邻域 → 写节点 → GUI 实时可见」全链路。

- 抽取 src-tauri 图谱读写核心为共享 lib；新增 bin target engram-mcp；
- 实现只读工具：`get_overview` / `search` / `read_node`（邻域拼接）/ `expand` / `read_path`；
- 实现写入工具：`create_node`（两阶段相似检查）/ `update_node`（**乐观锁 + 原子写，D3**）/ `link_nodes`（单值 rel，D1）；
- **写入基础设施（D3）**：tmp+rename 原子写；server 内写入串行队列；`update_node` 写前重读比对 `updated`，不一致返回 `CONFLICT` 且不落盘；
- **指南下发（D4）**：实现 `get_guide`；server 握手时下发当前指南版本号；写入工具返回体附带 1–2 行关键规范提示；
- 编写 Claude Desktop / dsh 客户端接入配置文档并实机验证。

**产出物**：engram-mcp.exe、共享 lib、客户端配置文档、验收记录。**依赖**：rmcp SDK spike（第 1 天）。

### PHASE 2 · 理解与检索增强——约 3–5 人天（向量检索另评 3–5 人天）

**阶段目标**：让 AI「读得懂联系，写得出好记忆」。

- AI_GUIDE 升级为读写双修版（v8）：何时新建节点、何时补充旧节点、何时连边、**rel 词汇表收敛定稿（D2）**；
- 边说明：子节点 frontmatter 新增可选 `rel_desc`（一句话说清为什么连着，D1），向后兼容；
- 语义向量检索（可选）：解决「搜『布局』找不到『力导向』」类同义词问题。克制原则——图结构本身是强语义索引（微软 GraphRAG 系列研究），关键词 + BFS 不够用时再启动。

**产出物**：AI_GUIDE v8、`rel_desc` 字段与解析、（可选）本地嵌入索引方案。

### PHASE 3 · 记忆生命周期（巩固 / 消退）——约 8–12 人天

**阶段目标**：记忆库具备生物学意义的生命周期——被反复访问的巩固，长期不用的消退归档（竞品无人做「遗忘」）。

- 访问计数与最后触达时间记录（server 侧统计，落 `.engram/meta`）；
- 消退策略：长期未触达节点降权、建议归档（归档 ≠ 删除，人可恢复）；
- 巩固策略：高频共现节点建议连边，弱关联定期体检；
- 多 AI 并发写协议：文件锁 + append-only 审计日志（P1 的乐观锁为第一道防线）；
- 对话自动提取（远期）：从 AI 会话沉淀候选节点，人审后入库。

**产出物**：生命周期策略模块、审计日志、归档工作区视图。**依赖**：阶段一稳定运行、写入量足以产生统计数据。

**远期（本版不承诺）**：dsh 原生记忆插件（待其插件 API 稳定）、多工作区注册表、HTTP 远程访问与多设备。

---

## 05 · 技术栈与工具链

| 层面 | 选型 | 理由 |
|---|---|---|
| 语言 / 形态 | Rust，src-tauri 新增 bin target | 与 GUI 同栈；单安装包分发；图谱核心抽 lib 零重复 |
| MCP 协议 | rmcp（官方 Rust SDK），stdio transport | 官方维护；stdio 是桌面客户端最通用接入方式 |
| 序列化 | serde / serde_json / serde_yaml | 现有依赖，直接复用 |
| 检索 | 阶段一：关键词 + BFS；阶段二可选：本地嵌入（如 fastembed） | 先结构后向量，避免过早引入重依赖 |
| 写入安全 | tmp+rename 原子写 + 乐观锁 + 串行队列（D3） | 单文件写入原子；读-改-写冲突显式返回 CONFLICT |
| 测试 | cargo test + stdio JSON-RPC 驱动脚本 | 协议层可无头自动化，不依赖真实 AI 客户端 |
| 分发 | 随 Engram NSIS 安装包附带 engram-mcp.exe | 用户零额外安装；文档给出配置片段 |

---

## 06 · 接口与数据模型

MCP 工具清单 v1：只读 6 个，写入 3 个，指南 1 个。

### 只读工具

- **`get_overview()`** —— 全局地图，AI 进入记忆库的第一站（对应人打开图谱扫一眼）。
  返回 `{ node_count, component_count, hubs: top10, clusters: 各聚类主题摘要, recent_changes: top10 }`
- **`search(query, limit=10)`** —— 标题 + 正文全文检索，找入口节点（对应 GUI 搜索框）。
  返回 `[ { id, title, snippet, score } ]`
- **`read_node(id, include_neighbors=true)`** —— 读单节点 + server 动态拼接「邻域名片」。
  返回 `{ meta, body, neighbors: [ { id, title, rel_type, rel_desc? } ] }`
- **`expand(id, depth=1..2)`** —— 涟漪的 AI 翻译：BFS 分层扩展；depth=1 即单击涟漪，depth=2 即双击聚焦。
  返回 `{ center, layers: [ { depth, nodes: [ { id, title, rel_to_parent } ] } ] }`
- **`read_path(from, to)`** —— 两点间最短链的沿路叙述，回答「A 和 B 有什么关系」。
  返回 `{ hops: [ { id, title, via_rel, via_desc? } ], reachable: bool }`
- **`get_guide()`** —— 返回 AI_GUIDE 最新全文（读写规范、rel 词汇表、节点粒度建议）。
  返回 `{ version, content_markdown }`

### 写入工具

- **`create_node(title, body, tags=[], force=false)`** —— 两阶段守门：发现相似节点且 force=false 时拒绝落盘，返回相似清单与建议（补充旧节点 or 确认新建）；AI 判断后以 force=true 重发。无状态设计，不依赖会话。
  返回 `{ status: CREATED | SIMILAR_EXISTS, id?, similar?: [ { id, title, overlap_reason } ], hints: [ 关键规范提示 1–2 行 ] }`（D4）
- **`update_node(id, mode, content)`** —— 更新已有节点。mode = `append`（默认，最安全）| `replace_body`（需指南授权场景）。
  **乐观锁（D3）**：写前重读比对 `updated`，若与上次读取不一致返回 `CONFLICT` 与两侧摘要，不落盘。
  返回 `{ status: UPDATED | CONFLICT, id, updated_at, hints?: [...] }`
- **`link_nodes(from, to, rel_type, desc?)`** —— 建立链接，**改写子节点 frontmatter 的单值 `rel`（D1）**；`rel_type` 必须在指南词汇表内（阶段一：`contains / solves / alternative`，D2）；`desc` 落 `rel_desc`（阶段二启用）。
  返回 `{ status: LINKED | ALREADY_LINKED | INVALID_REL, hints?: [...] }`

### 节点文件格式（与现有实现一致，向后兼容，D1）

```markdown
---
id: force-layout
title: 力导向布局
tags: [布局, 物理模拟]
parent: collision-force
rel: solves            # 单值：contains（默认）/ solves / alternative——与 GUI 线型渲染一致
rel_desc: 碰撞修正是斥力收敛的前置条件   # 阶段二新增，可选
created: 2026-08-20
updated: 2026-09-02
---

自由正文：设计决策、实测数据、历史变更……
```

### 客户端配置示例

```json
{
  "mcpServers": {
    "engram": {
      "command": "C:\\Program Files\\Engram\\engram-mcp.exe",
      "args": ["--workspace", "G:\\engram-workspaces\\main"]
    }
  }
}
```

---

## 07 · 风险与应对预案

| 风险 | 影响 | 应对 |
|---|---|---|
| rmcp SDK 不成熟或接口变动 | 开发阻塞或返工 | 第 1 天 spike 验证；锁定依赖版本；工具逻辑与协议层薄封装隔离 |
| dsh v0.1 预览版破坏性变更 | 适配失效 | 只走标准 MCP 接入，不写 dsh 原生插件；其插件 API 稳定后再评估 |
| AI 乱写导致图谱膨胀 | 记忆库信噪比恶化 | create_node 两阶段守门 + AI_GUIDE 粒度规范 + 返回体提示（D4）+ 阶段三消退兜底 |
| GUI 与 server 并发写同一文件 | 内容互相覆盖 | **P1 即防护（D3）**：tmp+rename 原子写 + server 串行队列 + update_node 乐观锁；多 AI 并发文件锁留阶段三 |
| watcher 未覆盖 server 外部写入 | GUI 不刷新，「看着 AI 写」体验断裂 | 阶段一首个验证项；现有 watcher 递归监听 `.chain`，大概率天然覆盖；若否补外部变更订阅（近零改动） |
| 大图谱概览现算性能 | get_overview 延迟 | 千级节点内现算无压力；超限后加增量缓存 |
| rel 词汇表被 AI 误用 | 线型渲染不出、关系语义漂移 | 阶段一仅放行三值（D2）；`link_nodes` 非法值返回 INVALID_REL；指南 v8 收敛后再扩展 |

---

## 08 · 测试与验收标准

### 测试策略

- **单元测试**：共享 lib 的图谱读写、BFS 扩展、最短路径、YAML 解析、乐观锁冲突判定（cargo test）；
- **协议测试**：脚本驱动 stdio JSON-RPC，对全部 10 个工具做冒烟与边界用例（不存在 id、非法 rel_type、相似冲突、**并发冲突 CONFLICT（D3）**、串行队列）；
- **watcher 端到端自动化**：复用 Rust 侧 ignored 集成测试先例，新增「server 写入 → GUI 数据源 2 秒内刷新」用例（D4 配套）；
- **回归测试**：GUI 现有功能全量手测（涟漪、聚焦、侧栏、滑条、切换图谱），确认零改动零退化。

### 端到端验收（Claude Desktop 实机）

1. `get_overview` 返回结构完整（节点数、枢纽、聚类、最近变动）；
2. `search` 按关键词命中目标节点；
3. `read_node` 返回正文 + 邻域名片；
4. `expand(depth=2)` 返回两层涟漪，与 GUI 双击聚焦范围一致；
5. `create_node` 先返回 SIMILAR_EXISTS，确认后落盘；
6. `update_node` 在外部修改后写回返回 CONFLICT（D3）；
7. GUI 在 2 秒内自动刷新显示新节点——整条链路的最终判据。

**通过标准**：七项全绿 + 回归无退化，方可发布 Engram 下一个 minor 版本。

---

## 09 · 后续演进与运维要点

- **版本策略**：engram-mcp 随 Engram 同版本发布；MCP 工具清单的任何变更记录 CHANGELOG（工具即对外契约）；
- **可观测**：server 侧结构化日志（调用方、工具名、耗时、写入动作、冲突事件）落本地文件，可审计可回放；
- **演进路线**：stdio → 可选 HTTP（多设备场景）；单工作区 → 多工作区注册表；关键词+BFS → 可选向量索引；rel 词汇表随指南 v8 收敛扩展；
- **生态跟进**：跟踪 MCP 协议版本、dsh 插件 API 稳定性、GraphRAG 类图检索研究，按阶段二/三节奏吸收；
- **运维交付**：客户端配置文档 + 故障排查清单（server 无法启动 / 客户端未发现工具 / GUI 不刷新 / CONFLICT 频发四类典型问题）。

---

*Engram 记忆化演进技术规划书 v1.1 · 2026-09-02 · 基线 Engram v2.6.0（commit e6d3539）· 规划书与实现工作分离：由规划使用者决定执行方式与节奏。*
