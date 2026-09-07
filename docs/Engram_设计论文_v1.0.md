# Engram：面向人机共治的工程记忆图网络

## ——问题分解、递进设计与演进路径（技术论文稿）

> 版本 v1.0 · 2026-09-07 · 基线 Engram v2.7.0 · 性质：设计整理，不执行实现

---

## 摘要

工程实践中，AI 协作长期受制于三重记忆断裂：模型无持久记忆、上下文窗口有限、记忆绑定于单一客户端而无法跨会话与跨工具共享。本文提出并论证 **Engram**——一个本地优先、纯文本、人机共治的外部记忆图网络。其核心设计命题有三：(1) **文件即唯一事实源**：节点 `.md` 文件只存干净的结构化事实，邻域、路径等关系叙述由服务在读取时动态拼装，关系变更零同步成本；(2) **工具即交互翻译**：AI 没有眼睛，一切机器接口都是图形界面交互（搜索、涟漪、聚焦、全局视图）在文字域的等价物；(3) **遗忘是特性**：记忆权重 = 突触强度 × 平行通道数，访问次数与图谱度数分别对应两个因子，长期未触达的图元降权归档而非删除。本文从真实工程实践出发，将总问题逐层分解为接入面、检索、遗忘、生命周期与图注意力五个子问题，给出递进式解决方案、九条设计不变式、可测验收标准与已获得的实证数据（31 项端到端全绿、109 项单元测试、布局间距硬保证实测）。Engram 的生态位不在通用记忆 API 的红海，而在开发者与研究者的人机共治工程记忆：透明、可携带、有生命周期。

**关键词**：外部记忆 · 工程记忆图谱 · MCP · 检索增强 · 图注意力 · 记忆生命周期 · 本地优先

**Abstract (EN)**: Engineering collaboration with AI is constrained by three memory fractures: no persistent model memory, bounded context windows, and memory bound to a single client. This paper presents Engram — a local-first, plain-text, human–AI co-governed external memory graph network. Three design theses ground the system: (1) *files are the single source of truth* — nodes store clean facts only, while neighborhood/path narratives are composed at read time; (2) *tools are interaction translations* — an AI has no eyes, so every machine interface mirrors a GUI interaction; (3) *forgetting is a feature* — memory strength equals synaptic strength times parallel pathways, with access counts and graph degree as the two factors, so stale elements decay into archives rather than being deleted. The total problem is decomposed into five sub-problems (access surface, retrieval, forgetting, lifecycle, graph attention), addressed by progressive designs, nine invariants, and empirical acceptance data (31/31 end-to-end checks, 109 unit tests).

---

## 1 · 引言

### 1.1 背景：工程记忆的三重断裂

人与 AI 协作推进一个项目时，记忆以三种断裂的方式存在：

1. **模型无持久记忆**：每次对话从零开始，上一轮建立的上下文（设计决策、失败原因、验收结论）随会话结束而蒸发；
2. **窗口有界**：上下文窗口无法容纳长周期工程的完整脉络，压缩即丢失；
3. **记忆绑定客户端**：Claude 记忆归 Claude，Cursor 归 Cursor——同一个工程在不同的 AI 眼里是失忆的。

因此产生一个朴素而根本的需求：**让工程记忆独立于模型、独立于客户端，成为双方共享、可长期累积的资产。**

### 1.2 问题定义

**Q0（总问题）**：如何构建一个工程记忆系统，满足——

- **可接入**（任意 AI 客户端可通过标准协议读写）；
- **可理解**（AI 读取时获得关系叙述而非孤立文件）；
- **可维护**（长期使用不膨胀、不腐化，且有"遗忘"能力）；
- **透明可携带**（纯文本、人可读、文件夹即全部资产、本地优先）。

### 1.3 贡献与结构

本文贡献：(1) 将 Q0 分解为五个子问题并给出问题-方案-验收的完整链条；(2) 以 v1.0–v2.7 的真实工程记录（布局冻结、涟漪物理、并发写、协议接入等）为证据，归纳九条设计不变式；(3) 给出检索三层与遗忘四组件的递进设计，并以认知心理学与神经科学依据校准；(4) 提出记忆图网络（QKV 图元 + 图注意力）作为终态架构。全文结构：第 2 节相关工作与生态位；第 3 节问题分解；第 4 节递进式方案；第 5 节设计不变式；第 6 节评估；第 7 节讨论；第 8 节结论。

