# Engram 三层重构 · 设计整理 v1

> **状态**：设计定稿（待实现）· **日期**：2026-10-03
> **关系**：本文是《三大层分析与设计 v3.0》的**重构版**——层名与骨架沿用，但两处实质性变化：
> ① 记忆层从"机制集合"升级为**由对话文件驱动的记忆系统**（补齐了理论 QKV 中一直缺失的 Q 的来源）；
> ② 增加"层与层接口隔离"的硬要求，并据此收掉了现有代码里的旁路。
> **性质**：本文件定义边界、接口与格式；**不含实现细节**。实现按 §12 分阶段进行。
> **代码进度**：**P0/P1/P2/P3/P4 全部落地**（记忆层专项除外）。对话模块 + `remember` / `dialogue_status` / `resolve_conflict` + 全响应 `guide_version` + 契约 v8（五个节点直写工具已移除，golden 28 条）+ GUI 人治写通道已删除（ADR 0015，文件树/信息栏降只读）+ legacy 祖父条款（扫描期隐式 `origin: legacy`，不回写文件）+ 对话阅读面（`get_dialogue` 命令 + `DialogueReader.svelte`，渲染期 md 投影不落盘）+ 文件层拆分为独立 crate `engram-file`（94 测试，编译期依赖方向强制）。指南 v18/v12。测试：file 95 / core 134 / cli 13 / golden 28 全绿；svelte-check 0 错 0 警；CDP 实跑验证通过。

---

## 1 · 一句话定位

Engram = **对话为原始输入、节点为记忆脉络、图谱与文件树为观察面**的工程记忆系统。
AI 是唯一的记忆生产者与消费者；软件提供**规则容器**与**观察窗口**。

> 用户原话（语义基准）：**对话就像别人给你讲解的内容，节点就像你通过别人的讲解分析整理好的脉络。**

---

## 2 · 三层边界

| | 文件层 | 记忆层 | 显示交互层 |
|---|---|---|---|
| **定位** | 事实源与原始输入的存储 | **系统核心逻辑**：规则与记忆动力学 | 观察面与操作面 |
| **负责** | `.chain/nodes/*.md` 扫描与原子读写；`.chain/dialogue/` 原样读写；watcher；schema/迁移 | 守门、唯一写路径、嵌入索引、强度与触达、归档、蒸馏、审计、**对话消费** | 渲染（图谱 / 文件树 / 对话阅读面）、搜索、信息栏、操作入口 |
| **输入** | 路径与字节 | 文件层原样数据 + AI 的 MCP 调用 | `MemoryView` + 用户操作 |
| **输出** | 原始数据（bytes / 扫描结果） | 记忆视图、决策留痕、派生数据 | 屏幕与交互事件 |
| **禁令** | ❌ 不认识"对话"语义 ❌ 不认识"记忆"概念 ❌ 不做校验 | ❌ 不 import GUI/CLI ❌ 不产 JSON（JSON 属接口适配） | ❌ **禁止直读** `index/`、`stats.json`、`audit.jsonl`、`code_map/` ❌ 禁止直接写文件 |
| **现落点** | `scanner/`、`dialogue.rs`、`ops::atomic_write`、`watch`、`schema`、`migrate` | `dialogue_log.rs`、`embed`/`index`/`stats`/`retrieval`/`consolidate`/`audit`/`code_map`/`profile` | `App.svelte`、`src/lib/*`、`src/components/*`、`commands.rs` |

**依赖方向（单一、编译期强制）**：显示 → 记忆 → 文件。

---

## 3 · 四条不变量

| # | 不变量 | 含义 | 破坏它的后果 |
|---|---|---|---|
| **I1** | 对话是**唯一原始输入**，append-only | 不可再生、只追加、不裁剪 | 溯源链断裂 |
| **I2** | 记忆入口**唯一**（AI 经 MCP） | 没有第二条写节点的路 | 出现两个事实源 |
| **I3** | 写路径**唯一**（守门 + 留痕） | 结构违规阻断、规范违规标注 | 并发覆盖、无审计 |
| **I4** | 显示层只吃 `MemoryView`、写只走 `Intent` | 不直读派生文件、不直接写文件 | 改一层震到另一层（分层失效） |

