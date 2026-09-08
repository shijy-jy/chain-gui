# Engram 记忆层与 M-Code 实现框架（v1）

> **版本**：v1.0（前置准备交付物）· **日期**：2026-09-08
> **依据**：《三大层分析与设计 v3.0》（记忆层 M-1~M-5）、《记忆化演进技术规划书 v1.1》、《阶段性整理_现状与双模式规划 v1.0》（M-Code）、《架构设计终版 v1.0》（§1.1 core 职责 / §1.3 数据布局 / §5 9→13 工具 / §6 可观测性）、ADR 0003/0006–0010、宪法 9 条
> **性质**：本框架钉死技术方案、模块与函数封装、输入输出接口；**不包含函数具体实现**。实现按 §8 顺序分阶段进行，每阶段独立审核。

---

## 1 · 范围与总原则

- 覆盖：检索阶梯 L1–L5、ACT-R 强度 + 双时钟、stats 统计、4 个新 MCP 工具（9→13）、蒸馏、冲突冻结、重复检测、M-Code 代码骨架。
- 三条红线沿用：**事实源不脏**（一切统计/索引/骨架进派生区，可重建）；**检索降级链**（任何增强失效退化关键词并显式声明，宪法第 6 条）；**GUI 零破坏**（全部加性）。
- 版本联动（实现时必须同步）：schema 1.0→**1.1**（minor，B 类：派生文件格式落地）；工具契约 v1→**v2**（9→13，golden 重固化）；指南 v7→**v8**（分析，L1 编码规范）/ v2→**v3**（开发）。

## 2 · 技术方案定案表

| # | 决策点 | 定案 | 出处/理由 |
|---|---|---|---|
| T1 | 检索阶梯 | **规范 L1–L5（查询表示 × 记忆表示逐级放宽，ADR 0006）**：L1 精简线索↔trigger；L2 多线索↔trigger；L3 大意↔线索；L4 三内容（title/body/tags）↔关键线索；L5 三内容↔大意。**可见性规则**：归档节点自 L4 起可见（ADR 0006）；蒸馏骨架 L4 可见（ADR 0009）。**机制对应**（三大层 M-3）：L1=编码侧（指南 v8：trigger 句 + tags）；L2=语义召回（`recall(query,k)` 向量）；L3=联想扩散（expand 升级，rel_desc 边权，P1：本框架只定接口）。 | ADR 0006/0009、三大层 M-3 |
| T2 | 强度公式 | `S = ln(Σ (now − t_j)^(−d))`，**零人工权重**；d 初始 0.5，由命中/未命中反馈校准（校准数据经 §6 指标采集） | ADR 0007 |
| T3 | 双时钟 | 墙钟 = `now_iso8601()`；记忆时钟 = **每次工具调用 +1**（ADR 0008 字面）落 stats.json 全局计数；**节点触达**（读命中/写/recall miss）另计于 per_id，用于强度公式——两者是不同计数器，勿混 | ADR 0008 |
| T4 | 冷启动 | 强度为空（新库）→ 排序退化「创建时间 + 图谱度数」并**显式声明** | 三大层 C4 |
| T5 | 强度信号 | `f(访问次数, 最近访问, 图谱度数, importance)`；importance 首版单档 `high`，只影响排序不参与归档 | 三大层 M-4 |
| T6 | 归档 | `archive_node`：`archived: true` + 标题前缀 `[归档]`，文件移入 `.chain/archive/`；90 天未触达为建议阈值（仅提示，不自动执行）；`unlink_nodes` 补断边 | 三大层 M-4（archive 目录已存在，fold 在用） |
| T7 | 冲突冻结 | 并发双写 → 要么 `CONFLICT:` 要么冻结 `[待裁决]`（status 置 blocked + 标题前缀），**绝不静默覆盖** | ADR 0003 |
| T8 | 重复检测 | create 两阶段：①标题相似度启发式（归一化后编辑距离/公共子串）②嵌入余弦 > 阈值（0.9）→ hint「疑似重复」，复用 `alternative` 边表达竞争；force 放行 | 三大层 M-4.3 |
| T9 | 蒸馏 | `consolidate`：**dry_run 默认**；产物 = **BFS/共现聚类**簇摘要 + 逐条来源引用 + `derived: true` + 标题前缀 `[蒸馏]`；检索自 **L4** 起可见、derived 默认降权；人审摘帽 = 用户删除 derived 标记后转普通节点 | ADR 0009（聚类算法 = BFS/共现，非 Louvain） |
| T10 | 嵌入后端 | fastembed **=6.0.3**（精确锁）+ ort **=2.0.0-rc.13**（精确锁，防 rc 漂移）+ BGE-small-zh-v1.5（512 维，本地缓存文件集；spike 实测读取 5 个核心文件：tokenizer.json / config.json / special_tokens_map.json / tokenizer_config.json / model_optimized.onnx）；同义词门槛复用 `docs/test-assets/synonym-testset.json`（已 4 组全过） | 准备阶段 F 实测 |
| T11 | 模型分发 | **建议**：模型随安装包内置（+92MB，离线可用，符合本地优先）；备选：首启从 hf-mirror 下载（网络不可靠，本机 github/hf 均被墙）——**待用户拍板** | 本框架提案 |
| T12 | M-Code 试点语言 | **建议 Rust**（自己的项目、tree-sitter-rust 成熟、可立即用 Engram 自身验证）——**待用户拍板** | 阶段性整理 §3.3 待决 |
| T13 | M-Code 提取 | tree-sitter：公开接口优先（pub fn/struct/trait + 签名）→ 调用边 → Mermaid 文本；骨架挂载 `code_map: <相对路径>` frontmatter 引用 `.chain/code_map/<node-id>.md`；正文只放一句概述 | ADR 0010、阶段性整理 §3.3 |
| T14 | stale 联动 | watcher 扩展监听代码目录（分析模式）→ 对应骨架标 `stale: true` → AI 进场调 `refresh_code_map`（安静优先）+ stale 兜底 | ADR 0010 |
| T15 | 派生物清单 | `.chain/index/`（嵌入索引）、`.chain/stats.json`、`.chain/code_map/`、`.chain/audit.jsonl`——全部可重建、不进 YAML | ADR 0004、终版 §1.3 |