---

## 2 · 相关工作与生态位

### 2.1 AI 记忆系统

- **MemGPT / Letta**[7]：把 LLM 视作操作系统，用分页式"主存/外存"管理上下文；缺人可读的持久载体与可视化。
- **Mem0 / MemGPT 类记忆平台**[8]：托管式记忆 API，提取偏好与事实并向量化；云端、黑盒、无工程结构语义。

### 2.2 图增强检索

- **Microsoft GraphRAG**[5]：先建实体关系图、再做社区摘要的全局问答；证明**图结构本身是强语义索引**——Engram 的检索设计据此坚持"先结构后向量"。
- 知识图谱 RAG 的一般结论：关系叙述（路径、邻域）比孤立片段更适合回答"为什么/怎么演变"类问题。

### 2.3 神经记忆模型

- **Titans（Learning to Memorize at Test Time）**[6]：以"惊讶度"驱动的遗忘与巩固，验证了**记忆系统的核心不是存储而是注意力分配**——与 Engram 的"遗忘即检索"闭环同构。

### 2.4 图谱布局与可视化

- Fruchterman–Reingold 力导向[13]与 Sugiyama 分层交叉归约[14]为布局理论基础；d3-force 的 `forceCollide`[2]给出"最小间距硬保证"的标准实现；cytoscape.js-fcose 的 `nodeSeparation`[3]与 Graphviz 的 `nodesep/ranksep`[4]给出"分离参数作为主参数"的工程先例——Engram v2.5 的"最小间距/最大间距"双滑条直接继承。

### 2.5 协议层

- **Model Context Protocol (MCP)**[1]：Anthropic 2024 年发布的模型-工具标准协议；stdio 传输是桌面客户端最通用形态；Rust 官方 SDK（rmcp）[9]使服务可随桌面应用同栈分发。

### 2.6 认知科学依据

- **编码特异性原则**（Tulving, 1973）[10]：提取线索在编码时决定——检索的上限由"写入时写了什么线索"决定；
- **记忆强度 = 突触强度 × 平行通道数**[11]：访问次数对应突触强度、图谱度数对应平行通道；
- **海马体索引理论**：海马体存索引不存内容——索引与内容分离的神经解剖学依据；
- **遗忘曲线**（Ebbinghaus, 1885）[12]与人类遗忘的多为"提取失败"而非"存储丢失"——归档优于删除。

### 2.7 生态位结论

| 系统 | 记忆载体 | 透明 | 生命周期 | 定位 |
|---|---|---|---|---|
| MemGPT/Letta | 服务端分页 | 低 | 无 | 通用记忆 API |
| Mem0 | 云端向量 | 低 | 无 | 通用记忆 API |
| GraphRAG | 文档+图索引 | 中 | 无 | 检索增强 |
| **Engram** | **纯文本 .md + 派生索引** | **高** | **巩固/遗忘** | **人机共治工程记忆** |

Engram 不与通用记忆 API 竞争：它的护城河是**透明（人可读可治）、可携带（文件夹即资产）、有遗忘**。

---

## 3 · 问题分解：从总问题到子问题

### 3.1 分层假设

Q0 分解为三层能力假设：

- **L1 人机交互层**：人用 GUI 看见并治理记忆——已完成（v2.6，涟漪/聚焦/常驻侧栏）；
- **L2 机器交互层**：AI 用 MCP 读写同一份记忆——主体完成（阶段一 M1–M4，9 工具）；
- **L3 记忆智能层**：记忆自组织（检索、遗忘、图注意力）——本规划核心。

### 3.2 已解决问题复盘（v1.0→v2.7 实证）

