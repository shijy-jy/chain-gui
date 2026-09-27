# Engram 节点布局重设计方案（v3.0 布局专项）

> 状态：**已实现并验证**（算法层 + 渲染层 + 交互层全部落地）
>
> ⚠️ **后续变更（v3.3）**：**图谱上不再显示节点名称**（用户要求）。名称与正文由右侧常驻信息栏承载，
> 节点身份改为「类型配色 + 圆点大小(度) + 悬停看 id · 类型」。因此 §10 描述的整套标签预算系统
> （`src/lib/label_layout.ts`、`tools/label_verify.cjs`、字号与标签数滑条）**已随该决定删除**。
> 保留 §10 是为了记录当时的诊断与踩坑——`text-max-width` 后写不生效、取景与标签的振荡回路、
> 动画期间算标签导致错位等；这些结论对将来任何"要在图上渲染动态文字"的需求依然成立。

> 数据来源：`G:\perf1500`（1500 节点真实基准）、`D:\TA`（80 节点）、`G:\ta`（74 节点）、`G:\engram`、`test-data`
> 复现脚本：`tools/layout_verify.cjs`（算法回归）、`_scratch/graph_stats.cjs`、`_scratch/layout_ab.cjs`、`_scratch/layout_detail.cjs`、`_scratch/layout_radial.cjs`、`_scratch/make_layout_svg.cjs`（标签回归 `tools/label_verify.cjs` 已随 v3.3 删除）
> 对比图：`_shots/layout-compare.png`；实测截图：`_shots/v3-ta-80-clean.png`、`_shots/v3-perf1500-depth3.png`
> 外部依据：d3-force 源码（`main` 分支）、Obsidian 官方 Graph view 文档、`obsidian-graph-spawn` 逆向记录
>
> **实施结果速览**（详见 §9、§10）
> | 指标 | 改造前 | 改造后 |
> |---|---|---|
> | 边交叉数（1500 节点） | 361 997 | **0** |
> | 布局耗时（1500 节点） | 174 ms / 80 帧主线程 | **4.5 ms 单次** |
> | 收敛后残余位移（80 节点真实图） | 211 px/帧（永不静止） | **0（完全静止）** |
> | 可复现性 | 每次不同（`Math.random` 兜底） | **同输入同输出** |
> | 渲染帧率（44 / 1500 节点） | — | **163.9 fps（= 165Hz 垂直同步上限）** |
> | 标签遮挡 | 1500 节点全展开时 ~530 个标签互相压字并盖住节点 | **硬约束：不盖节点、不压字**；1500 节点全展开时仅显示 7 个 |
> | 标签字号 | 11px 但随 zoom 缩放（fit 后屏幕 3.9px，糊） | **屏幕恒 11px**（世界字号按 1/zoom 反算） |



---

## 0. 一句话结论

Engram 的图**在数学上是森林（forest of trees）**，而现在用的是**为一般图设计的全局力导向模拟**。这两个事实错配，就是"不流畅、观感不舒服"的全部根源：在 1500 节点上现有算法不是"不够好"，而是**爆炸**（世界尺寸 46000×49500 px、36 万次边交叉、80 帧后每帧仍移动 26521 px）。改成树感知的确定性布局，计算量从 174ms/80 帧变成 **1.2–2.0ms/单帧**，边交叉从 **361997 → 0**。

---

## 1. 关键证据：图到底是什么结构

用 `_scratch/graph_stats.cjs` 直接解析各工作区 `.chain/nodes/*.md` 的 frontmatter：

先看小图（用户日常面对的 80 / 74 节点图）——**它们同样是坏的**，只是坏得不那么显眼：

| 指标 | 现有·`D:\TA` 80 节点 | 现有·`G:\ta` 74 节点 | 树感知布局（同图） |
|---|---|---|---|
| 边交叉数 | **206** | **18** | **0 / 0** |
| 最小间距（40+半径）违例对 | **48 对** | **12 对** | 0 / 0 |
| 布局耗时 | 27.8 ms / 80 帧 | 17.4 ms / 80 帧 | **0.5–12 ms 单次** |
| 收敛后残余步长 | **300 → 211 px/帧** | **358 → 308 px/帧** | 0（完全静止） |
| 世界包围盒 | 1013 × 1127 | 2493 × 3808 | 1380 × 1056 |

**这是全篇最重要的一条实测**：残余步长在两个**小图**上比在 1500 节点大图上更"稳定地没收敛"——80 节点末帧仍有 211px/帧、74 节点仍有 308px/帧，而且两个真实工作区的残余速度都**远高于**它们的节点尺寸（14–38px）。也就是说：

> 用户看到的"不流畅、不舒服"，主体不是大图性能问题，而是**小图上永不停歇的漂移**：节点以超过自身尺寸几倍的速度被弹簧拽、被碰撞推开，互相抵消后表现为整块蠕动 + 局部乱挤（48 对/12 对间距违例就是"挤"的直接证据）。截图时它在动，回看时位置又变了（`Math.random()` 兜底导致不可复现）。

顺便，这也解释了为什么"调滑条没用"：§4bis.2 的 5 处模型缺项决定了无论怎么调，系统都不会收敛到静止。

再看各工作区的拓扑（用 `_scratch/graph_stats.cjs` 直接解析 `.chain/nodes/*.md` frontmatter）：

| 工作区 | 节点 | 边 | 分量 | 最大深度 | 最大度 | **多父节点** | **环** | 边类型 |
|---|---|---|---|---|---|---|---|---|
| `G:\perf1500` | 1500 | 1499 | 1 | 7 | 9 | **0** | **0** | contains ×1499 |
| `D:\TA` | 80 | 79 | 1 | 7 | 9 | **0** | **0** | contains ×79 |
| `G:\ta` | 74 | 73 | 1 | 12 | 19 | **0** | **0** | contains ×73 |
| `G:\engram` | 13 | 10 | 3 | 2 | 10 | **0** | **0** | contains |
| `test-data` | 5 | 4 | 1 | 2 | 2 | **0** | **0** | contains |

三条硬事实：

1. **每个节点最多一个 `parent`**（`docs/Engram_schema_v1_定义与迁移接口.md` 的 frontmatter 只有一个 parent 字段）→ 图是有向森林，不是一般图。
2. **无环**（代码层有 cycle guard，实测 cycles=0）→ 不存在需要力导向松解的环约束。
3. **树在正确布局下边交叉理论值为 0**。任何 >0 的交叉都是布局算法的失败，不是数据的问题。

