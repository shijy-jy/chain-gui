<!-- CHAIN_GUIDE_DEV_VERSION: 15 -->
# 开发模式 AI 使用指南（知识库搭建）

> 本指南适用于 `.chain/.mode = dev` 的开发模式工作区：自由知识图谱、个人知识库搭建。
> 分析模式工作区请使用《AI 工程实践哲学指南 + Chain Protocol》——两套指南按工作区模式标签区分，不可混用。

## 0. 这个模式是什么

开发模式 = 开发者/用户**手工搭建知识库**的图谱模式，AI 只做辅助整理。与链协议分析模式的区别：

- **无 AI 协议要求**：字段、结构全部自由，不要求目标/设计/任务/验证层级
- 数据仍是纯文本 `.chain/nodes/*.md`——可 git 管理、可迁移、任何编辑器可改
- 模式由初始化时写入的 `.chain/.mode` 标签确定，随工程走、不可混用

## 1. 目录结构与模式标签

```
工程根目录/
└── .chain/
    ├── .mode          ← 模式标签（本指南适用：内容为 dev）
    ├── nodes/         ← 知识节点，一个 .md 文件 = 一个节点
    ├── artifacts/     ← 证据产物（可选）：截图/文档/数据，按节点分层
    ├── PROCESS_LOG.md ← 过程日志（可选）
    └── AI_GUIDE.md    ← 本指南
```

> 建议用 git 管理整个工程目录（软件不强制）：每次整理提交一次，改错可回滚。

## 2. 节点文件格式

**最简形态：任意 .md 文件即节点**（没有 frontmatter 也行）——id 取文件名、标题取正文第一个 `# 标题`。

**完整形态（可选 frontmatter）**：

```markdown
---
id: 贝叶斯定理          ← 可省略；省略时取文件名（含中文）
type: note              ← 可省略；知识库默认中性类型 note
title: 贝叶斯定理       ← 图谱显示名（建议写主题名）
parent: 概率论          ← 链接：父节点 id；可省略（独立节点）
status: none            ← 可省略；知识库节点一般无状态
tags: [数学, 概率]      ← 可省略
evidence: [artifacts/贝叶斯定理/推导笔记.pdf]   ← 可省略
---

正文：自包含地讲清这个知识点。公式用 LaTeX（行内 $...$、独立行 $$...$$）。
```

字段全部可有可无，缺失自动兜底。唯一建议：**title 写主题名**（图谱靠它认节点）。

## 3. 知识库搭建设计（本指南的核心）

### 3.1 一个节点 = 一个知识点

- 一个节点讲清一件事：概念、方法、结论、资源、踩坑记录均可
- 正文**自包含**：脱离上下文也能读懂；公式用 LaTeX 渲染
- 内容过细 → 拆子节点；内容重复 → 合并

### 3.2 链接 = 关系

- `parent` 链接表达"归属/从属"：上层主题 → 下层细节
- **自由拓扑**：允许多个根（多个主题中心）、孤立节点（素材卡片）、环
- 结构首选**主题中心（hub）式**：每个大主题一个根节点，细节挂在它下面

### 3.3 常见组织模式（按需选用）

| 模式 | 用法 |
|---|---|
| 主题树 | 领域 → 主题 → 知识点，逐层细化 |
| 索引页 | 一个节点做目录（正文列链接），子节点是条目 |
| 卡片库 | 孤立节点 + tags 分类，不强做链接 |
| 时间线 | 按日期建节点，串成学习/进展记录 |

### 3.4 命名与可追溯

- title 在库内**唯一可辨**，避免多个节点同名（图谱按标题认节点）
- id 稳定不轻易改（其它节点靠它链接）
- 证据文件放 `artifacts/<节点id>/`，文件名前缀节点 id；软件里点证据文件名可直接打开

### 3.5 递进关系建模（v2：问题 → 方案 → 局限 → 新方案的迭代链）