**验证责任**：治理权全在 AI（人治通道已放弃），因此**验证权在痕迹**——每个决策（含"不保留"）都必须留下记录。这是 I2/I3 唯一的质量保证机制。

---

## 4 · 三层接口

### 4.1 接口 ① 文件层 → 记忆层（原样读取，不认识语义）

```
read_dialogue_source(path) -> bytes        # 原样读；不解析、不校验
append_dialogue_source(path, line)         # append-only 追加一行；永不截断
scan_nodes(root) -> FileView               # 纯事实
atomic_write(path, bytes)                  # 唯一落盘原语
```

`FileView` = 现有 `ChainSnapshot` **减脂**：`nodes / archived / edges / manifest / validation`。
**字段表里不允许出现** `strength / last_touch / stale / derived / indexed`（那是记忆层语义）。

### 4.2 接口 ② 记忆层 → 显示层：`MemoryView`（唯一读接口）

```
MemoryView {
  base: FileView                                  # 原样透传，显示层不需第二次请求
  annotations: { [node_id]: {
      strength | null, reads, writes, last_touch_ago,
      derived, archived, frozen,
      index: { indexed, stale } | null,
      origin: "dialogue/log.jsonl#<seq>" | "legacy" | "bootstrap"
  }}
  caps: { vector, keyword, consolidate, degraded, degrade_reason }   # 降级契约
  clock: { memory, wall | null, guide_version }
}
```

**收益**：记忆层内部换嵌入模型 / 换索引格式 / 增删字段 → 字段集不变 → **显示层零改动**。

### 4.3 接口 ③ 显示层 / AI → 记忆层：`Intent`（唯一写路径）

```
Intent = Remember{...} | Consolidate{...} | ResolveConflict{...}
```
**验收**：显示层永不出现 `fs::write` / `atomic_write` / 直接改 frontmatter。

---

## 5 · 对话文件（原始输入）

### 5.1 形态

```
<workspace>/.chain/dialogue/log.jsonl      ← 一个工作区一份持续文件
```

**一工作区一文件**：整个项目的对话是一条连续叙事，而不是按会话切碎。
**append-only**：只追加完整行（单行写入），既有字节永不改写。
**不裁剪**：记忆系统成熟前全部保留。将来的裁剪语义定为「**归档整个文件 + 开新文件**」——绝不删中间段，append-only 因此永远成立。

### 5.2 格式（单一 JSONL）

```jsonl
{"k":"head","v":1,"session":"s-2026-10-03-a","model":"<client 自报>","guide":"analysis v14","started":"..."}
{"k":"msg","seq":1,"session":"s-2026-10-03-a","ts":"...","role":"user","text":"..."}
{"k":"msg","seq":2,"session":"s-2026-10-03-a","ts":"...","role":"assistant","text":"..."}
{"k":"tool","seq":3,"session":"s-2026-10-03-a","ts":"...","name":"create_node","args":"title=方案 · FFT 统计波谱法","result":"ok"}
{"k":"decision","seq":4,"session":"s-2026-10-03-a","ts":"...","decided":"keep","covers":[1,2],"nodes":["t-041"],"reason":"形成完整方案链，提炼为节点"}
{"k":"decision","seq":5,"session":"s-2026-10-03-a","ts":"...","decided":"skip","covers":[3,3],"nodes":[],"reason":"纯工具回执，无新知识"}
```