`perf1500` 的度分布：度1（叶）1067 个、度2 有 212 个、度7–8 有 192 个、度9 有 6 个 → 典型"少量枢纽 + 大量叶子"的浅宽树（maxDepth 7）。

### 1.1 这意味着什么

- 力导向模拟擅长的是**未知拓扑的一般图**（社区结构、环、多父 DAG）。Engram 没有这些。
- 树有专门的 **O(n) 确定性最优算法**（Reingold–Tilford / Buchheim），能在 1–2ms 内给出零交叉、可复现、不抖动的布局。
- 继续用物理模拟去逼近一个已知有解析解的问题，是"用迭代法解一元一次方程"——既慢又不准，还引入了抖动。

---

## 2. 现有实现的量化诊断

### 2.1 现有算法在 1500 节点上是爆炸的

忠实复刻 `src/App.svelte:220-529`（v2.5 参数：`minDist=40 / repulsion=30000 / gravity=0.15 / edgeLen=80 / MAX_F=60 / MAX_STEP=10 / alpha 衰减 0.97`）跑 `G:\perf1500`：

| 指标 | 现有（环形散点+力导向 80 帧） | 树感知 layered | 树感知 radial |
|---|---|---|---|
| 计算耗时 | 174.5 ms（**每帧约 2.2ms × 80 帧**，主线程） | **1.2 ms**（单帧 O(n)） | **2.0 ms**（单帧 O(n)） |
| 边交叉数 | **361 997** | **0** | **0** |
| 世界包围盒 | 46 023 × 49 541 px | 49 036 × 672 px | **1 056 × 1 132 px** |
| 收敛后残余步长 | **26 521 px/帧（完全没收敛）** | 0 | 0 |
| 最小间距违例 | 1 对 | 0 | 0 |

### 2.2 三个根因（按严重度）

**根因 1：初始散点半径随 n 线性膨胀（`chain_to_cytoscape.ts:50`）**

```ts
const R = 180 + n * 5;                    // n=1500 → R = 7680px
const ringGap = Math.max(120, R * 0.42);  // → ringGap = 3226px
// 第 d 层半径 = R + (d-1)*ringGap → 第7层 = 27034px，直径 54067px
```

初始世界直径 5.4 万像素，而视口约 1600×900。`initialView()` 立刻 fit，于是首帧就是 `zoom≈0.016`、节点屏幕直径 **0.41px**。用户看到的是"一坨雾"，而且是**先爆炸再拉回**。

**根因 2：弹簧力无上限，位移钳制治标不治本（`App.svelte:411` vs `:511`）**

```ts
const f = SPRING * (d - REST) * alpha;   // d=46000 → f = 0.15*(46000-80) = 6888（无 MAX_F 钳制）
// 只有位移被钳：if (sp > MAX_STEP) 限到 10px/帧
```

斥力有 `MAX_F=60` 上限（`:391`），弹簧**没有**。第一帧弹簧就把节点以 ~6900 的力度甩出去，位移钳制只是限速而非限力 → 速度场在 80 帧内持续巨大，`maxStep` 始终 2 万+ px，早停条件（`maxStep<0.3`）永远不成立 → 模拟"跑完配额"就停在一个完全未收敛的中间态。

**根因 3：斥力作用范围被硬截断在 152px（`App.svelte:281`）**

```ts
const cell = Math.max(edgeLen, 2 * (minDist + 2 * maxR));   // = 152px
// 只查 3×3 邻胞 → 超过 152px 的节点对之间斥力为零
```

世界要铺开到 4 万像素，但斥力只在 152px 内生效 → 远距离节点之间没有任何排斥约束，只能靠 `0.05*alpha` 的弱中心引力勉强拉回，而 alpha 每帧衰减 → 越到后面越拉不住。这就是"既爆炸又缩团"的矛盾观感的来源。

**附加根因 4：残余抖动（永不静止）**

即使在小图上模拟"收敛"，末帧残余速度仍有 ~10px（受 `MAX_STEP` 钳制），叠加 `alpha *= 0.97` 的 80–400 帧预算，宏观看是"整块缓慢蠕动"。文档注释里"收敛"的定义是 `maxStep<0.3` 连续 12 帧，实测大图从未达成。

**附加根因 5：算力与渲染抢同一个主线程**

模拟在 `requestAnimationFrame` 里跑（`:526`），同一帧内还要 `cy.batch()` 写回 1500 个节点位置（`:523`），触发 cytoscape 全量位置重算 + 重绘。1500 节点 × 每帧 2.2ms 物理 + 1500 次位置写入 → 帧预算直接爆掉，用户拖动时更明显。

### 2.3 视口换算：为什么"看着像雾"

1600×900 视口、`fit(undefined, 60)` 之后：

| 布局 | 世界尺寸 | fit zoom | 节点屏幕直径 | 标签字号 |
|---|---|---|---|---|
| 现有力导向 | 46 023 × 49 541 | 0.016 | **0.41 px** | 0.17 px |
| 现有力导向（80 节点图） | — | ~0.6 | ~16 px | 6.6 px（卡在阈值上） |

`min-zoomed-font-size` 分档（`ui/perf.ts:26-43`）在 1500 节点档位是 12 → 标签字号 0.17px 远低于阈值，**全部丢弃**。所以大图 fit 之后既没有标签也没有可辨认的节点，只剩"雾"。

---

## 3. 新方案：三层解耦

核心思路：**把"位置从哪来"和"怎么画"彻底分开**，并且让布局从"逐帧迭代的物理过程"变成"一次算完的纯函数"。

```
┌─ L1 结构层（布局）── 输入拓扑 → 输出浮点坐标，纯函数、可复现、O(n)
│    · 模式选择：layered（≤300 可见节点） / radial（大图默认） / 局部力导向（仅多父或环）
│    · 一次算完，不跑动画收敛
│
├─ L2 过渡层（动画）── 只在"布局结果变了"时插值 300ms，cubic ease-out
│    · 从旧坐标补间到新坐标；不重跑物理 → 永不抖动、永不超调
│
└─ L3 渲染层（cytoscape）── 静止时零位置写入
     · 视口裁剪 + 分级细节（label/edge/节点按 zoom 与视口决定画不画）
```

### 3.1 布局模式与选择规则