当知识是"**解决一个大问题 → 方法有局限 → 需要新方法 → 又有新局限**"的缺陷驱动递进时，用**交替链 + 关系类型**表达：

```
问题：实时渲染大场景
 └─ 方案 · 路径追踪            （rel: contains，实线）
     └─ 局限 · 路径追踪方差大   （rel: contains，实线；由上一方案引出）
         └─ 方案 · ReSTIR 时空复用（rel: solves，虚线 = 解决父的局限，递进主线）
             └─ 局限 · 可见性复用失效（rel: contains，实线）
                 └─ 方案 · GRIS     （rel: solves，虚线）
 └─ 方案 · 光子映射（备选）      （rel: alternative，点线 = 备选方案）
```

- **`rel` 字段**（写在子节点 frontmatter，可选）：`contains`（默认，父包含子，实线）/ `solves`（子解决父的局限，虚线——递进主链）/ `alternative`（子是父的备选，点线）
- **角色靠 title 前缀 + tags**：`方案 ·`、`局限 ·`、`问题：` 一眼可辨
- **正文模板自包含**：每个"方案"写清"解决了什么 + 没解决什么（局限）"；每个"局限"写清"由哪个方案引出 + 为什么必须解决"
- 同一局限引出多个候选方案 → 挂多个 `solves` 子节点，用 tags 标"采用/备选/弃用"

### 3.6 内容质量建议（不强制，但推荐）

- 引用外部资料时记书目（标题/作者/年份/链接），关键要点写进正文
- 定期回看：孤立节点补链接、长正文拆节点

## 4. AI 在开发模式工作区的行为守则

1. **先读 `.chain/.mode` 确认模式**；dev 工作区绝不套分析模式的链协议去要求用户补字段/改结构
2. **不破坏数据**：删除/覆盖前确认 + 备份；建议先 git 提交留痕
3. **协助整理**：建节点写清主题名、链接表达真实关系、正文自包含；不编造知识来源
4. **不拍脑袋**：搭建知识结构前参考优秀实践（Obsidian 卡片盒/Zettelkasten、双链笔记、维基式分类等），检索手段不限
5. **界面即所见**：改完文件软件自动刷新，无需通知动作

## 5. 与分析模式的关系

- 两套指南按 `.chain/.mode` 区分：`analysis` = 链协议（目标/设计/任务/验证 + 九条守则）；`dev` = 本指南（自由知识库）
- 同一工作区模式不可混用（标签绑定）；AI 每次操作前先确认标签

## 6. 知识库维护与触发线索（v3）

- **trigger 句**：节点正文开头写「> 触发：<同义话术>（；分隔）」，写未来会用什么话术想起它（如「> 触发：费曼技巧；以教促学；输出倒逼输入」）。
- **tags**：填检索同义词（≤5 个，每个 ≤20 字）。
- **recall 工具**：语义召回；索引未建立或模型缺失时自动退化关键词检索并在返回中显式声明（degraded 字段）。
- **导航上下文（v15）**：`search` / `recall` / `read_node` / `expand` 的每条结果自带 `parent / depth / children_count / degree / origin / updated`——顺着节点链往上往下梳理时，**一次检索就能判断下一步该读哪个节点**，不必逐条再读一次；`degree`（无向关联边数）同时是**人看图时球体大小的依据**（人机同源，别再自己估算中心性）；`get_overview` 还带 `structure` 块（roots / **root_ids**（parent=null 的入口，按子树规模倒序，≤20）/ max_depth / leaves / depth_hist + 各入口 hub 的 subtree_size）；`dialogue_status` 带 `gaps`（用户找过但没找到的查询 → 该补节点了）、`open_loops`（已 success 但无验证子节点且无「自验收」的 task）与 `coverage`（`total / read / unread / unread_ids[]`）。
- **两条读取策略（软件不预设导读路径，用上面字段自己规划）**：
  - **层级优先（还原项目时）**：从 `structure.root_ids` 起 `expand(id, direction:"children")` 逐级向下读——信息量逐级指数增加，**先看骨架再决定读哪条原文**；`expand` 每个节点带 `hop`（距中心跳数）+ `first_line`（正文首行机械截断，不是摘要）。
  - **跨链随机（开发自己项目时）**：用 `coverage.unread_ids` 抽未读节点，**每次随机不重复**地读，读到即覆盖；`direction:"parents"` 反查上下文。