| 记录 | 何时写 | 关键字段 |
|---|---|---|
| `head` | 每个新会话开始时一次（单文件里可出现多次） | `session` / `model` / `guide` / `started` |
| `msg` | 每条用户/AI 消息 | `seq` / `session` / `role` / `text`（+ `part`/`parts`） |
| `tool` | 每次 MCP 工具调用（**结果摘要**，非全量转储） | `name` / `args` / `result` |
| `decision` | 每次记忆决策 | `decided` / `covers` / `nodes` / `reason` |

### 5.3 三条设计要点

1. **`seq` 工作区级单调、全记录唯一**——跨会话连续，是幂等与排序锚点。
2. **`decision.covers` = 覆盖的"消息 seq"区间**——同时充当两件事：
   * **消费锚点**："这段对话处理到哪了"
   * **审计证据**："我怎么处置的"
   新 AI 接管时只需读**最后一次 `covers` 右端点之后**的记录，不必重读整部历史。
3. **长消息拆行**：超 4096 字节按 UTF-8 字符边界切分，同 `seq` + `part`/`parts` 标记；
   一条逻辑消息对应多条物理行，**拼回必须逐字一致**（有单测保证）。

### 5.4 为什么"不保留"也必须留一行

`decided:"skip"` + 非空 `reason`。**这是"AI 自主"与"AI 遗漏"的唯一区分证据**——没有它，自主就退化成无审计，交接保真度直接失效。因此空 `reason` 在 API 层被拒绝。

### 5.5 读保守 / 写严格

| 侧 | 规则 |
|---|---|
| 读 | 未知键忽略（前向兼容）；坏行**隔离并报出行号**，其余记录照常返回（对话不可再生，宁可报出来给人修） |
| 写 | 写入前校验（单行、非空、`covers` 合法、`reason` 非空）；宁拒绝，不污染 |

---

## 6 · 对话 vs 节点

| | 对话文件 | 节点 |
|---|---|---|
| 本质 | 原始输入、不可再生 | 加工产物、可修订 |
| 内容 | 过程、试错、被否方案、原始知识 | 结论、决策、可用知识 |
| 结构 | 平铺日志 | 图谱（parent + rel + 状态） |
| 检索 | 关键词子串 | 语义召回 + 图扩散 + 强度排序 |
| 图谱性 | 不进图、不计度数 | 进图、进 `entry_hubs` |
| 心智 | **"为什么不这么做"** | **"是什么 / 怎么做"** |

**为什么不能合并**：若对话直接变节点，图谱退化为聊天记录——而图谱的价值正是"快速接管"（`entry_hubs` / `active_chain` / 状态健康度）。现有 303 节点已近可读上限，对话体量是其百倍。

**交接保真度判据（验收标准）**：
> 给新 AI 只开放 MCP，应能在 N 次工具调用内回答：根目标是什么、走到哪一步、失败过什么、为什么这么走。
> **节点给"是什么"，对话给"为什么"。**

---

## 7 · 检索设计（含一条重要修正）

### 7.1 现状澄清

| | `search` | `recall` |
|---|---|---|
| 机制 | **关键词子串匹配**（title/tags/body/代码骨架） | **语义召回**（向量余弦 + 强度加成 + 两档阈值 + derived 降权） |
| 本质 | 检索的 L4/L5 档，**也是 recall 的降级路径** | 记忆层的回忆入口（M-3） |
| 得分 | 固定 `1.0` | 真实相似度 + 排序 |

`search` **不是回忆**；回忆的入口本来就是节点（`recall` 向量 → 冷启动 → 关键词降级）。

### 7.2 修正：对话检索必须**显式**，不得作为降级结果

```
search(query, scope: "nodes" | "dialogue" | "both" = "nodes")
recall(query, k, include_archived, scope: "nodes" | "dialogue" | "both" = "nodes")
```

**理由**（三条，全部是正确性问题而非风格问题）：