| # | 现实问题 | 根因 | 方案 | 归纳的原则 |
|---|---|---|---|---|
| P1 | 布局动画"冻结"在密集散点 | 点击节点触发 grab→停模拟、free 因"涟漪中不重排"跳过重启 | free 只认真实拖拽（≥8px）、dbltap 不杀模拟、前 30 帧禁早停 | **布局不可被输入事件杀死** |
| P2 | 切换图谱后布局"贴在一起" | 质心交叉归约以面积收缩换零交叉 | 归约每轮保持包围盒面积（缩放无关交叉数） | **归约不得压缩整体铺开** |
| P3 | 节点重叠/间距无保证 | 斥力为软约束 | 碰撞力每帧硬保证 ≥ 最小间距（d3-force 式） | **约束用硬保证而非调参期望** |
| P4 | 无关分量飘散、全局观察不便 | 无上限约束 | 跨分量对做上限拉回修正 | **最小/最大间距双边界** |
| P5 | 亮度层级观感不当 | 参数写死 | 亮度对比主滑条 + 默认值圆点一键还原 | **参数开放 + 默认值可回归** |
| P6 | 侧栏遮档画布控件 | fixed 覆盖层 | 画布 margin 动态预留（padding 对绝对定位无效） | **常驻 UI 必须让位画布** |
| P7 | 多值样式渲染崩溃 | cytoscape data() 不支持多值属性 | 逐边内联样式 | **先实测 API 边界再设计** |
| P8 | GUI 与外部写入竞态 | 乐观锁缺失 | tmp+rename 原子写 + updated 比对 + 串行队列 | **写入保护全覆盖** |

### 3.3 未解决问题分解

**Q1 接入面**：仅 stdio → 云端 AI 够不到。子问题：Q1.1 传输（HTTP/SSE）、Q1.2 鉴权、Q1.3 多设备。

**Q2 检索**：关键词匹配撑不起"凭语义想起"。子问题：Q2.1 编码线索缺失（写入侧）、Q2.2 语义召回（向量索引）、Q2.3 联想扩散（图结构）。

**Q3 遗忘**：无删除/归档/重要性/冲突检测。子问题：Q3.1 强度信号、Q3.2 归档机制、Q3.3 语义冲突、Q3.4 巩固蒸馏。

**Q4 生命周期**：记忆库随规模退化。子问题：Q4.1 访问统计口径、Q4.2 消退阈值、Q4.3 并发协议、Q4.4 审计。

**Q5 记忆图网络（终态）**：图元携带 QKV 信息，注意力驱动检索与组织。子问题：Q5.1 嵌入选型、Q5.2 图注意力传播、Q5.3 索引一致性、Q5.4 可解释性、Q5.5 GUI 联动。

---

## 4 · 递进式解决方案

### 4.1 L2 机器交互：工具即交互翻译（已主体完成）

**递进链**：读（6 工具）→ 写（3 工具）→ 守门（D2 词表、D3 乐观锁）→ 指南下发（D4）。

- `get_overview` = 人打开图谱扫一眼；`search` = 搜索框；`expand(depth=1|2)` = 单击涟漪 / 双击聚焦；`read_path` = "A 和 B 什么关系"；`read_node` = 侧栏 + 邻域名片；
- 写入三工具全部带健康守门：`create_node` 同名拦截（force 确认）、`update_node` 乐观锁（CONFLICT 不落盘）、`link_nodes` 词表校验（INVALID_REL）；
- 实测：31 项端到端全绿（含 CONFLICT、坏词表、重名三类边界）。

### 4.2 检索三层（L1→L2→L3）

| 层 | 方案 | 优先级 | 成本 | 理论依据 |
|---|---|---|---|---|
| L1 编码侧规范 | AI_GUIDE v8：存入时写 trigger 句 + tags（提取线索） | P0 | 零代码 | 编码特异性[10]——**检索上限的决定者** |
| L2 语义召回 | `recall(query,k)`：向量索引存 `.chain/index/`（可重建派生物）；embedding 可插拔、默认本地；不可用自动降级关键词并声明 | P0 | 中 | 海马体索引理论——索引与内容分离 |
| L3 联想扩散 | `expand` 升级扩散激活，`rel_desc` 作边权 | P1 | 低 | 平行通道假说[11] |

**降级链不变式**：工具永远可用——任何索引失效都必须退化为关键词 + 显式声明，绝不让检索工具"失效"。

### 4.3 遗忘四组件

1. **强度信号**（P1）：`access_count` / `last_accessed` 落 `.chain/stats.json`（派生文件，**不进 YAML**——防 git 噪音与 watcher 风暴）；强度 `f(访问次数, 最近访问, 图谱度数)`，预留 `importance` 第四因子；
2. **归档而非删除**（P1）：`archive_node` 写 `archived: true` + 标题 `[归档]` 前缀（GUI 零改动）；`unlink_nodes` 补断边不对称；
3. **语义冲突检测**（P1）：创建时高相似 hint，复用 `alternative` 边表达竞争——零新增概念；
4. **巩固蒸馏**（P2）：`consolidate` 将节点簇蒸馏为**语义骨架节点**（`contains` 回指），闲置节点建议归档；默认 `dry_run=true`（延续写入保护哲学）。理论校准：细节遗忘 = 主动精简留骨架，产物是骨架而非全文摘要。