- **`expand.direction`**：`children`（向下） / `parents`（向上） / `both`（默认）。**读节点正文就是放弃读原文**——`first_line` 只做机械截断，**不得据此二次概括节点内容**（那会丢失信息）。
- **归档**：长期不用的节点用 `remember` 的 commits 意图 `op:"archive"` 归档（保留全文，检索默认不显示，可 include_archived 找回）；断边修正用 `op:"unlink"`（见 §8，旧独立工具已移除）。
- **蒸馏**：知识库长到维护吃力时用 consolidate（默认 dry_run，先看计划再执行）；产物标记 derived，人审后摘帽。

## 7. 代码工程：骨架挂理论节点（M-Code，v8）

**对有代码的工程**：代码骨架**挂在理论/概念节点上**（该节点的信息栏「代码」栏）——**不要为代码模块另建一群骨架节点**，图谱保持概念纯净，代码是概念的可执行证据。

- **语言矩阵（自动检测，挂载免手选）**：rust / csharp / cpp（含 hlsl·glsl·cuda 同解析器）；`.cs` 与 shader 族并存目录自动判 **unity**（Unity 工程专用双解析器）
- **挂载**：在概念节点信息栏「代码」点「挂载源码文件…」选择源码（或手工在 frontmatter 写 `code_map: <源码相对路径>`，文件或目录均可；**v6 起允许绝对路径**——跨盘挂载如 G 盘知识库 ← D 盘 Unity 工程）；**正文只放一句概述**，公开接口与调用关系由骨架派生文件承载
- **生成/刷新**：信息栏「刷新骨架」或 `engram-cli sync-code-map --workspace <工程根> [--lang auto|rust|csharp|cpp|hlsl|glsl|cuda|unity]` → 公开接口 + 签名 + 文件:行:列 + 调用边 + Mermaid 图，落 `.chain/code_map/<id>.md`
- **骨架浏览**：信息栏「代码」栏可滚动阅读；内容多时点 **⧉ 展开全屏页**（覆盖整个窗口的大字体全量阅读，Esc/✕ 关闭）；**MCP 侧**：`read_node(include_code_map=true)` 取 `code_map_md`（骨架全文，未挂载为 null）
- **检索语义**：模块名/函数名/签名进入 recall 与关键词检索；骨架即该概念的可执行证据
- **stale 兜底**：源码变更后骨架标 `stale: true`——AI 进场发现 stale，**先刷新再基于最新骨架工作**（安静优先），绝不基于过期骨架做判断
- 骨架是**派生物**（可重建、可删除——重跑即恢复），不进事实源；改源码不改骨架不是知识变更，同步一下即可

## 8. 对话账本与记忆入口（三层重构 v9 · 唯一入口）

**本节的规则优先于前文一切写工具说明**：自 v10 起，节点写入的唯一入口是 `remember`——旧工具 create_node / update_node / link_nodes / archive_node / unlink_nodes **已从 MCP 移除**（契约 v7），不再可用；凡前文（§6 等）提到这些工具之处，一律改经 `remember` 的 `commits` 意图执行。

### 8.1 心智模型

对话是"别人给你讲解的内容"，节点是"你听完后整理好的脉络"。因此：

- **工作过程先进对话账本**（`.chain/dialogue/log.jsonl`，append-only、不裁剪）：每次会话用 `remember`（kind=msg / kind=tool）追加你的消息与工具轨迹；
- **只有当值得长期记住时**，才在同一或后续 remember 调用里用 `commits` 把整理好的脉络落成节点——**不是每句话都建节点**，否则图谱退化成聊天记录；
- 不保留也必须留痕：`remember`（kind=decision, decided=skip, reason=…）——**reason 必填**。否则"有意跳过"与"忘了记"无法区分。