1. **语义不同**：「降级路径」（节点找不到 → 往下滑）与「目标切换」（我这次就是要查过程）是两件事。前者应显式声明 `degraded:true`，后者应是 AI 的主动选择。
2. **污染缺口信号**：`recall` 未命中会写 `stats.gaps`（线索缺口清单），这是"节点层缺线索"的信号，未来可喂给 `consolidate` 建节点。对话混入召回会污染这个信号。
3. **性能**：对话体量是节点的几十倍；默认扫它会击穿现有性能预算（全库正文检索 < 400ms）。

**结论**：节点优先是**结构性的**（默认值即节点），对话是"往下挖一层"的显式动作。

### 7.3 对话命中的标注

对话命中统一标 `matched_on: "dialogue"`，并附 `session` 与 `seq`，便于 AI 判断"这是过程还是结论"。

---

## 8 · 规则强制（放弃候选阶段后的替代设计）

原候选阶段承担两件事，人审删除后必须重新分配规则校验：

| 违规类型 | 例子 | 处置 | 理由 |
|---|---|---|---|
| **结构性** | 建环、悬空 parent、重复 id、非法 rel、路径穿越 | **阻断** | 破坏图谱可用性，AI 自己也无法使用 |
| **规矩性** | 缺 `> 触发：` 句、title 不合命名规范、正文不自包含 | **不阻断**：audit 留痕 + frontmatter 标记（如 `conventions: [missing_trigger]`）+ 返回提示 | 保留 AI 自主；违规可事后扫出 |

---

## 9 · 治理权与冻结

| 机制 | 现状 | 重构后 |
|---|---|---|
| 冲突冻结 `[待裁决]` | 冻结 + **人工裁决** | 冻结 + **AI 自愈**（`resolve_conflict`）✅ |
| GUI 人用通道 | `create_node_human` / `delete_node_human` / `set_parent_human` | **已删除**（ADR 0015）✅ |
| 文件树模式编辑面 | ＋ / ✎ / 🗑 三态 | **已降为只读阅读面**（阅读/检索/定位保留）✅ |
| ADR 0003 | 治理权在人 | **已改写**（ADR 0015）：治理权在 AI，验证权在痕迹 |

**冻结机制保留的理由**：DSH 实测同一工作区可能同时有 4 个 MCP 进程（并发写冲突必然出现）——冻结从"治理落点"变为"**并发保护信号**"。

---

## 10 · 工具契约演进（v5 → v6）

| 工具 | 处置 | 说明 |
|---|---|---|
| `get_guide` | 保留 | 指南内嵌 + 首读强制 |
| `get_overview` | 保留 | + `guide_version` |
| `search` / `read_node` / `expand` / `read_path` / `recall` | 保留 | 读通道；`search`/`recall` 加 `scope`（默认 `nodes`） |
| **`remember`** | **新增（核心）** | 追加对话 + 判断提炼节点 + 决策留痕；**取代 create/update/link** |
| `consolidate` | 保留 | 维护（蒸馏） |
| **`resolve_conflict`** | **已实现（契约 v8）** | 冻结自愈（治理权转移后的裁决出口） |
| **`dialogue_status`** | 新增（只读） | 消费进度 / 未消费起点 / 指南版本漂移 / 坏行 |
| `create_node` / `update_node` / `link_nodes` / `unlink_nodes` / `archive_node` | **已移除（契约 v7）** | "节点入口"时代的工具，与 I2 冲突；能力并入 `remember` 的 commits 意图 |

**连带成本**：契约 v6（破坏性）→ golden 18 条重固化、指南 v14/v8 → v15/v9、CHANGELOG + 版本矩阵递增（宪法第 8 条）。
**不可丢的能力**：`update_node` 的乐观锁（`expected_updated`）必须并入 `remember`，否则并发保护消失。

### `dialogue_status` 返回形状（草案）

```
{
  records, sessions[], guide_versions[],
  last_covered_to, unconsumed_from, unconsumed_count,
  decisions: { keep, skip, revise },
  malformed: [{ line, reason }]
}
```

---