| 模式 | 算法 | 适用 | 实测 |
|---|---|---|---|
| **layered**（分层整洁树） | Buchheim/Reingold–Tilford O(n)，同层对齐、父节点居中于子节点 | 可见节点 ≤ 300；小图（≤80）默认 | 74 节点：bbox 1380×1056，aspect 1.31，fit zoom 0.74，节点屏显 19px **全部可读** |
| **radial**（径向整洁树） | 子树占角度扇区、扇区 ∝ 叶子数（Shneiderman），层半径线性递增 | 大图（>300）默认；深度 >8 | 1500 节点：bbox **1056×1132**，fit zoom 0.69，节点屏显 17.9px |
| **local-force**（局部力导向） | 现有模拟，但只在"含多父/环"的子图内跑，且带冷却终止 | 未来出现多父 DAG 或交叉引用时 | 保留代码但默认不启用 |

**选择规则伪代码**：

```
if (visibleNodes <= 300)  layered()
else                      radial()
if (hasMultiParentEdges)  layered/radial 打底 + 仅对该子集做 local-force 松弛（限 60 帧、必收敛）
```

### 3.2 为什么 radial 是大图的正解（而不只是"另一种画法"）

同样 1500 节点：

- layered：宽 49 036px、高 672px，aspect **73:1** → 一个 16:9 视口怎么 fit 都是"一条细线"，屏显 0.82px。
- radial：1056×1132，aspect **0.93** → 接近正方形，跟视口比例匹配，fit zoom 是 layered 的 22 倍，屏显 17.9px。

原因：layered 的宽度 ∝ 叶子总数 × 叶间距（1067 个叶子横向铺开），radial 把叶子铺在**圆周**上，宽度 ∝ √n 量级。对"枢纽+大量叶子"的浅宽树，这是决定性的。

### 3.3 渐进披露（真正的"流畅"来源）

实测"展开到第 N 层"的可读性（perf1500）：

| 展开层数 | 可见节点 | fit zoom | 节点屏显 | 标签字号 | 结论 |
|---|---|---|---|---|---|
| 1 | 7 | 6.70 | 174 px | 73.7 px | 极清晰 |
| 2 | 44 | 0.93 | 24.2 px | **10.2 px** | **舒适** |
| 3 | 263 | 0.15 | 4.0 px | 1.7 px | 已不可读 |
| 4 | 1262 | 0.03 | 0.8 px | 0.4 px | 雾 |

**结论：可视可读的预算约 150–300 个节点。** 超过就必然成雾——这不是布局算法能解决的，是屏幕像素的物理下限。所以：

- 大图**默认折叠**到 2 层（几百节点内，标签可读）；
- 点击节点 = 展开/折叠其子树（`◀▶` 角标），展开时用径向布局重排 + 300ms 补间；
- 提供"聚焦视图"= 以选中节点为根、深度 N 的径向子图（现有双击聚焦的升级版，改为**重布局**而不是只 dim）；
- "全图总览"作为一个**明确的降级模式**（只有点和线，无标签），而不是默认体验。

### 3.4 渲染层优化（静止即零成本）

1. **布局期间不做逐帧位置写入**：一次算完 → 一次 `cy.batch()` 写入 → 之后只有补间动画期间写。
2. **视口裁剪**：cytoscape 本身不裁剪画布外的元素；1500 节点全量绘制是常态。径向布局天然把节点限制在 1056×1132 的圆盘内，**很大程度缓解**（放大后视口外节点仍会被绘制，需要时用 `display:none` 按视口分块控制）。
3. **标签分级**：`min-zoomed-font-size` 已有，但要改成**基于可见节点数 + zoom 的动态阈值**，而不是仅按总节点数分档（现在 1500 节点档位设 12，导致 80 节点的图与 1500 节点的图用同一套感知策略）。
4. **边分级**：低于某 zoom 只画"骨架边"（连通分量的生成树边），其余边淡出/不画。1500 边在 zoom<0.2 时视觉上只会糊成一片。
5. **静止时完全停止 rAF**：不再有任何"呼吸式蠕动"。这是"舒服"的关键——**静态画面必须真的静止**。

### 3.5 是否要把布局搬进 Web Worker

- 树布局 1–2ms，**不需要** Worker。
- 但若保留 local-force 兜底（多父/环场景）或未来引入 Barnes-Hut 大图松弛，则应放进 Worker + `Float64Array` transfer，避免与渲染抢主线程。
- 建议：**现在不做**（收益低于复杂度），但把布局函数设计成**纯函数 + 可转移数组接口**（输入 `Int32Array` 边表 + 输出 `Float64Array` 坐标），为将来搬 Worker 留接缝。

---

## 4. 观感目标：Obsidian 能借鉴什么、不能借鉴什么

### 4.1 先澄清"拿源码"这件事

Obsidian 桌面主程序**闭源**。`obsidianmd` 组织下的公开仓库实测清单（GitHub API 拉取，共 20 个）里没有任何一个是主程序或图谱视图的实现：