### 8.2 remember 用法

```
remember { session, kind:"msg", role:"user"|"assistant", text }        # 追加一条消息
remember { session, kind:"tool", name, args, result }                   # 追加工具轨迹
remember { session, kind:"decision", decided:"keep"|"skip"|"revise",
           covers:[from,to], nodes:[...], reason }                      # 决策留痕（消费锚点）
  + commits: [ { op:"create", title, body, tags, force? } |             # 落节点意图（复用守门）
               { op:"update", id, mode, content, expected_updated? } |
               { op:"link", from, to, rel, desc? } |
               { op:"unlink", from, to } |
               { op:"archive", id, reason? } ]
```

- `session` = 会话 id（仅字母/数字/-/_/.）；工作区持续账本里靠它区分谁在说。
- `covers` 指向被本决策消费的**消息 seq 区间**（含端点）：既是"处理到哪"的锚点，也是审计证据。新会话接管时先 `dialogue_status` 拿到 `unconsumed_from`，只读未消费段。
- 有 `commits` 时必须有事件（否则节点没有溯源锚点，会被拒绝：REMEMBER_NO_EVENT）。
- 新建节点的 `origin` 自动写为 `dialogue/log.jsonl#<seq>` ——「这条记忆从哪来」由此可查。

### 8.3 规则强制（两档）

- **结构违规 → 阻断**：非法 rel、自环、悬空端点、非法会话名、空 reason……（REMEMBER_* 错误码）。
- **规矩违规 → 只标记不阻断**：正文缺「> 触发：」句等。节点 frontmatter 会写 `conventions: [missing_trigger]`，响应里提示。你有完全自主，但**违规可被事后扫出**——留痕即责任。

### 8.4 并发冲突与冻结自愈（v11）

并发双写（update 意图的 `expected_updated` 与实际不符）→ 节点进入**冻结态**：标题加 `[待裁决]` 前缀、`status: blocked`、`frozen: true`、写 `freeze_reason`，**冲突内容绝不落盘**。冻结期间其它写路径一律拒绝——唯一出口是：

```
resolve_conflict { id, title, status, body?, expected_updated? }
```

- 裁决前先 `read_node` 核对双方内容，一次写回最终裁决（title/status/body），去除冻结标记；
- 非冻结节点调用报 NOT_FROZEN；expected_updated 不符报 CONFLICT（不改变冻结态）；
- 裁决留痕（audit `unfreeze`）。治理权在 AI，**验证权在痕迹**——每次裁决都必须可事后复盘。

### 8.5 存量节点（legacy 祖父条款，v12）

- 无 `origin` 字段的节点是**重构前的存量记忆**：软件在扫描时隐式标记 `origin: legacy`（不回写文件）。
- **祖父条款**：legacy 节点只读不改——可以被引用 / 挂子节点 / 归档，但**不得作为新记忆的扩写基础**（不要基于它继续堆知识）；新知识一律经对话产生新节点。
- 判断方法：`read_node` 返回的 `origin` 字段（`legacy` / `dialogue/log.jsonl#<seq>`）。

### 8.6 接管与检索

- `dialogue_status`：账本规模 / 会话与指南版本 / 消费进度 / 决策计数 / 坏行清单（malformed 需修复；对话不可再生，修复前先备份），另带 `gaps` / `open_loops` / `coverage`（读过与未读的节点，用于规划「层级优先」或「跨链随机不重复」的读取路线）。
- **节点优先是默认**：`search` / `recall` 默认只搜节点；要翻对话必须显式 `scope:"dialogue"`（过程性内容不进语义召回，否则污染线索缺口信号）。
- 每个工具响应携带 `guide_version`：版本一变你当次就能发现，自主决定是否重读本指南（不强制）。