## 11 · 存量数据与 bootstrap

| 类别 | 处理 |
|---|---|
| **存量节点**（12 个工作区 303 个，含批量补建的 `v-*` 验证节点） | `origin: legacy`，**祖父条款**：只读不改，可被引用 / 挂子节点 / 归档，**不得作为新记忆的扩写基础**（防历史被继续放大） |
| `AI_GUIDE.md` 工作区副本 | 保留（内嵌 + 过期即刷已实现）；不参与"对话入口"规则 |
| demo/dev、perf1500 等示例区 | 祖父条款 |

> **"存量节点"的定义**：它不是架构概念，是**迁移期概念**——指磁盘上已存在、但**没有对话来源**的节点。

**bootstrap 通道**：空工作区初始化时可一次性导入（标记 `origin: bootstrap`），此后仅允许对话入口。

---

## 12 · 阶段与验收

| 阶段 | 动作 | 验收判据 |
|---|---|---|
| **P0 对话落地** | 文件层对话读写；记忆层账本（格式/覆盖区间/增量消费）；`dialogue_status` 只读；工具返回带 `guide_version` | 现有测试全绿；契约只加不删（v6） |
| **P1 入口唯一** | `remember` 上线；删除 5 个节点工具；指南 v15/v9 写入协议；GUI 写通道改道 | 任何新节点都带 `origin: dialogue/log.jsonl#<seq>`；**跳过决策也有留痕** |
| **P2 人治下线** | 删 3 个人用命令；文件树写工具降只读；`resolve_conflict`；ADR 改写 | GUI 只读仍能完成看/搜/读/定位全部动作 |
| **P3 交接验收** | 祖父条款落地；对话/节点分工入指南；新 AI 接管实测 | ✅ 已落地：legacy 隐式标记（`walker::mark_legacy`，不回写文件）+ 对话阅读面（CDP 实跑验证）。"新 AI 仅用 MCP 在 N 次调用内还原项目"的接管实测留给真实 AI 会话（工具链已齐备：`dialogue_status` / `read_node(origin)` / `get_guide`） |
| **P4 分层强制** | 拆 `engram-file` / `engram-memory` / `engram-api` 三 crate | ✅ **文件层已拆为独立 crate `engram-file`**（model/scanner/fsio/schema/migrate/watch/workspace/evidence/profile/guide/audit/dialogue/chain_ops/node_edit，94 测试），编译期不依赖记忆层；记忆层与 API 层保留在 `engram-core`（API 边界 = `ops` 模块，JSON 适配集中于此）——把 api 提升为独立 crate 是纯机械化搬迁，留作后续步骤（设计稿 §15 接口已为其定型） |

### 贯穿性 DoD（"记忆系统不完善也能用"）

> 删光 `.chain/index/`、`stats.json`、`audit.jsonl`、`code_map/` 后，软件仍能：打开工作区、看图谱、看文件树、读任何节点、搜索、显示归档/蒸馏徽标（从 frontmatter 读）。
> **只有向量召回与强度显示降级，并显式声明。**

### 分层独立性的机械检查

1. **跨层 import 检测**：用 crate 依赖图强制（不是 lint）；
2. **降级冒烟**：删光派生物跑 GUI 最小路径；
3. **接口锁**：`MemoryView` / `Intent` 字段集纳入 golden 契约，新增字段只能加性 + CHANGELOG + 版本矩阵递增。

---

## 13 · 风险与缓解