### 4.4 记忆图网络（Q5，终态）

**图元 QKV 映射**：

| 角色 | 记忆图实体 | 载体 |
|---|---|---|
| Q 查询 | 当前对话意图编码 | recall 时现算 |
| K 检索签名 | 标题+正文+rel_desc 嵌入 | `.chain/index/` 派生 |
| V 内容 | 正文 + 邻域/路径叙述 | `.md` 文件本身 |
| 图结构 | 注意力传播先验 | 现有图谱 |
| 生命周期 | 强度信号（Q3.1） | stats.json |

**递进子阶段**：
- **4.1 向量索引**：本地嵌入（默认 BGE-small-zh，选型须过三组中文同义词实测门槛）；千级节点全库重嵌 < 1 分钟；
- **4.2 图注意力检索**：个性化 PageRank 变体——边转移概率 = `sim(child, query) × rel_weight × life_prior`（度归一化消 hub 偏置；rel_weight：contains 1.0 / solves 1.2 / alternative 0.8）；`expand` 输出权重分解 `{sim, rel, life}`；
- **4.3 自适应**：访问统计反馈进权重（与 4.3 节强度信号合流——**"遗忘即检索"闭环**）；仅当可解释权重实测不足时才评估本地微调（明确门槛）。

**GUI 联动**：涟漪亮度 = 拓扑层深衰减 × 注意力权重（权重通道默认关闭，保持现观感）。

### 4.5 接入面演进（Q1）

stdio（现状，本地客户端全覆盖）→ HTTP transport + token 鉴权（独立评估主线，决定"任意 AI"时间表）→ 多设备/多工作区注册表（远期）。

---

## 5 · 设计不变式（红线）

| # | 不变式 | 违反的后果 |
|---|---|---|
| I1 | 节点文件为唯一事实源（纯文本 Markdown） | 引入二进制/双写即腐化 |
| I2 | 一切派生数据（索引/统计/审计）可重建，与事实源分离 | 双索引漂移、不可审计 |
| I3 | GUI 零改动或近零改动 | 每加一层交互重写一层界面 |
| I4 | 工具即契约：变更进 CHANGELOG、版本化 | 客户端工具签名漂移 |
| I5 | 写入保护全覆盖（守门/乐观锁/原子写/串行队列） | AI 乱写、并发覆盖 |
| I6 | rel 三类型不扩张（contains/solves/alternative），语义细节交给 rel_desc | 线型语义膨胀、AI 误用 |
| I7 | 检索降级链：任何增强失效退化为关键词并显式声明 | 检索工具"失效" |
| I8 | 统计不进事实源（派生文件承载） | git 噪音、watcher 风暴 |
| I9 | 约束用硬保证（碰撞力/上限拉回），而非调参期望 | 布局退化重现 |

---

## 6 · 评估与验收

### 6.1 已获得实证

- 阶段一：**109 项 Rust 单测 + 31 项 stdio 端到端全绿**（握手/工具清单/9 工具/坏词表/乐观锁 CONFLICT/重名拦截/指南下发）；
- 布局硬保证实测（v2.5，story 37 节点）：最小间距设定 40/80/30 → 实测最小边缘间隙 39.9/88.7/36.1（均 ≥ 设定值）；无关分量上限 240 → 实测 241（连续 6 采样稳定）；
- 聚焦视图实测：双击拉近 2.3×（0.81→1.87），再双击精确回全局（0.81）；
- 界面遮挡实测：画布/控件与侧栏 16px 让位（展开态 1311<1327、收起态 1647<1663）。

### 6.2 各子问题验收标准

| 子问题 | 通过判据 |
|---|---|
| Q2.2 | `recall("布局")` 命中"力导向布局"；索引不可用时降级关键词并声明 |
| Q3.1 | 连续使用两周后 stats 反映真实触达；git diff 不含统计文件 |
| Q3.2 | 归档节点 GUI 可见 `[归档]` 前缀；可恢复；`unlink_nodes` 断边成功 |
| Q3.4 | `consolidate` dry_run 输出可审阅建议；确认后骨架节点 `contains` 回指 |
| Q5.1 | 三组中文同义词实测达标；千节点全库重嵌 < 1 分钟 |
| Q5.2 | `expand` 权重排序符合人眼判断；权重分解三项可解释 |
| Q1.1 | 云端 AI（HTTP + token）完成一次真实读写闭环 |