| 仓库 | 是什么 | 有图谱布局代码吗 |
|---|---|---|
| [obsidian-api](https://github.com/obsidianmd/obsidian-api) | `obsidian.d.ts` 类型定义 | 否 |
| [obsidian-help](https://github.com/obsidianmd/obsidian-help) | 帮助文档源 | 否（但有图谱设置语义） |
| [obsidian-developer-docs](https://github.com/obsidianmd/obsidian-developer-docs) | 开发者文档 | 否 |
| [obsidian-sample-plugin](https://github.com/obsidianmd/obsidian-sample-plugin) | 插件模板 | 否 |
| [jsoncanvas](https://github.com/obsidianmd/jsoncanvas) | Canvas 文件格式规范 | 否（Canvas ≠ Graph view） |
| [obsidian-maps](https://github.com/obsidianmd/obsidian-maps) | Bases 的地图视图 | 否 |
| 其余（releases / translations / themes / eslint-plugin / knap / clipper / importer / headless …） | 周边工具 | 否 |

**结论：没有可供移植的 Obsidian 布局算法。** 能拿到的是官方帮助文档里的**设置语义** + 社区插件的逆向结论。真正的算法必须来自开源实现（见 4.4）。

### 4.2 官方文档确认的 Obsidian 力模型（[Graph view 官方文档](https://help.obsidian.md/plugins/graph)）

Obsidian 对用户只暴露**四个**力参数：

| 官方设置 | 官方描述原文（要点） | 对应的 d3-force 概念 |
|---|---|---|
| **Center force** | "How compact the graph is. Higher values create a more circular layout." | `forceX` / `forceY`（中心引力） |
| **Repel force** | "How strongly nodes push each other apart." | `forceManyBody`（带 Barnes–Hut 的四叉树） |
| **Link force** | "The tension on each link — like tightening or loosening a rubber band." | `forceLink.strength()` |
| **Link distance** | "The length of lines between notes." | `forceLink.distance()` |

> 上述"对应 d3-force 概念"一列为**推断**（依据参数语义与下文 4.3 的逆向证据）；四参数本身为 VERIFIED。

另有两条官方确认的观感规则：

- **节点大小 ∝ 被链接次数**："The more notes that link to a given note, the larger its circle becomes." → 与 Engram 现在的 `14 + min(√degree,6)*4`（`App.svelte:1298`）同一思路，Engram 的平方根缓增+封顶是合理的。
- **Text fade threshold**："Control how transparent note names appear." → Obsidian 把**标签淡出阈值做成了用户可调项**，而不是硬编码。Engram 现在硬编码在 `ui/perf.ts` 分档里（按总节点数选 6/8/10/12），既不可调，也不随实际 zoom 与可见密度变化。

还有官方确认的**渐进披露**机制：

- **Local graph + depth 滑条**："you can set a **depth** level — each depth level reveals notes connected to the notes shown at the previous level"，并且明确建议用它来探索"某个笔记如何连向整个 vault"。
- **Groups**（最多若干组）="Color-code groups of notes to distinguish them visually"，按搜索表达式分组上色。
- **Filters**：可关掉 orphans、attachments、unresolved links → 用**过滤**来控制规模，而不是靠布局算法硬撑。

### 4.3 社区逆向的关键结论（对 Engram 直接可用）

社区插件 [obsidian-graph-spawn](https://github.com/tjqscott/obsidian-graph-spawn) 的 README 是对 Obsidian 图谱模拟最扎实的一份公开逆向，三条硬事实：

1. **模拟跑在 Web Worker 里**：`leaf.view.renderer.worker`，其 `sim.js` 是"d3-force + 一层薄消息处理"。
2. **所有节点都从原点 (0,0) 起步**：`{ id, x: 0, y: 0, vx: 0, vy: 0, fx: null, fy: null }`，靠浮点噪声打破对称性。
3. **worker 协议接受外部注入初始位置**：`nodes` payload 是 `id → [x, y]` 映射，且**确定性**（插件用带种子的随机数发生器，不用 `Math.random`，保证同一张图收敛到同一画面）。

它还给出一个对 Engram 极其关键的判据（作者的"它做不到什么"）：

> "**It does not untangle a single connected hairball.** If your vault is one component, there is nothing to separate and this will do nothing for you. It is for forests."

**这恰好说明：连 Obsidian 生态的补丁都救不了"单一连通分量 + 力导向"的组合**，而 Engram 的每一个真实工作区都是**单一连通分量**（见 §1 表）。所以"照抄 Obsidian 的力参数"这条路在 Engram 上从原理上就不成立——它面对的问题不同（一般 vault 是多分量的稀疏链接图），Engram 面对的是**一棵树**。

### 4.4 该借鉴的四条 / 该弃用的三条

**借鉴（观感层面）**

| 借鉴项 | Obsidian 做法 | Engram 落法 |
|---|---|---|
| 节点大小 = 关系重要性 | 半径 ∝ 入链数 | 已有（度 → 半径，平方根缓增封顶），保留 |
| 标签淡出阈值 | 用户可调的 **Text fade threshold** | 改为**动态阈值**（zoom × 可见节点数），并暴露一个滑条 |
| 局部视图 + 深度控制 | Local graph 的 depth 滑条 | "聚焦视图"升级为**以选中节点为根重布局 + 深度滑条**（本方案 §3.3） |
| 用过滤控制规模 | Filters（orphans/attachments/unresolved） | 已有归档/代码筛选；补"按类型/状态/深度过滤" |
| 布局与渲染解耦 | **模拟在 Web Worker**，worker 只吐坐标 | 布局做成纯函数；坐标一次算完（暂不搬 Worker，见 §3.5） |
| 初始位置可外部注入 + 确定性 | worker 收 `id → [x,y]`，种子随机 | **布局函数必须纯函数、同输入同输出**（验收指标 §5 已列） |

**弃用（原理层面）**

| 弃用项 | 原因 |
|---|---|
| 把力参数当万能旋钮 | Obsidian 用户生态的共识是"四个滑条解决不了初始化/结构问题"（graph-spawn 的核心论点）；Engram 的 `/ 斥力 / 引力 / 边距 / 最大间距` 四个滑条**同样治不了 §2 的爆炸** |
| 每帧跑的全局物理 | Obsidian 用 d3-force + Barnes–Hut（O(n log n)）+ Worker 才敢跑；Engram 是手写 O(n·k) 网格 + 主线程，且**没有上限钳制的弹簧**。在树上更是**根本不需要**物理 |
| 单一"有机"布局覆盖所有规模 | Obsidian 靠过滤 + 局部图来控制规模；Engram 若坚持一张图看全 1500 节点，任何布局都会落到"雾"（§2.3 已量化：0.41px） |

### 4.5 一句话对比

> Obsidian 的图谱是**多分量稀疏链接图**，所以它需要 Worker 里的 d3-force，并且**至今要考社区插件补初始位置**；Engram 是**单分量树**，该用 O(n) 的确定性树布局——**照抄 Obsidian 等于用最贵的工具解最简单的问题**。

---

## 4bis. 开源算法侧的参数基线（d3-force 源码级对照）

以下默认值**全部来自 d3-force 源码**（`main` 分支），不是二手转述。这是判断 Engram 力模型"哪里抄歪了"的基准。

### 4bis.1 d3-force 的真实默认值

`src/simulation.js`：

| 参数 | d3 默认值 | 含义 |
|---|---|---|
| `velocityDecay` | **0.6** | 每 tick 速度只保留 60%（即阻尼 40%） |
| `alphaMin` | **0.001** | 冷却终止阈值 |
| `alphaDecay` | `1 - alphaMin^(1/300)` ≈ **0.0228** | 约 **300 tick** 冷却到静止 |
| 初始位置 | **确定性叶序螺旋**：`半径 = 10*√(0.5+i)`、`角度 = i*π*(3−√5)` | 不是随机散点，是黄金角螺旋 |
| `randomSource` | `lcg()` **种子随机** | 可复现 |
| 积分 | `node.x += node.vx *= velocityDecay` | 半隐式欧拉，无位移钳制 |

`src/manyBody.js`（斥力）：

| 参数 | d3 默认值 |
|---|---|
| `strength` | **−30** |
| `theta` | **0.9**（`theta2 = 0.81`，Barnes–Hut 打开） |
| `distanceMin` | **1** |
| `distanceMax` | **Infinity（无截断）** |
| 算法 | `d3-quadtree` 四叉树 + `visitAfter/visit` → **O(n log n)** |
| 近距保护 | `if (l < distanceMin2) l = Math.sqrt(distanceMin2 * l)`（**软化 1/d²，防奇点**） |
| 重合节点 | `jiggle(random)` 用**注入的种子随机**打破对称 |

`src/link.js`（弹簧）：

| 参数 | d3 默认值 |
|---|---|
| `distance` | **30** |
| `iterations` | **1** |
| **`defaultStrength`** | **`1 / min(count[source], count[target])`** —— `count` 是节点度数 |
| 位移分配 `bias` | `count[source] / (count[source] + count[target])` —— **度数高的节点动得少** |
| 力的计算基准 | 用**预测位置** `target.x + target.vx − source.x − source.vx`（含速度，不是纯位置差） |

`src/collide.js`（碰撞）：

| 参数 | d3 默认值 |
|---|---|
| `strength` | **1** |
| `iterations` | **1** |
| 算法 | `d3-quadtree`（O(n log n)，不是 O(n²) 双循环） |
| **施加方式** | **写入 `vx/vy`（速度），不是直接改坐标** |
| 半径分配 | 按 `ri²/(ri²+rj²)` 加权，**两个节点都动** |

### 4bis.2 Engram 力模型与 d3 的 5 处根本性偏离

| # | 维度 | Engram 现状（`App.svelte`） | d3-force 基准 | 后果 |
|---|---|---|---|---|
| 1 | **斥力截断** | `cell = 152px`，只查 3×3 邻胞（`:281,353-405`）；>152px 的节点对斥力为 **0** | `distanceMax = Infinity`，四叉树对**所有**距离生效 | 世界要铺到 4 万像素，斥力却只管 152px → 只能靠弱中心引力（0.05α，且 α 每帧衰减）勉强拉，**既爆炸又拉不住** |
| 2 | **近距奇点** | `if (d2 < 4) { d2 = 4; dx = random*4; dy = random*4 }`（`:385-389`）——用 **非种子 `Math.random()`** | `l = √(distanceMin²·l)` 软化；`jiggle()` 用注入的**种子**随机 | 布局**不可复现**（同一张图每次跑出来不一样）；重合节点方向随机抖动 |
| 3 | **弹簧强度** | 常数 `SPRING = 0.15`（`:411`），**与度数无关** | `1/min(deg_src, deg_tgt)`，且按 bias 分配：**高度数节点几乎不动** | 1500 节点 1499 条边全部以 0.15 刚度硬拉，**总拉力 ∝ 边数**，直接把图拽爆；枢纽节点被拖得最惨（本应最稳） |
| 4 | **弹簧无上限** | `f = SPRING*(d−REST)*alpha`，**没有 MAX_F 钳制**（斥力有 60 上限，弹簧没有） | `(l − distance)/l * alpha * strength`，位移与 `l` 同阶、被 strength 压低 | d=46000 时 f=6888 → **第一帧即爆炸**；位移钳制 `MAX_STEP=10`（`:511`）只是限速，不是限力 |
| 5 | **碰撞写坐标** | 直接改 `pos.x/pos.y`，且**串行迭代中即时生效**（`:375-383, 187-213`） | **写入 `vx/vy`**，由积分统一推进；四叉树加权 | 串行就地改坐标 = 力的施加顺序影响结果（非对称），多个邻居的推挤互相抵消 → 宏观看是"**整块缓慢蠕动**" |

另外两处：

| # | 维度 | Engram | d3 |
|---|---|---|---|
| 6 | 阻尼 | `v *= 0.86`（保留 86%） | `v *= 0.6`（保留 **60%**）→ d3 阻尼强得多，**收敛快、不飘** |
| 7 | 冷却预算 | `alphaDecay = 0.97`、`forceMaxIter` 80–400 帧 | 约 **300 tick** 到 `alphaMin`；1500 节点档位 80 帧**根本不够收敛** |

**结论**：Engram 的自研模拟是"d3-force 风格"的**近似版，但恰好丢了 d3 里保证稳定的那几件东西**——无限程斥力、近距软化、度数加权弹簧、碰撞走速度、强阻尼。这不是调参能补的，是模型缺项。

### 4bis.3 若要保留物理兜底（多父/环场景）该怎么改

如果将来 Engram 出现多父 DAG 或交叉引用、需要保留力导向，应按 d3 模型重写而不是继续调现有参数：

1. 斥力：换成四叉树 Barnes–Hut（`theta=0.9`、`distanceMax=Infinity`、`distanceMin=1` 软化），复杂度 O(n log n)。
2. 弹簧：`strength = 1/min(deg_src, deg_tgt)`，`bias` 按度数分配，力的计算带入 `vx/vy` 预测位置。
3. 碰撞：`forceCollide` 语义——**写入速度**、四叉树、半径加权、`iterations=1`。
4. 积分：`velocityDecay = 0.6`，去掉 `MAX_STEP` 位移钳制（钳制掩盖问题而非解决问题），改由 `alpha` 冷却自然收敛。
5. 随机：换成**种子 LCG**（d3 用 `lcg()`），保证同输入同输出。
6. 初始位置：用 d3 的**叶序螺旋**（半径 `10√(0.5+i)`）代替 `R = 180 + n*5` 的巨型圆环；或直接用树布局坐标作为初始位置。

> 注意：以上是"兜底方案"。**对 Engram 当前的树结构，正确做法是 §3 的确定性树布局——不需要物理模拟。**

### 4bis.4 开源实现侧还确认了什么

- **d3-force 的默认初始位置就是确定性螺旋**（`initialRadius=10`、`initialAngle=π(3−√5)` 即黄金角）——即"好的初始位置比调力参数更重要"这一点，d3 自己就是按这个原则设计的；而 Obsidian 恰恰覆盖掉了它（改成全部堆在原点），这正是 `obsidian-graph-spawn` 要修的问题。
- d3-force 主模拟里**没有** `forceCollide` 的默认启用，也没有内置的分层/树布局——树布局属于另一支（Buchheim/Reingold–Tilford/ELK），本方案 §3 采用的就是这一支。



---

## 5. 验收指标（可量化）

新方案必须能过这些机器可验的指标（脚本已就绪）：

| 指标 | 现有 | 目标 | 验证方式 |
|---|---|---|---|
| 边交叉数（1500 节点树） | 361 997 | **0** | `_scratch/layout_ab.cjs` |
| 边交叉数（80 节点真实图 `D:\TA`） | 206 | **0** | 同上 |
| 边交叉数（74 节点真实图 `G:\ta`） | 18 | **0** | 同上 |
| 布局计算耗时（1500 节点） | 174.5 ms / 80 帧 | **< 5 ms 单次** | 同上 |
| 世界包围盒长宽比 | 0.93（但面积 2280M） | **0.5–2.0 且面积 < 50M** | 同上 |
| 收敛后残余位移 | 26 521 px/帧（80 节点图亦有 211 px/帧） | **0（完全静止）** | 同上 |
| 最小间距违例（80 节点图） | 48 对 | **0** | 同上 |
| 布局可复现性 | 随机（`Math.random` 兜底） | **同输入同输出** | 跑两次比对坐标哈希 |
| 可见节点 ≤2 层时标签字号 | 0.17 px | **≥ 10 px** | `layout_radial.cjs` 的渐进披露表 |

---

## 6. 实施顺序建议

1. **P0 · 止血（小改动，立刻消除爆炸）**
   - 给**弹簧力**加与斥力同级的 `MAX_F` 钳制（`:411`）——这是爆炸的第一推力。
   - 把 `R = 180 + n*5` 的巨型圆环（`chain_to_cytoscape.ts:50`）换成不随 n 线性膨胀的初始散点。
   - 把 `Math.random()` 兜底换成种子随机（`:387`），恢复可复现性。
   - ⚠️ P0 只能消除"爆炸"，**不能解决"蠕动"**——根因是碰撞写坐标 + 冷却预算不足（§4bis.2 的 #5、#7）。
2. **P1 · 换布局（主要工作量）**：把 `runForceLayout`（`App.svelte:220-529`，约 310 行）替换为 `treeLayout(pos, mode)` 纯函数（layered/radial 双模式），保留 300ms 补间动画。**可一并删除**这些为力导向服务的补丁：
   - `polishCrossings` / `countCrossings` 质心归约（`:75-218`）——树布局下交叉数本来就是 0
   - 交叉惩罚力（`:419-472`）
   - 连通分量间距上限补偿（`:473-495`）
   - `R = 180 + n*5` 的环状散点

   预计净删 250+ 行，同时把布局耗时从 174ms（1500 节点）降到 2ms。
3. **P2 · 渐进披露**：折叠/展开 + 聚焦重布局（§3.3）——这是大图体验的关键。
4. **P3 · 感知分级**：动态标签阈值（暴露成滑条，对齐 Obsidian 的 Text fade threshold）、边骨架分级、**静止即停 rAF**。
5. **P4 · 兜底**：若未来出现多父 DAG/交叉引用，按 §4bis.3 用 d3 模型重写 local-force（而不是继续调现有参数）。

> **滑条的去留**：现在的 `最小间距 / 斥力 / 引力 / 最大间距` 四个滑条是**力导向的参数**。换成树布局后它们失去意义，建议替换为：`层间距`（levelGap）、`同层间距`（siblingGap）、`布局模式`（分层/径向/自动）、`标签淡出阈值`。这四条才是树布局下真正影响观感的旋钮。

---

## 7. 附：对比可视化

`_shots/layout-compare.png`（1680×1040）由 `_scratch/make_layout_svg.cjs` 生成 SVG 后无头 Edge 栅格化，四宫格同数据同视口对比：

| 面板 | 内容 |
|---|---|
| 左上 | 现有算法 · 全图 1500 节点 → 一团蓝雾（0.41px 节点 + 36 万次交叉） |
| 右上 | 径向树布局 · 全图 1500 节点 → 清晰同心环，结构一眼可读 |
| 左下 | 现有算法 · 只展开 2 层（44 节点）→ 可辨但有交叉与间距违例 |
| 右下 | 径向树布局 · 只展开 2 层（44 节点）→ 零交叉、整齐、静止 |

> 说明：左列面板每次运行会有差异（现有算法含 `Math.random()` 兜底，见 §4bis.2 #2），右列是完全确定性的。

---

## 9. 实施记录（v3.0 已落地）

### 9.1 代码变更

| 文件 | 变更 |
|---|---|
| `src/lib/tree_layout.ts` | **新增**（约 480 行）：`computeTreeLayout` 纯函数、`layered`/`radial` 双模式、`nodeDisplaySize`、`pickMode`、`estimateLayeredSize`、`preferLayered`、`rootNodeId` 口径 |
| `src/App.svelte` | 删除 `runForceLayout`/`polishCrossings`/`countCrossings`/`segCross`/`applyMinDist`（约 310 行力导向代码）→ 换成 `relayout()` 纯函数 + 300ms 补间；新增渐进披露（`visibleSet`/`shallowDepth`/`autoDepthFor`）、`fitVisible` 统一适配出口、`hiddenNodes` 可见性同步、拖拽=钉住（`lock()`）+「重排」按钮、四个新滑条 |
| `src/lib/chain_to_cytoscape.ts` | 删除"根锚原点 + BFS 同心圆环"初始散点与连通分量锚点（约 110 行，力导向的起点，环半径随 n 线性膨胀）；归档节点外围环改常数半径 |
| `tools/layout_verify.cjs` | **新增**：算法回归（交叉数/静止性/可复现性/层高对齐/可见集坐标完整性/边界） |
| `_cdp_eval.cjs` / `_cdp_shot.cjs` | CDP 求值与截图工具（真实应用端到端验证） |

**滑条变更**：`最小间距 / 引力 / 最大间距`（力导向参数，换布局后失去意义）→ `可见深度 / 层间距 / 同层间距 / 形态`（分层·径向·自动）+「重排」按钮。

### 9.2 实测验证（真实应用 + 真实数据）

用 CDP 驱动真实 WebView2 应用（`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=9223`）跑出的结果，不是离线估算：

| 验证项 | 结果 |
|---|---|
| 1500 节点加载 → 布局 | 4.5 ms，`layered`（深度 3 裁剪后 44 可见） |
| 1500 节点全展开 → 布局 | 1.99 ms，`radial`，交叉数 0 |
| **静止性** | `settle()` 后 1.2 秒内 **0 个节点移动**（改造前同尺度是每帧几十到几万像素） |
| **可复现性** | 重排两次坐标完全相同（改造前含 `Math.random`，每次不同） |
| **可见性不变量** | `relayoutTrace.visSize == targetSize` 全部成立；`visProbe.shownKeptCount == keptCount`（44/44） |
| 深度切换 d=2→3→4→99→2 | 可见数 10→36→74→80→10，每次 `hideNodes`/`showNodes` 增量正确 |
| 帧率（44 节点 / 1500 节点全展开） | 均 **163.9 fps**（= 165Hz 垂直同步上限）→ 渲染侧不是瓶颈 |

### 9.3 实施中踩到的四个真 bug（都已修，写进代码注释防回退）

这些都是"照着方案写"不会暴露、只有端到端跑真实数据才会现形的问题：

1. **径向布局整体塌陷**（现象：包围盒只有 188px）
   根因：深度数组只在"节点有子节点"的分支里给子节点赋值，而该分支对叶子是 `continue` —— **叶节点深度永远是 -1**，兜底成 0 后被放到圆心。
   修法：深度改为独立 BFS 预计算。
   教训：把两件事塞进一个遍历（叶子数归并 + 深度传播）时，要检查每个分支是否都覆盖。

2. **`snap.manifest.root` 是工作区路径，不是节点 id**（现象：根节点被隐藏，图上只剩 1 个点）
   实测值就是 `"D:\\TA"` / `"G:\\perf1500"`。一直当节点 id 用，于是真正的根（无父节点者，如 `知识库索引`）在深度表里拿不到值，被深度裁剪排除。
   修法：新增 `rootNodeId()`：优先取 `manifest.root` 若它确实是节点 id 且不是任何边的 child 端；否则取第一个无父节点者。

3. **`positions` 用紧凑索引返回，调用方用原索引用**（现象：36 个可见节点只显示 14 个）
   模块内部按"可见节点紧凑顺序"建数组，调用方却拿 `snap.nodes` 的原索引去取坐标 —— 索引错位，部分可见节点拿到 `undefined`，在渲染层被当作"不该显示"隐藏。
   修法：`positions`/`sizes` 改为**按 `snapshot.nodes` 原索引对齐**（被裁掉的为 `undefined`），并在回归脚本里加断言"可见集必须全部拿到坐标"。

4. **`hiddenNodes` 用增量而非完整期望集**（现象：从"全部展开"跳到更浅深度时根节点被误藏）
   `hiddenNodes = new Set(hideNodes)` 里 `hideNodes` 只是"本帧新隐藏的"，于是"该藏的没藏、该露的被漏"。
   修法：改为完整期望集 `nextHidden`（所有不在 target 的节点）。

另外两处"方案与直觉不符、以实测为准"的修正：

5. **形态回退判据从"宽高比"改成"fit 后可读性"**：起初用"宽高比 > 8 就回退径向"，结果 80 节点真实图（3399×384，宽高比 8.85）被误判回退——而它恰是分层的理想场景（fit 后节点 11.8px）。扁平本身不是问题，"fit 完看不见"才是问题。
6. **安全阀必须守最小缩放**：起初无条件把世界压进 `maxRadius`，结果 12000px 宽的分层树被压成 6000px 半径（缩放 0.175），层间距 96px → **17px**，形态直接被毁。改为守住 `MIN_SAFETY_SCALE=0.35`，宁可让极宽世界溢出（交给形态回退与深度裁剪处理）。

### 9.4 已知取舍与后续

- **大图全展开仍会成雾**（1500 节点全展开 fit 后节点 1.7px）：这是屏幕像素的物理下限，不是布局缺陷。产品语义由**可见深度裁剪**承担（默认 3 层 / 自动），"全图总览"作为明确的降级模式存在。
- **标签密度**：径向布局最外环叶子密集时标签会挨得近。后续可加"标签密度策略"（按环内相邻节点弧长决定是否显示标签），当前靠 `min-zoomed-font-size` + 深度裁剪兜底。
- **折叠角标**：`childCount` 已在布局结果里备好（每个节点的直接子节点数），可据此做"点击节点展开/收起其子树"的交互，是下一步的自然延伸。
- **多父 DAG / 交叉引用**：当前数据全是树。若未来出现，按 §4bis.3 用 d3 模型重写 local-force 兜底，而不是继续调现有参数。


## 10. 显示效果（标签遮挡）—— v3.1 追加

### 10.1 问题与根因

用户反馈："节点一多，自动布局之后文字把节点全部挡住了。"

根因不是字号，而是**标签块太大 + 每条都画**：

| 项 | 原值 | 后果 |
|---|---|---|
| `font-size` | 11px | 本身没问题 |
| `text-max-width` | **150px** | 一行能塞 20 个汉字，视觉上是"一块板" |
| `text-wrap` | wrap | 长标题撑到 3–4 行，最高可达 80px |
| 显示策略 | **全部显示** | 只受 cytoscape 的 `min-zoomed-font-size` 一刀切 |
| `min-zoomed-font-size` | 按**总节点数**分档（6/8/10/12） | 一张 1500 节点的图只展开 44 个节点时，阈值仍是 10 → 标签被压到 zoom>0.9 才出现，**与实际可读性无关** |

一个 26px 的圆点配一块最宽 150px 的标签板，节点一多必然互相盖住、也盖住节点本身。

### 10.2 解法：屏幕空间标签装箱（v3.3 已删除；代码见 git 历史 `6971211`）

> 该方案在 v3.3 被整体删除——用户决定"图谱上不显示名称"。下面的记录保留给将来需要
> 在图上渲染动态文字时参考（尤其是 10.3 的三个坑，与"显示什么"无关、是渲染器行为）。

不再"画所有标签"，而是**在屏幕空间里挑选画哪些**，三条硬约束按优先级：

1. **不许盖住任何节点**（除自己的锚点）—— 这是"文字盖住节点"的直接对策
2. **标签之间不许重叠**（留 3px 呼吸）
3. **标签总覆盖面积 ≤ 视口 14%** —— 避免整屏是字

满足约束的前提下按重要性贪心放入：**度 → 深度 → id**（枢纽节点优先被标出，稳定可复现）。
纯函数、可离线回归（当时的 `tools/label_verify.cjs`）。

配套的几何压缩：`max-width 150→72`（≈7 汉字/行）、最多 2 行（超出加省略号）、下移 8→5。

### 10.3 三个必须一起做对的细节（否则约束形同虚设）

1. **标签必须"屏幕恒字号"**：cytoscape 的 `font-size` 是世界单位、会被 zoom 缩放。
   固定 11px 会导致缩到总览时字糊到不可读（实测 fit 后 zoom≈0.35 → 屏幕 3.9px）。
   改为按 `1/zoom` 反算世界字号（上限 4 倍），标签在屏幕上恒为 11px —— 与 Obsidian 一致。
2. **取景必须"与标签无关"**：cytoscape 的 `fit()` 走元素渲染包围盒，而标签属于渲染包围盒，
   于是"标签显示 → 包围盒变大 → zoom 变小 → 标签隐藏 → 包围盒变小"构成**振荡回路**。
   改为显式用 `nodes.boundingBox()` + 34px 余量自己算 zoom 与 pan，标签预算因此是 zoom 的纯函数。
3. **口径必须唯一**：把 `min-zoomed-font-size` 设为 0，标签可见性完全交给预算。
   否则它按"总节点数"档位闷掉标签（见 10.1 表最后一行），与预算统计打架。

### 10.4 实测（真实应用 + 真实数据）

| 场景 | 结果 |
|---|---|
| 1500 节点全展开（zoom 0.07） | 候选 1500 → **1451 个因"会盖住节点"被拒**，只留 7 个 |
| 80 节点全展开（zoom 0.64） | 80 候选 → 61 个因盖节点被拒、6 个因压字被拒，显示 13 个，覆盖率 **4.0%** |
| 放大到 zoom 1.0 | 显示 24 个，覆盖率 7.3%（放大后自然放得下更多） |
| **画面/预算一致性** | zoom 0.4/0.55/0.73/1.0/1.6 五档，`renderedLabels == stats.accepted` 全部成立 |
| 屏幕字号 | 各档实测屏幕字号恒为 11.0px（世界字号 31.3→5.5 自动反算） |

### 10.5 另一个被量化纠正的决定：形态选择改成"两选一比"

早前 `preferLayered()` 是**单边判据**（只估分层，不行才回退径向）。实测被推翻：

| 场景 | 分层 fit 后节点 | 径向 fit 后节点 | 该选 |
|---|---|---|---|
| 1500 节点图只展开 2 层（44 节点） | 2304×192 → **9.4px** | 752×753 → **19.9px** | **径向**（2.1 倍） |
| `D:\TA` 全展开（80 节点） | 3399×384 → 11.8px | 775×859 → 6.0px | 分层 |
| `G:\ta` 全展开（74 节点） | 1955×1056 → 19.2px | 824×1497 → 13.6px | 分层 |

改用 `chooseLayoutMode()`：**两种形态的尺寸都估出来、比 fit 后可读性、选大的**。
径向尺寸用与 `radial()` 同一套弧长递推估最大半径（≈2×maxRadius 即包围盒边长）。

### 10.6 顺带修的观感问题

- **图例遮挡自动收起**：图例是 DOM 覆盖层，会盖掉图的左侧。判据从"有多少节点落在图例矩形内"
  （漏判——图例常从节点缝隙穿过，压在连线上）改为**图例与图的包围盒面积比 > 12% **就收起。
  用户手动点过图例开关后不再自动干预（存 localStorage）。
- **图例文案**同步 v3.0 行为（"拖动松手 = 钉住"、"布局 = 树结构确定性排版"、"可见深度"）。

---

## 8. 复现命令

```powershell
# ★ 算法回归（生产模块 + 真实数据；架构不变量都在这里守卫）
node tools\layout_verify.cjs "G:\perf1500" "D:\TA" "G:\ta"
#   断言：交叉数=0 / 同输入同输出 / 层高对齐且层间距未变形 / 可见节点全部拿到坐标 / 边界不崩

# 图结构表征（节点/边/分量/深度/度分布/多父/环）
node _scratch\graph_stats.cjs   "G:\perf1500" "D:\TA" "G:\ta" "G:\engram"

# A/B 布局质量对比（改造前力导向 vs 树布局：交叉数/包围盒/间距违例/残余抖动）
node _scratch\layout_ab.cjs     "G:\perf1500"
node _scratch\layout_ab.cjs     "D:\TA"

# 爆炸机理拆解（初始半径/力预算/视口换算/分步耗时）
node _scratch\layout_detail.cjs "G:\perf1500"

# 径向 vs 分层 + 渐进披露可读性
node _scratch\layout_radial.cjs "G:\perf1500"
node _scratch\layout_radial.cjs "G:\ta"

# 生成对比图（SVG → PNG）
node _scratch\make_layout_svg.cjs "G:\perf1500" > _shots\layout-compare.svg
& "C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe" --headless=new --disable-gpu `
  --window-size=1680,1040 --screenshot="$PWD\_shots\layout-compare.png" `
  "file:///$($PWD.Path -replace '\\','/')/_shots/layout-compare.svg"
```

### 端到端验证（真实应用，需要 cd 到仓库根）

```powershell
# 1) 起 vite（后台）
npm.cmd run dev

# 2) 起应用并打开 CDP 端口（WebView2 支持）
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=9223"
& "G:\test1.x\target\debug\app.exe"

# 3) 用 Node 22（内置 WebSocket；系统 Node 20 没有）
$node22 = "G:\nodejs22\node-v22.19.0-win-x64\node.exe"

# 读布局状态（静止性 / 可复现性 / 可见性不变量 / 标签一致性）
& $node22 _cdp_eval.cjs "(async () => { const D = window.__engramDebug; await D.settle(); return JSON.stringify({ visible: D.visibleCount, hidden: D.hiddenCount, info: D.layout.info, labels: D.labels, trace: D.relayoutTrace }); })()"
# labels.renderedLabels 必须等于 labels.stats.accepted（画面与预算一致）；两者不等说明标签类没同步干净

# 截图
& $node22 _cdp_shot.cjs "_shots/foo.png" 1600 900
```

> 注：应用有单实例锁，若 `app.exe` 起不来先 `Get-Process app | Stop-Process -Force` 清掉旧进程。