## 3 · 模块布局（crate 内新增，全部在 core）

```
crates/engram-core/src/
├── embed.rs          ← 嵌入后端封装（fastembed + 本地模型加载；可插拔 trait）
├── index.rs          ← .chain/index/ K 存取、哈希校验、全库重嵌
├── stats.rs          ← 双时钟、触达计数、强度公式、线索缺口清单
├── retrieval.rs      ← recall 阶梯编排（L2–L5）+ 降级链
├── consolidate.rs    ← 社区聚类摘要、逐条引用、人审摘帽语义
├── code_map.rs       ← tree-sitter 提取、stale 标记、Mermaid 生成
└── ops/ (扩展)       ← +4 工具：recall / archive_node / unlink_nodes / consolidate

crates/engram-cli/    ← +2 子命令：reindex（全库重嵌）/ sync-code-map（骨架生成与刷新）
crates/engram-mcp/    ← 注册 4 新工具；instructions 增补错误码与 schema 版本
crates/engram-gui/    ← 加性：归档视图开关、[待裁决]/[蒸馏] 徽标、Mermaid 渲染、重嵌按钮
```

依赖新增（core）：`fastembed = "=6.0.3"`、`tree-sitter` + `tree-sitter-rust`（语言待拍板后换）。fastembed/ort 的编译已验证（阶段 F）。

## 4 · 数据布局增量（schema 1.1 记录，全部可重建）

```
.chain/
├── index/
│   ├── embeddings.bin    ← 节点嵌入（f32 行存；id → 行号映射在 meta.json）
│   ├── meta.json         ← { format: 1, dim: 512, model: "bge-small-zh-v1.5",
│   │                         hash: "<节点文件内容哈希>" , updated_at }
│   └── (任一节点文件变更 → 该 id 条目 stale，recall 时按需重嵌)
├── stats.json            ← { clocks: { wall, memory }, per_id: { reads, writes, last_touch, strength },
│   │                         gaps: [线索缺口], calibrate: { d } }
├── code_map/<node-id>.md ← 骨架（Mermaid + 签名清单 + stale 标记）
└── audit.jsonl           ← append-only：写入/迁移/蒸馏/归档动作
```