---

## 7 · 讨论

### 7.1 核心权衡

- **纯文本 vs 结构化存储**：牺牲查询效率换透明与可携带——用派生索引补偿（I2）；
- **本地优先 vs 云端**：放弃多设备即时同步，换隐私与零依赖——HTTP 接入面只解决"够得到"，不改变数据属地；
- **克制 vs 功能**：向量、微调均为"有门槛的可选项"——先结构后向量，先可解释后学习（I7、Q5.3 门槛）。

### 7.2 局限性

- 检索质量依赖嵌入模型对中文/公式混合内容的覆盖（选型门槛即为此设）；
- 千级节点为当前规模假设，超限后需向量库与增量索引（已列入演进路线）；
- 多客户端并发写的一致性由乐观锁 + 串行队列保证，极端时序下仍可能拒绝而非合并。

### 7.3 风险与应对（摘要）

| 风险 | 应对 |
|---|---|
| 协议/SDK 变动 | 薄封装适配层、锁版本 |
| 嵌入模型升级 | 全库重嵌脚本（I2 使成本可控） |
| AI 乱写膨胀 | 守门 + 指南 + 消退兜底 |
| 索引不一致 | 内容哈希校验 + 单点重嵌 |
| 过早训练 | 明确门槛：可解释权重实测不足 |

---

## 8 · 结论与未来工作

Engram 的演进遵循一条清晰的递进主线：**先把"人怎么用"翻译给机器（L1→L2），再让记忆自己会组织（L2→L3）**。阶段一已证明"文件即事实源 + 工具即交互翻译"成立（9 工具、31 项验收、随包交付）；阶段二以检索三层与遗忘四组件为主体，理论依据已校准（编码特异性、突触强度×平行通道、海马体索引理论），等待决策进入实现；终态为 QKV 图元 + 图注意力的记忆图网络，其中"遗忘即检索"闭环是 Engram 区别于一切通用记忆 API 的生态位。

未来工作的顺序：**M6（L1 规范 + recall）→ M7（归档 + 断边 + 强度统计）→ M8（冲突检测 + consolidate）**，HTTP 接入面作为独立并行主线评估；记忆图网络 4.1→4.2→4.3 随之上线。

---

## 参考文献

[1] Anthropic. *Model Context Protocol*. https://modelcontextprotocol.io, 2024.
[2] Bostock M. *d3-force: forceCollide*. https://d3js.org/d3-force/collide.
[3] iVis-at-Bilkent. *cytoscape.js-fcose*（nodeSeparation 参数）. https://github.com/iVis-at-Bilkent/cytoscape.js-fcose.
[4] Ellson J., Gansner E. et al. *Graphviz and Dynagraph — Static and Dynamic Graph Drawing Tools*. AT&T Labs Technical Report.
[5] Edge D., Trinh H. et al. *From Local to Global: A Graph RAG Approach to Query-Focused Summarization*. Microsoft Research, 2024.
[6] Behrouz A., Zhong P., Mirrokni V. *Titans: Learning to Memorize at Test Time*. 2024.
[7] Packer C. et al. *MemGPT: Towards LLMs as Operating Systems*. 2023.
[8] Mem0 / Letta 记忆平台技术文档.
[9] modelcontextprotocol. *Rust SDK (rmcp)*. https://github.com/modelcontextprotocol/rust-sdk.
[10] Tulving E., Thomson D. M. *Encoding specificity and retrieval processes in episodic memory*. Psychological Review, 1973.
[11] 记忆巩固的突触机制：记忆强度 = 突触强度 × 平行通道数（知乎校准原文，2026）。
[12] Ebbinghaus H. *Über das Gedächtnis*. 1885.
[13] Fruchterman T. M. J., Reingold E. M. *Graph Drawing by Force-directed Placement*. Software — Practice and Experience, 1991.
[14] Sugiyama K., Tagawa S., Toda M. *Methods for Visual Understanding of Hierarchical System Structures*. IEEE Trans. SMC, 1981.
[15] Tauri 2 / Svelte 5 / Cytoscape.js 官方文档。

---

*Engram 论文式设计整理 v1.0 · 2026-09-07 · 规划文档，未执行修改。*