| 风险 | 表现 | 缓解 |
|---|---|---|
| **单文件无限增长** | 长期使用后 `log.jsonl` 到几十 MB；现有 `search` 是整文件读盘 | 显示层默认只渲染尾部 N 条；`search` 对对话走行级流式扫描；为对话文件单定性能基线 |
| **渲染大文本** | 单条 AI 长回复上万字，渲染卡顿 | 单条默认折叠 + 展开；复用现有按需渲染（选中才渲染） |
| **并发交错** | 多会话写同一文件，叙事被打断 | `session` + `ts` 分组视图；长消息拆行防撕裂；`seq` 由文件末尾最大值 + 串行写锁决定 |
| **"一文件"与"可裁剪"张力** | 从大文件中间删段会破坏 append-only | 裁剪语义定为"归档整个文件 + 开新文件"（格式现在即按此设计） |
| **一次性大爆炸重构** | 220 测试 + 16 个 CDP 脚本 + golden 同时红 | 分五阶段，每阶段独立验收（§12） |

---

## 14 · 已定 vs 待定

### 已定稿（本轮锁定的决定）

| 项 | 结论 |
|---|---|
| 层与层 | 文件层（纯字节）/ 记忆层（规则+记忆动力学）/ 显示层（渲染+交互），依赖单向 |
| 记忆入口 | **唯一：对话**；AI 唯一通道：**MCP** |
| 治理权 | 全在 AI；**验证权在痕迹** |
| 对话存储 | `.chain/dialogue/log.jsonl`，**一工作区一份**，append-only，**暂不裁剪** |
| 对话格式 | JSONL + `session` + 工作区级 `seq` + `covers` 区间 + `part` 拆行 |
| 渲染 | 渲染期投影为 Markdown，**不落盘第二格式**；显示层新增"对话阅读面" |
| 检索 | 节点优先是默认；对话需显式 `scope` |
| 存量节点 | `origin: legacy` 祖父条款 |
| 指南 | 内嵌 + 首读强制 + 每工具返回带 `guide_version`（**不强制重读**） |
| 记忆层两个已知缺陷（强度 clamp 归零、墙钟恒空） | **不在本次范围**，重构完成后单独立项 |

### 待定（不阻塞 P0）

| # | 事项 | 建议 |
|---|---|---|
| 1 | 拆行阈值 | 4096 字节（已实现，可调） |
| 2 | 对话是否进 `search` 默认范围 | **否**（默认 `nodes`，需显式 `scope`） |
| 3 | `dialogue_status` 的字段集 | 见 §10 草案 |
| 4 | schema 版本 | 账本格式落地记为 **1.2**（minor，B 类迁移） |
| 5 | 对话阅读面的入口位置 | 与"文件树模式 / 图谱模式"并列的第三个显示模式，或文件树内的一级节点 |

---

## 15 · 代码就绪状态（P0 部分）

| 模块 | 层 | 职责 | 测试 |
|---|---|---|---|
| `crates/engram-core/src/dialogue.rs` | 文件层 | 原样读写、append-only、路径与命名校验（**不认识格式**） | 5 条 |
| `crates/engram-core/src/dialogue_log.rs` | 记忆层 | JSONL 格式唯一定义、解析、覆盖区间、增量消费、溯源串 | 12 条 |
| `crates/engram-core/src/ops/remember.rs` | 记忆层 | **记忆唯一入口**：账本事件追加 + commits 节点意图 + 决策留痕 + origin 溯源 | 8 条 |
| `ops::dialogue_status`（mod.rs） | 记忆层 | 账本只读状态（消费进度/决策计数/坏行） | —（经 MCP golden 覆盖） |
| `engram-mcp` | 接口适配 | `remember` / `dialogue_status` 注册；`to_result` 统一注入 `guide_version` | golden 23 条 |

**分层验证**：文件层不含任何 `k`/`seq`/`role` 概念；换格式只改 `dialogue_log.rs`。
**验证结果**：`cargo test --workspace` → **core 228 / cli 13 / golden 23 条全绿**（重构前 core 203）。

---

**结论**：本次重构的实质不是重新分目录，而是**四条不变量的落地**（§3）。放弃人治通道后，验证责任从"人事前审核"转移到"痕迹事后可查"——这正是 `decision skip` 也必须留一行的原因：它是"AI 自治"这套设计唯一的质量保证机制。