## 5 · 函数封装与输入输出接口（核心交付）

### 5.1 embed.rs（嵌入后端）

```rust
pub trait Embedder: Send + Sync {
    fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError>;
    fn dim(&self) -> usize;
}
pub enum EmbedError { LoadFailed(String), InferenceFailed(String) }

/// 本地模型加载（默认路径：%LOCALAPPDATA%\Engram\models\bge-small-zh-v1.5；随包分发时改为安装目录相对路径）
pub fn load_local_embedder(model_dir: Option<PathBuf>) -> Result<Box<dyn Embedder>, EmbedError>;
/// 降级链兜底：无模型/加载失败时返回 None（调用方退化关键词检索并显式声明）
pub fn try_load_embedder() -> Option<Box<dyn Embedder>>;
```

### 5.2 index.rs（嵌入索引）

```rust
pub struct IndexStore { /* 私有：路径、meta、缓冲 */ }
impl IndexStore {
    pub fn open(root: &Path) -> Result<Self, String>;              // 懒加载；缺失 = 空索引
    pub fn is_stale(&self, id: &str, content_hash: &str) -> bool;
    pub fn upsert(&mut self, id: &str, hash: &str, vec: Vec<f32>) -> Result<(), String>;
    pub fn remove(&mut self, id: &str) -> Result<(), String>;
    pub fn flush(&mut self) -> Result<(), String>;                 // 写 embeddings.bin + meta.json
    pub fn vec(&self, id: &str) -> Option<&[f32]>;
    pub fn ids(&self) -> impl Iterator<Item = &str>;
    pub fn rebuild_all(root: &Path, embedder: &dyn Embedder) -> Result<RebuildReport, String>;
}
pub struct RebuildReport { pub total: usize, pub re_embedded: usize, pub elapsed_ms: u64 }
// 验收判据：千节点全库重嵌 < 60s
```

### 5.3 stats.rs（双时钟 + 强度）

```rust
pub struct StatsStore { /* 私有 */ }
impl StatsStore {
    pub fn open(root: &Path) -> Result<Self, String>;              // 懒加载；缺失 = 空
    pub fn clock(&mut self) -> ClockSnapshot;                      // { wall, memory }
    pub fn bump_memory_clock(&mut self);                           // 全局：每次工具调用 +1（ADR 0008）
    pub fn touch(&mut self, id: &str, kind: TouchKind);            // per_id 触达（与全局时钟解耦）
    pub fn strength(&self, id: &str) -> Option<f32>;               // ACT-R 公式（T2）
    pub fn cold_start_rank(&self, id: &str) -> f32;                // 强度为空时的退化排序因子（T4）
    pub fn gaps(&self) -> Vec<String>;                             // recall 失败日志 → 线索缺口清单
    pub fn flush(&mut self) -> Result<(), String>;
}
pub struct ClockSnapshot { pub wall: String, pub memory: u64 }
pub enum TouchKind { ReadHit, Write, RecallMiss }
```

### 5.4 retrieval.rs（recall 阶梯编排）

```rust
/// 阶梯（ADR 0006 规范 L1–L5）：L1 精简线索 / L2 多线索 / L3 大意 / L4 三内容（含归档、蒸馏产物）/
/// L5 三内容↔大意；实现上按相似度阈值与 k 逐级放宽，任何增强失效 → 退化关键词并显式声明（宪法第 6 条）。
/// 归档节点：索引保留其嵌入（meta 标 archived），默认过滤、include_archived=true 时纳入（自 L4 起可见）。
/// derived（蒸馏）节点：默认降权。
pub fn recall(ctx: &Workspace, query: &str, k: Option<usize>, include_archived: bool) -> Result<Value, String>;// 返回 JSON 一级键：{ query, total, returned, mode: "vector"|"keyword"|"cold-start",
//   degraded: bool, degrade_reason: Option<String>, results: [{ id, title, score, sources? }] }
pub fn expand_activated(ctx: &Workspace, id: &str, depth: Option<u32>) -> Result<Value, String>;
// expand 升级（L3）：rel_desc 作边权、度归一化——P1 阶段，接口先定
```

### 5.5 四个新 MCP 工具（契约 v2，golden 重固化到 13 条）

| 工具 | 参数（schema 字段） | 返回一级键 |
|---|---|---|
| `recall(query, k=10, include_archived=false)` | `query: String`, `k: Option<usize>`, `include_archived: bool` | `query,total,returned,mode,degraded,degrade_reason,results[{id,title,score,sources?}]` |
| `archive_node(id, reason?)` | `id: String`, `reason: Option<String>` | `archived:true,id,title,archived_to,reason,hint` |
| `unlink_nodes(from,to)` | `from: String`, `to: String` | `unlinked:true,from,to,rel_removed,hint` |
| `consolidate(targets?, dry_run=true, k=8)` | `targets: Option<Vec<String>>`, `dry_run: Option<bool>`, `k: Option<usize>` | `dry_run,plan:[{cluster_id,members,sources,summary_preview}],hint`；dry_run=false 时另加 `created:[{id,title,derived}]` |

约束：`archive_node`/`unlink_nodes`/`consolidate` **开发模式为主**（知识库维护，阶段性整理 §2.2），`consolidate` 分析模式共享（骨架蒸馏语义，§3.5）；`consolidate` dry_run 默认 true；`recall` 双模式可用（同阶梯）。

### 5.6 consolidate.rs（蒸馏核心）

```rust
pub struct ConsolidatePlan { pub clusters: Vec<Cluster> }
pub struct Cluster { pub members: Vec<String>, pub sources: Vec<String>, pub summary: String }
pub struct ConsolidateReport { pub created: Vec<String>, pub derived: bool }

pub fn plan(root: &Path, targets: Option<&[String]>, k: usize) -> Result<ConsolidatePlan, String>;
pub fn run(root: &Path, targets: Option<&[String]>, k: usize) -> Result<ConsolidateReport, String>;
// 产物节点：derived:true + 标题 [蒸馏] + 正文含逐条来源引用（source 节点 id 列表）
// 人审摘帽：删除 derived 标记（GUI 徽标 + 按钮，加性）
```

### 5.7 code_map.rs（M-Code）

```rust
pub struct Skeleton {
    pub node_id: String,
    pub language: String,
    pub exports: Vec<ExportSig>,        // { name, kind: fn|struct|trait|impl, signature, file:line }
    pub call_edges: Vec<CallEdge>,      // { from, to, file:line }
    pub mermaid: String,
    pub stale: bool,
}
pub struct ExportSig { pub name: String, pub kind: String, pub signature: String, pub loc: String }
pub struct CallEdge { pub from: String, pub to: String, pub loc: String }

pub fn extract_skeleton(src_root: &Path, node_id: &str, lang: &str) -> Result<Skeleton, String>;
pub fn refresh_code_map(root: &Path, node_id: &str) -> Result<Skeleton, String>;   // 重新提取并写派生文件
pub fn mark_stale(root: &Path, node_id: &str) -> Result<(), String>;               // watcher 联动
pub fn skeleton_to_markdown(s: &Skeleton) -> String;                               // 写 .chain/code_map/<id>.md
```

### 5.8 CLI 扩展

```
engram-cli reindex --workspace <p>            # 全库重嵌（清空重建 index/）
engram-cli sync-code-map --workspace <p> [--lang rust] [--node <id>]
```

### 5.9 错误码增量（宪法第 7 条同步更新）

`RECALL_INDEX_MISSING:`（索引缺失且降级失败）/ `EMBED_FAILED:`（嵌入后端故障，降级声明用）/ `CONSOLIDATE_EMPTY:`（无可蒸馏簇）。`INVALID_REL:` 与 `WORKSPACE_MODE_MISMATCH:` 一并补齐（原目标态）。

### 5.10 现有接口集成点

- `Workspace`：增加 `stats: StatsStore`、`index: IndexStore` 惰性字段（不影响现有 9 工具输出，golden 1.0 条目不回归）；
- 写入路径（create/update/link/archive/unlink/consolidate）：成功后写 audit.jsonl + 触发 index 条目标 stale；
- 读路径（read_node/search/recall/expand）：触达回写 stats（Write 后同步 flush）；
- watcher：监听目录扩展 `archive/` 与代码目录（分析模式）；
- GUI：归档视图开关、`[待裁决]`/`[蒸馏]` 徽标、Mermaid 渲染、重嵌按钮——全部加性。

## 6 · 可观测性（终版 §6 落地）

- 指标采集点：工具调用延迟、recall 命中率（→ d 校准）、CONFLICT 计数、线索缺口清单；
- 采集数据落 stats.json 的 `calibrate` 区，不进 YAML；
- 校准规则：连续 N 次（初值 50）命中/未命中反馈后重估 d（实现时以真实数据回归，不预设数值）。

## 7 · 测试与验收判据

| 判据 | 目标 |
|---|---|
| 同义词门槛 | `docs/test-assets/synonym-testset.json` 4 组全过（已实测过的门槛，集成后回归） |
| 语义召回 | `recall("布局")` 命中「力导向布局」且排序首位 |
| 重嵌性能 | 千节点全库重嵌 < 60s（`RebuildReport.elapsed_ms`） |
| 降级链 | 删除模型/索引后 recall 退化为关键词且 `degraded:true` 显式声明 |
| 并发 | 双进程同写 → 要么 `CONFLICT:` 要么 `[待裁决]`，绝不静默覆盖 |
| golden 契约 | 13 工具全量固化（新增 4 工具的用例 + 原 11 条调用不回归） |
| 大图分级 | <1k / 1k–10k / >10k 三档性能基线 |
| M-Code | 用 Engram 自身验证：提取 engram-core 骨架 → 人读骨架能还原模块职责（试点验收） |

## 8 · 实现顺序（对应三大层 §3.4）

1. **M6'**：指南 v8/v3（L1 编码规范）+ embed.rs + index.rs + `recall`（含降级链、失败日志）→ golden 10 工具；
2. **M7'**：`archive_node`/`unlink_nodes` + stats.rs 强度/双时钟 + watcher archive 扩展 → golden 12 工具；
3. **M8'**：冲突冻结 `[待裁决]` + 重复检测 + `consolidate`（dry_run）+ audit.jsonl → golden 13 工具；
4. **M-Code**：code_map.rs（tree-sitter 试点语言）+ CLI sync-code-map + GUI Mermaid/徽标；
5. **记忆图网络 4.1→4.2**：图注意力检索（接口已留，本框架不展开）。

## 9 · 待定清单的裁定（实现启动时已按建议拍板）

> 以下 5 项按建议值拍板（2026-09-08，用户确认），实现以此为准：

1. 模型分发：**随安装包内置**（+92MB，本地优先；发布脚本把模型目录并入 bundle resources）；
2. M-Code 试点语言：**Rust**（tree-sitter-rust；用 Engram 自身验证）；
3. 阈值初值：归档建议 **90 天**、重复检测余弦 **0.9**、d 校准窗口 **50 次**——真实数据回归后调；
4. 前缀字面：归档 = `archived: true` + 标题前缀 `[归档]`；蒸馏 = `derived: true` + 标题前缀 `[蒸馏]`；冲突冻结 = status `blocked` + 标题前缀 `[待裁决]`；
5. schema 版本：派生文件落地记为 **1.1**（minor，B 类迁移）。

## 10 · 与宪法/ADR 对应

| 本文 | 依据 |
|---|---|
| T1–T4、§5.4 recall | ADR 0006/0007/0008、宪法第 6 条 |
| T6/T7/T9、§5.5/5.6 | ADR 0003/0009 |
| T12–T14、§5.7 | ADR 0010、阶段性整理 §3.3 |
| T15、§4 | ADR 0004、终版 §1.3、schema 文档（minor 语义） |
| §5.9 错误码 | 宪法第 7 条（实现时同步更新 ARCHITECTURE.md） |
| §5.10 GUI 加性 | 宪法第 5/6 条（零破坏） |
