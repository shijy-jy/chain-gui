# CHANGELOG

本文件遵循 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.0.0/) 格式。MCP 工具契约变更必须在此显式记录（ADR 0008 配套）。

## [3.2.0] - 2026-10-04

### 记忆层第一阶段：源输入建立节点 S1–S4（设计稿 docs/Engram_记忆层第一阶段_源输入建立节点_设计_v1.md）

用户方向：写节点前多遍关注同一段源输入（要点/残余/结构），列出全部候选方向+权重，按分布形状决定建几个节点；方向未定的细节做伏笔登记；保留式抽取必须种子可重放。

- **S1 决策痕迹扩展**（`dialogue_log.rs`）：decision 记录新增加性字段 `mode`（commit/sample）、`seed`、`selected`、`candidates[]`（{dir, score}）、`foreshadowing[]`（{covers, note}）。旧记录零变化（读保守：未知键忽略、缺省 None/空）。
- **S2 伏笔登记**：`decided` 新增 `foreshadow`（"注意到但方向未定"的潜在痕迹）——**不进图谱、不进事实源，只活在痕迹里**；不得携带 commits/nodes/candidates（REMEMBER_FORESHADOW_NO_COMMITS）。`dialogue_status` 新增 `foreshadowing` 聚合视图（take 50）与 `sample_decisions` 统计；decisions 计数扩为 keep/skip/revise/foreshadow。
- **S3 多峰建节点**：commit 模式 + candidates → 顶层 commits 每个方向各建一个节点；新建节点 `origin` 自动带方向序号（`dialogue/log.jsonl#<seq>.<idx>`，`parse_provenance` 兼容解析回 seq）；响应 created[] 附 `dir`。candidates 全量留痕——将来发现方向选错，替代方案就在痕迹里。
- **S4 保留式抽取**：`mode:"sample"` + `seed` + `candidates[].commits`（每个候选自带"若抽中要执行的完整意图"）。工具按种子做**确定性加权抽取**（FNV-1a → xorshift64，std-only 无随机源）——同种子同结果，复盘可重放；只执行抽中方向的 commits，未抽中方向留在痕迹。AI 声明 `selected` 与工具抽取不一致 → REMEMBER_SELECTED_MISMATCH 拒绝（防不可重放）。
- **校验**（宁拒绝，不污染）：sample 模式要求 seed 非空、candidates 非空、score 有限且 >0、selected ∈ candidates；commit 模式下 candidates 携带 commits 拒绝（REMEMBER_CANDIDATE_COMMITS_ONLY_SAMPLE）。
- **指南 v22 / v16**：新增「记忆层第一阶段：三遍关注、伏笔登记与保留式抽取」章节（分析模式附录 + 开发模式 §8.7）；自适应成本原则（默认单遍 argmax，自检矛盾/重复逼近阈值/源段多义才开多假设）。
- **契约 v9 → v10**：remember 参数新增 mode/seed/selected/candidates/foreshadowing；golden 28 → 34 条（+伏笔登记/多峰/抽样/一致性拒绝/dialogue_status×2）；core 测试 230 → 242（对话账本 +12）。
- 记忆层两个已知缺陷（强度 clamp 归零、墙钟恒空）仍按设计稿归入重构后专项，不在本次范围。

## [3.1.0] - 2026-10-03

### AI 导航增强（契约 v9）：让"顺着节点链梳理"少走弯路

用户视角确认了核心机制：**AI 靠结构化节点反向回忆——搜索锚定 → 沿 parent/edges 上下梳理 → 读得越多越知道下一步去哪**。该模型成立且是 Engram 的生态位；本轮的改进不是换模型，而是**让每一次读都自带"下一步去哪"的信息**，减少跳数：

- **检索结果附结构上下文**：`search` / `recall`（向量、冷启动、关键词降级、归档三条路径全部）每条结果增加 `parent / depth / children_count / degree / origin / updated` —— 一次检索即可判断"该读哪条、跳过哪条"，不必逐条 `read_node` 补位置。
- **`get_overview` 增 `structure` 块**：`roots`（根数）/ `max_depth` / `leaves` / `depth_hist`（深度直方图）+ 每个入口 hub 的 `subtree_size` —— 把"人眼从 3D 图看出的结构"翻译成 AI 可读的数据。
- **`dialogue_status` 增记忆健康度出口 + 覆盖度**：`gaps`（recall 未命中的查询 = 用户找过但没找到 → 该补节点了）、`open_loops`（已 success 但无验证子节点且无「自验收」注明的 task），以及 `coverage {total / read / unread / unread_ids[]}`（`stats.reads_map()` 驱动）——补上记忆系统自我修复的闭环，并让 AI 能规划"跨链随机、每次不重复"的读取路线。
- **`expand` 增 `direction`（children / parents / both，默认 both）**：children = 自根向下层级优先（还原一个项目时的读法），parents = 回溯来源；每个节点附 `hop`（相对中心的层距）与 `first_line`（正文首个内容行的 **120 字节机械截断**，不是摘要）。
- **`get_overview` 增 `structure.root_ids`**：parent=null 的入口节点 id（按子树规模倒序，≤20）——"层级优先"策略需要的是**入口 id**，而不是根的数量（`structure.roots` 只是计数）。
- **两条读取策略由 AI 自己规划，软件不预设导读路径**（用户 2026-10-03 决定）：① 还原分析模式项目 = 从 `structure.root_ids` 起 `expand(direction:"children")` 逐级向下，信息量逐级指数增长，先看骨架再决定读哪条原文；② 开发自己项目 = 用 `coverage.unread_ids` 抽未读节点随机不重复地读，读到即覆盖。**同时撤销两个被否的设计**：固定"导读路径（tour）"（把自适应遍历降级成一次性导览，且"读原文"被"读摘要"替换）与节点 `summary`/gist 字段（**读节点正文就是放弃读原文**，再次整理节点内容只会丢信息；只允许机械截断）。
- **真实工作区实测**（release 二进制 + MCP stdio，只读工具）：`G:\ta`（分析模式 74 节点 / 深度 11）`get_overview` 666ms、`expand(root,2,children)` 32 节点 28ms/15.4KB、`dialogue_status` 26ms 且 `coverage` 报出 15 个未读节点；`G:\perf1500`（1500 节点）冷启动首次扫描 13.5s / 随后各调用 0.36–0.42s，`expand` 44 节点 19.3KB、`unread_ids` 按 30 条封顶——证明"层级优先 / 跨链随机不重复"两种策略在真实数据上都有可直接使用的字段。
- 契约 **v8 → v9**；指南 **v21 / v15**（§ 导航上下文改写为两条读取策略 + `expand.direction` 语义 + `degree` 同源说明）；golden 28 条重固化；file 136 / core 95 / cli 13 / golden 全绿。
- 分析文档：`docs/Engram_图结构AI可用性分析_v1.md`（工具现状、痛点排序、P0/P1/P2 清单）。

### P2 结构性收尾：人机同源指标 + 分析模式层级布局 + 覆盖遮挡 + 冷启动并行

- **结构指标入接口（P2-7 人机同源）**：`degree` 进入 `structure_index`，于是 `search` / `recall` / `read_node` / `expand` 的每条结果都带 **度数**——它同时是显示层球径的依据。新增 `ops::snapshot_view(snap)`：给 GUI 快照的每个活跃节点补上 `degree / depth / children_count / subtree_size`（`scan_chain`、`chain-changed` 事件、`attach_code_map`、`detach_code_map` 四条路径统一走它）。**显示层不再自己重算结构指标**（删掉了前端按边遍历算 degree 的实现），信息栏新增「度 / 深 / 子 / 子树」一行——人看到的球多大 = AI 读到的 `degree`，两边同一个数、同一处定义。
- **分析模式层级球壳布局（P2-8）**：新增「布局」选择（自动 / 层级球壳 / 神经元，偏好持久化）。**自动 = 按工作区模式**：分析模式用**层级球壳**（半径 = depth × 层距，同层同壳、锥角随深度收窄、首次到达定层与后端 BFS 同算法；刻意不做力导向松弛，层壳不被揉散），开发模式保持三维神经元展开。
- **覆盖遮挡（P2-9）**：图例默认折叠成一行标题（点标题展开，偏好持久化）；`measureInsets` 量出图例/搜索框/缩放控件的贴边尺寸（单方向封顶 32%），`centerAll` 把内容居中到**未被压住的自由区**并按自由区尺寸放大距离——浮层不再盖住节点。
- **冷启动扫描并行化（P2-10）**：文件层新增 `scan_pool`（`std::thread::scope` + `AtomicUsize` 抢索引 + `catch_unwind`，零新依赖），扫描按文件并行、**按文件序回填**，输出与串行逐字节一致（golden 依赖的确定性不变）；条目 <8、单核、线程创建失败或任一 worker panic 一律回落顺序路径。
  - 实测（16 核；`G:\perf1500` 1500 节点）：端到端 `get_overview` 437ms / `expand` 259ms / `search` 299ms / `dialogue_status` 249ms，较改造前（419 / 366 / 378 / 361ms）**快 25–30%**；扫描本身 356→235ms（**1.5×**）；等 I/O 型负载探针 336→23ms（**14×**）。
  - 诚实说明：文档里那个 **13.5s 冷启动基线无法复测**——非管理员清不掉系统 page cache（需要 `SeProfileSingleProcessPrivilege`），上面全是热缓存数字。冷启动的成本恰好是"每文件约 9ms 的磁盘 I/O 等待"，而并行池对 I/O 等待的重叠正是收益最大的场景（I/O 型探针 14× 即为此形状），但没有直接测到。
  - file 测试 135 → **141**（新增保序/等价/回落/panic 安全 6 条，另 2 条 opt-in 基准）；`--release --ignored` 用 `G:\perf1500` 真实 1500 文件跑并行 vs 串行等价：dev 与 analysis 两模式**逐字段一致且同序**。

### 三维图结构（显示层 · 纯图结构，去掉水波纹）

- **新组件 `Graph3D.svelte`**：three.js（0.186）WebGL 渲染——InstancedMesh 球节点（类型配色 + 状态样式：failed 红 / in_progress 加亮 / pending 半透明 / blocked 压暗；代码节点青绿线框描边；选中放大；代码筛选压暗非代码节点）+ LineSegments 边（contains 实线、solves/alternative 虚线，源→目标类型色渐变）。布局复用 `tree_layout`（layered 层板 / radial 锥形，层级沿 Z 轴抬升；可见深度裁剪一致）。
- **Unity 式交互**：右键拖拽旋转视角 · 中键拖拽平移 · 滚轮缩放 · 左键点击选中（联动只读信息栏）· 左键双击相机聚焦；OrbitControls damping + 相机补间动画（聚焦/复位/缩放/搜索脉冲）；悬停浮层沿用现有 hoverTip。
- **水波纹彻底退役**：水面画布隐藏、波源/涟漪循环空实现、波纹参数面板隐藏、图例改为 3D 交互说明——纯图结构。
- **策略说明**：旧 2D cytoscape 机械以隐藏方式保留（影子替换，`__engramDebug` 契约不变、16 个 CDP 脚本不受影响），彻底删除留作后续清理项。
- 验证：svelte-check 0 错 0 警 + vite build；CDP 实跑——WebGL 画布挂载、波纹文案零命中、Unity 交互提示在位、点击选中/双击聚焦/滚轮缩放/右键旋转四类交互零异常（选中后信息栏正确显示 v-015/t-016 面板）。
- **截图目视复核与两轮修复**（新增 `_cdp_shot2.cjs`：`Page.captureScreenshot { fromSurface:false }` 走渲染器侧，绕开本环境虚拟桌面合成器无帧导致的空白截图）：
  ① **实例色全黑**——`MeshBasicMaterial({ vertexColors:true })` 让 shader 去取几何体 color 属性（球体没有）→ 全黑；InstancedMesh 的实例色**不需要** vertexColors（three 自动启用 USE_INSTANCING_COLOR）；
  ② **比例失调**——位置经 span 归一化而球径是固定场景单位，导致小图球巨大／大图球极小；改为**布局 px 直接映射（UNIT=0.1）+ 球径同系数**，相机由 `centerAll()` 按包围盒对角线 ×0.55 自适应（首次/换工作区自动 fit，滑条调整保留视角）。修复后目视确认：类型配色清晰（根紫大球 / design 蓝 / task 青 / verification 绿）、边线渐变、远端透视收缩、无水面效果。
- **三维神经元布局（v3.1 续调）**：**放弃二维树形态**（layered 层板 / radial 环）——改为纯三维展开：根 = 胞体（单根时子树在整球面按 Fibonacci 分布），每个节点占据一个锥形区域，子节点在父锥角内按黄金角螺旋分布 → 树突状辐射；半径 = 深度 × 层距；确定性哈希微扰（`fnv1a(id#salt)`）让枝条自然不呆板且**可复现（无随机种子）**。「形态」滑条重定义为**展开度**：标准 1.05 / 紧凑 0.8 / 舒展 1.35。球径与层距比例重调（`px×0.45×UNIT`）、层距 ×0.95 拉大、边线透明度提到 0.7/0.75（树突连线是神经元观感的关键）。
- 目视验证（`_cdp_shot2.cjs` 渲染器侧截图）：SDF 工作区 104 节点呈胞体+树突辐射，类型配色可辨（根紫 / design 蓝 / task 青 / verification 绿）、连线网络清晰、三维纵深明显。
- **三维优化第二轮（开源参考驱动）**：调研 [d3-force-3d](https://app.unpkg.com/d3-force-3d@3.0.5/files/README.md)（3D 力导向布局，velocity Verlet）、[vasturiano/3d-force-graph](https://github.com/vasturiano/3d-force-graph)（6.4k★ 的 ThreeJS 3D 图组件）、[AntV G6 d3-force3d](http://g6.antv.antgroup.com/en/manual/layout/build-in/d3-force3-d-layout)，据此落地：
  ① **3D 力导向松弛**（d3-force-3d，作为确定性神经元布局的种子后处理：forceLink 距离=层距×0.85/强度 0.45 + forceManyBody 3D 斥力 + forceCenter，固定 tick 数 → 枝条张开不重叠且可复现）；
  ② **邻域高亮**：hover/选中节点时其 1 跳邻居保持原色、其余压暗至 22%，关联边原位改色（不重建几何）— 复刻原 2D"聚焦压暗"语义；
  ③ **突触脉冲**：沿边流动的粒子（Points + AdditiveBlending，借鉴 3d-force-graph `linkDirectionalParticles`）——选中节点时只沿它的边流动，全图上限 260 条；
  ④ **Bloom 光晕**：EffectComposer + UnrealBloomPass（strength 0.38 / radius 0.45 / threshold 0.4，仅 ≤1200 节点启用，大图自动降级）。
  目视验证（`_shots/3d-final2.png`）：胞体高亮、树突张开、脉冲流动、发光适度且类型配色可辨。
- **2D cytoscape 机械彻底拆除**：`App.svelte` 2767 → **1287 行**；删 `tree_layout.ts`(556) / `ripple.ts`(89) / `chain_to_cytoscape.ts`(133)；新增 `node_style.ts`（色表 + LayoutMode）；卸载 cytoscape / cytoscape-dagre / @types-cytoscape（cytoscape 仍作为 **mermaid 的传递依赖**留在 lock）；`__engramDebug` 精简为 `{snapshot, mode, layout}`；src/ 残留搜索零命中。

### 三层重构 P0/P1：对话账本与记忆入口（设计稿 docs/Engram_三层重构_设计整理_v1.md）

用户方向：文件层（事实源）/ 记忆层（核心逻辑）/ 显示交互层三层架构，接口隔离；对话文件是唯一原始输入，节点是"听完讲解后整理好的脉络"；AI 使用记忆系统的唯一通道是 MCP。

- **文件层 `dialogue.rs`（新）**：对话原始输入的纯字节接口——原样读、append-only 追加一行、路径与命名校验（防穿越）。**不认识格式**（无 k/seq/role 概念）。
- **记忆层 `dialogue_log.rs`（新）**：对话账本格式的唯一定义处——JSONL（head/msg/tool/decision）；`seq` 工作区级单调、全记录唯一；`decision.covers=[from,to]` 覆盖消息 seq 区间（消费锚点 + 审计证据）；长消息超 4096 字节按 UTF-8 边界拆行（同 seq + part/parts，拼回逐字一致）；坏行隔离报行号；**decision 空 reason 拒绝**（"有意跳过"与"忘了记"的唯一区分）。
- **`remember` 工具（新，契约 v6 核心）**：记忆唯一入口——追加账本事件（msg/tool/decision）+ 可选 commits 落节点意图（create/update/link/unlink/archive，全部复用现有写路径 → 守门/乐观锁/原子写/审计一个不少）。结构违规阻断（REMEMBER_* 错误码）；规矩违规只标记不阻断（frontmatter `conventions: [missing_trigger]`）。新建节点自动写 `origin: dialogue/log.jsonl#<seq>` 溯源。有 commits 必须有事件（REMEMBER_NO_EVENT）。
- **`dialogue_status` 工具（新，只读）**：账本规模 / 会话与指南版本 / 消费进度（`unconsumed_from` 之后的记录是接管要读的部分）/ 决策计数 / 坏行清单。
- **所有工具响应携带 `guide_version`**：指南版本变化当次可见，AI 自主决定重读（不强制）。
- **Node 新增 `origin` / `conventions` 可选字段**（serde 缺省省略 → 存量节点输出零变化）；walker 开发模式宽松提取。
- **指南 v15 / v9**：新增「对话账本与记忆入口（唯一入口）」章节；remember 取代 create/update/link 的渐进语义。
- **工具契约 v5 → v6**（新增 2 工具 + 全响应注入 guide_version）；golden 契约 18 → 23 条重固化；采集器新增 UTF-8 干净的 Node 版 `tools/_collect_golden.mjs`（PS 版偶发 stdin 超时，逻辑等价保留）。
- **工具契约 v6 → v7（入口唯一化收尾）**：**移除** `create_node` / `update_node` / `link_nodes` / `archive_node` / `unlink_nodes` 五个节点直写工具——记忆写入只剩 `remember` 一个入口（设计稿 §I2）。golden 重写为纯 remember 流程（23 → 24 条：结构违规阻断、update/unlink/archive 意图、账本消费与溯源全走新通道）；指南 v16 / v10（旧工具标注"已移除"）。
- **工具契约 v7 → v8（P2 冻结自愈）**：新增 `resolve_conflict(id, title, status, body?, expected_updated?)`——人治通道下线后 CONFLICT 的裁决出口：冻结节点（[待裁决]）由 AI 读双方内容后一次写回最终裁决、去除冻结标记；非冻结节点报 NOT_FROZEN；expected 不符报 CONFLICT 且不再冻结；audit 留痕（unfreeze）。golden 24 → 28 条（完整冲突→冻结→自愈循环）；指南 v17 / v11。
- **core 测试 203 → 230**（新增对话 17 条 + remember 8 条 + resolve_conflict 2 条）；golden 28 条全绿。
- **GUI 人治写通道移除（P2 收尾，ADR 0015）**：Tauri 命令 update_node / create_node / delete_node / set_parent / fold_chain / create_node_human / delete_node_human / set_parent_human 全部删除；core `node_edit` 三个 `*_human` 函数与人用护栏删除；信息栏编辑表单与文件树模式三态操作面（＋/✎/🗑）删除，文件树降为**只读阅读面**；GUI 保留只读 + 维护通道（reindex / code_map / 快照 / 过程日志 / 工作区管理）。core 230 → 224（移除 6 条人用通道测试）。这是对 ADR 0012（GUI 零破坏）的显式豁免（用户 2026-10-03 拍板）。
- **CDP 实跑验证（debug app + vite dev 源码直载）**：零控制台错误；工具栏「＋ 节点」已消失；文件树模式只余阅读功能（树/检索/下一篇/原文/定位，「新建/编辑/删除」字样全 DOM 零命中）；信息栏打开节点后**无编辑表单**，检索线索 / 触发句 / 代码骨架（unity 94 导出）正常渲染；svelte-check 0 错 0 警 + vite build 通过。窗口截图受本环境虚拟桌面 15×15 限制，最终视觉复核留交互会话。
- **P3 交接验收**：① legacy 祖父条款——无 `origin` 字段的存量节点在扫描期隐式标记 `origin: legacy`（`walker::mark_legacy`，**绝不回写文件**，有单测），指南 v18/v12 写入"只读不改、不得作为新记忆扩写基础"；② 对话阅读面——新增 Tauri 命令 `get_dialogue`（记忆层 dialogue_log 的结构化账本）+ 前端 `DialogueReader.svelte`（工具栏 💬 按钮；会话分组 / 全文检索 / Markdown 渲染 / 决策与工具轨迹徽标 / 未消费标记 / Esc 关闭；**渲染期投影，不落盘第二格式**）。CDP 实跑验证：打开/关闭正常、零写控件、空账本提示正确。
- **P4 分层强制**：文件层拆为独立 crate **`engram-file`**（model / scanner / fsio（原子写/宽松解析/变更哈希）/ schema / migrate / watch / workspace / evidence / profile / guide / audit / dialogue / chain_ops / node_edit；94 测试）——**编译期不依赖记忆层**（"文件层不认识记忆"由 Cargo 依赖图强制）；`engram-core` 保留记忆层 + API 层（API 边界 = `ops` 模块，JSON 适配集中），旧路径经 re-export 全兼容；api 提升为独立 crate 留作机械化后续。磁盘整理：清空 `target/debug`（31.5 GB，G 盘曾满导致链接失败——构建产物可重建）。
- 记忆层两个已知缺陷（强度 clamp 归零 / 墙钟恒空）按设计稿 §14 归入重构后专项，不在本次范围。

## [2.18.0] - 2026-09-22

### 文件树模式 = 人的编辑面（新建 / 编辑 / 删除 / 改挂载）；图谱与文件树是同一套链的两种显示与编辑方式

用户要求：给阅读模式加新建与编辑——「新建的文件在文件树下自动在节点图里生成节点挂载在下面」，两模式是同一套系统的两种显示方式和编辑方式（图谱偏 AI 用，文件夹偏人用）。

- **视图更名与定位**：`📖 阅读模式` → **`🗂 文件树模式`**（切换按钮 `🗂 文件树模式 ⇄ ◧ 图谱模式`，Esc 返回）
- **新建（＋ 新建）**：文件树左上与阅读区各有一键新建；表单＝标题 / id（分析模式按 `g-/d-/t-/v-` 前缀自动建议下一个空闲号）/ 类型 / 状态 / 父节点（默认＝当前选中节点，可搜索改挂）/ 关系（contains·solves·alternative）/ 标签 / 正文。创建即写入 `.chain/nodes/<id>.md`，**同一份事实源**：文件树长在所选父节点下面，图谱里同一个父节点下同时长出这个节点（实测图谱节点数 +1、挂载边正确）
- **编辑（✎ 编辑）**：标题 / 状态（分析模式）/ 标签 / 正文（Markdown + LaTeX 大编辑区）/ 父节点与关系；Ctrl+S 保存、Esc 取消；保存走 core 守门（原子写 + rev 乐观锁，并发冲突拒绝而非静默覆盖）。编辑中切节点会被拦住（先保存或取消）——草稿不会静默丢
- **删除（🗑 删除）**：两段式确认；分析模式护栏＝不能删根、不能删还有子节点的节点（避免悬空分支）
- **改挂载**：编辑面里的父节点+关系；分析模式不允许断成根、不允许成环（新父节点在本节点子树里 → 拒绝）
- **后端（分析模式人用通道；MCP 契约零变化）**：`engram-core::ops::node_edit` 新增 `create_node_human` / `delete_node_human` / `set_parent_human`（与 MCP 版共用内部实现，`human=false` 即原行为＝分析模式一律拒绝）+ 三个同名 Tauri 命令。分析模式人用通道额外做词表校验（类型限 goal/design/task/verification、状态限五态，拒绝 note/none）与结构护栏；MCP 的 `create_node` / `delete_node` / `set_parent` 行为与错误文案不变（golden 契约 18 条照旧全绿）
- **测试**：core 197 → **203**（新增 6 条：新建必须挂已存在父节点、协议词表校验、删根/删有子节点护栏、断根与成环护栏、开发模式默认值不变、MCP 通道仍拒绝分析模式）
- **修一个真 bug**：v2.19 让文件树模式吃 watcher 实时推送后，写入期间 watcher 会推来「写入之前」的扫描结果，把刚写回的 snapshot 覆盖（表现：新建节点的正文/标签闪回旧值）。新增 `markSelfWrite()` 自写窗口（1200ms 内丢弃 watcher payload，窗口外的外部写入照常实时刷新）
- **实测（CDP + 隔离的临时工作区，0 异常 0 控制台错误）**：
  - 开发模式：新建 → 图谱节点数 +1、挂载边正确、树行出现，**磁盘文件 parent/rel/tags/正文逐字核对通过**；编辑（标题/正文/标签）→ rev+1 且磁盘一致；改挂载 A→B → `parent: rp-b` + 边 `rp-b->rp-a`；两段式删除 ✓
  - 分析模式：新建 design（in_progress）挂到根 goal → 图谱 +1、`validation.valid`、磁盘一致；删根 → 「根节点不能删除（链协议要求保留唯一根 goal）」；删还有子节点的节点 → 「该节点还有 1 个子节点（t-001）……链不能出现悬空分支」；把根挂到自己的后代 → 「这会形成环……链协议禁止环」且根 parent 不变；合法改挂 t-001→g-001 ✓
- 说明：本视图仍是**人类专属**——不写工作区额外文件、不注册 MCP 工具、不进 AI 指南副本、不进 `__engramDebug`；AI 侧「分析模式链由 AI 维护」的能力与契约完全未变

## [2.17.0] - 2026-09-21

### 阅读模式：图结构 ↔ 节点文件树（人专用视图；AI 不可识别、不可使用）

用户反馈：**节点一多，用图谱读内容很不方便**——图上只有标题，正文要一个个点侧栏。本轮加一条显示方式切换。

- **切换**：工具栏「📖 阅读模式 / ◧ 图谱视图」一键切换（Esc 退出）；进入后是覆盖全窗口的阅读视图，图谱原样留在下面，退出即原状（ARCHITECTURE §5 加性改动，零破坏）
- **左：节点文件树**——按链协议父子关系（frontmatter `parent`）把图结构梳理成文件树：折叠展开（展开/折叠全部）、类型色点、id、状态字形（✓/◐/✕/⛔/○）、归档/待裁决/蒸馏/代码骨架/未闭环标记；开发模式的环与悬空父节点容错（标「环」「悬」，只读展示不改数据）；「含归档」开关；树宽可拖拽
- **树内检索**：标题 / id / 标签 / **正文命中**（带上下文片段，输入去抖 160ms）——工作区再大也能定位到某一篇
- **右：全文阅读**——渲染（Markdown + KaTeX）/ 原文（文件正文原样）切换、字号三档、复制正文、面包屑（根 → 当前）、子节点与证据文件跳转、代码骨架 ⧉ 全屏页入口；底部「上一篇 / 下一篇」沿文件树前序把整库当一本书读（顶部显示 n / 总数）
- **人专用不变量**（刻意设计，写死在代码注释里）：不写工作区任何文件、不新增后端命令、不注册 MCP 工具、不进 AI 指南副本（`resources/AI_GUIDE*.md` 无任何描述）、不进 `window.__engramDebug` 调试接缝；视图偏好只落 GUI 本地 localStorage——**AI 既无法识别也无法使用**
- 顺带：阅读模式下工作区文件变化不再被「编辑中不覆盖」挡住，AI/外部写入实时进入文件树；水面涟漪渲染循环在覆盖层下停掉（不空烧 CPU）
- 实现：新增 `src/lib/node_tree.ts`（纯派生构树：确定性排序、环/悬空容错、压平可见行、阅读顺序、检索）+ `src/lib/ReaderMode.svelte`（阅读视图）；`App.svelte` 只加视图状态与切换入口
- 验证：svelte-check 0 错误 0 警告；CDP 驱动真实应用实测（截图 `_shots/read-*.png`）——ta 74 节点 进入阅读模式 21–178ms / 展开全部 34–40ms；perf1500 1500 节点 进入 175–200ms / 展开全部 302–320ms（1500 行 · 7576 DOM）/ 全库正文检索 < 400ms；「在图谱中定位」「原文/渲染」「字号」「上一篇/下一篇」「Esc 退出后图谱原样」逐项通过；运行期 0 异常 0 控制台错误

## [2.16.0] - 2026-09-09

### 支链闭环规则 + 任务开环可视化（用户反馈：分析模式支链走到任务就断了）

- **协议（指南 v14）**：task 置 `success` 前必须挂 `verification` 子节点（v-*）作为验收凭据——success 字面定义即「完成并验证通过」；例外：任务本身即验收/纯资料收集 → 正文注明「自验收：结论」代替
- **图谱可视化**：无验证子节点且无自验收注明的 task 节点 → **琥珀色虚线框「未闭环」标记** + 悬停提示 + 图例说明（存量开环一目了然，随日常维护收敛）
- 实测存量开环：ta 33 / RESTRI 5 / render_unified_oss 32（共 70 个未闭环 task）——新协议只约束新增，存量逐条补 v-* 或自验收注明

## [2.15.0] - 2026-09-09

### 外部 AI 实测反馈三修（检索式接手能力报告）

- **① recall 首次调用挂死修复**：模型加载移出调用线程——共享嵌入器状态机（Loading/Ready/Failed）+ 后台预热（MCP 启动即预热，recall 兜底自暖）；未就绪时**立即**关键词降级（degraded:true + 「加载中/不可用」原因），就绪后自动切回向量。降级承诺真正兑现，任何时刻有响应
- **② MCP 骨架通道**：`read_node` 增 `include_code_map` 参数 → 返回 `code_map_md`（骨架全文）+ `code_map_stale`（陈旧标记，未挂载为 null）——纯 MCP 客户端实现级还原不再被工具封顶
- **③ get_overview 入口推荐**：新增 `entry_hubs` 字段——总索引/总览类节点（标题命中）或连接度 ≥2 的入口候选（评分 = 标题命中×100 + 度数，前 5；纯叶子不进推荐，链式小图退化为按度排序）
- 工具契约 **v4 → v5**（read_node/get_overview 增字段）；指南 v13/v8（MCP 骨架通道入册）；golden 18 条再生成
- 测试：core 197 / cli 13 / golden 18（新增：加载中降级立即返回、骨架通道、入口推荐）

## [2.14.0] - 2026-09-09

### 代码骨架全屏展开页（内容多面板小看不清的解法）

- 代码栏头部新增 **⧉「展开全屏」按钮**：点击打开**覆盖整个窗口的独立页面**（大字体正文 14px/代码 13px + Mermaid 调用图 + 接口清单 + 调用边），整页滚动，Esc / ✕ 关闭
- 实现：`src/lib/CodeViewer.svelte` 全页阅读器（markdown-it 渲染管线 html:false 防注入 + Mermaid 单独渲染）+ App 全屏覆盖层（z-3000 淡入）
- 说明：最初按「独立 Tauri 窗口」实现（WebviewWindowBuilder + initialization_script 传参），本环境 WebView2 运行时窗口导航失败（恒 about:blank，含手动导航）——改为应用内全屏页，体验等价且零窗口管理复杂度；后端 open_code_window 命令已移除
- 验证（CDP + 视觉引擎）：⧉ → 覆盖层渲染 69k 字符骨架 + Mermaid、可滚动、Esc 关闭 ✓；视觉评审 9/10（文字大而清晰、布局干净、无瑕疵）

## [2.13.0] - 2026-09-09

### 连线交叉最小化（图谱布局硬规则）

- **首帧层内质心排序**（chainToElements）：BFS 每层节点按「已就位邻居的角度质心」排布——子节点贴父节点（Sugiyama 式两层归约），树/链结构首帧即近零交叉
- **交叉计数网格分桶**：边中点分桶 + 5×5 邻域（cell=最大边长，不漏对），O(E·k) 替代 O(E²)——1500 边可负担
- **质心交叉归约全规模启用**：收敛后 polishCrossings 对所有规模运行（原仅 ≤200 边）
- **逐帧交叉惩罚分桶化**：上限 200→800 边；超大图靠首帧排序 + 收敛归约
- 实测（perf1500 合成基准）：交叉数 **72,595 → 296（−99.6%）**；视觉评审确认连线基本不交叉、结构树状化；图例新增「布局自动减少连线交叉」说明

## [2.12.0] - 2026-09-09

### 多语言代码内化（C#/C++/HLSL/Unity）+ 代码栏可滚动浏览

**多语言骨架提取（v2.16）**：
- 语言矩阵：rust / csharp / cpp（hlsl·glsl·cuda 同解析器）/ **unity**（.cs 走 C# + shader 族走 C++ 双解析器，Unity 工程一次提取全覆盖）
- 语言自动检测：单文件按扩展名；目录浅层占比 + 零计数深扫兜底；`.cs` 与 shader 族并存自动判 unity
- C# 提取：命名空间/类/结构/接口/枚举 + public/internal/protected 方法/属性 + 调用边；C++ 提取：命名空间/类/结构/枚举 + 非 static 函数 + 调用边
- **跨盘绝对路径挂载**：`code_map` 允许绝对路径（G 盘工作区 ← D 盘 Unity 工程）；CLI `--lang auto|rust|csharp|cpp|hlsl|glsl|cuda|unity`
- tree-sitter 0.22 → 0.23（LanguageFn 统一矩阵）；指南 v11/v6；golden 再生成
- 挂载落地：render_unified_oss 8 节点（d-015..d-021，C++）、RESTRI 2 节点、water 3 节点（unity/csharp/hlsl）、ta 4 节点（SDF shaders 138 exports / windy / windtest / waterrenderer.test）

**代码栏浏览修复**：flex 子项禁止收缩 + `.code-md` 自滚动撑满剩余高度；「⤢/⤡」一键展开/收起（大骨架完整滚动浏览）

## [2.11.0] - 2026-09-09

### 1500 节点承载力 + 性能策略中枢 + UI 扩展接口（视觉引擎驱动的全面优化）

**性能（目标：1500 节点流畅且有余量）**
- 力导向布局空间哈希网格化：斥力+碰撞合一为 3×3 邻胞近似 O(n)（原全对 O(n²)）；n>400 不再跳过布局（按档迭代 120/80 代铺开）；拖拽标记 Int8Array O(1)；>800 节点跳过跨分量上限（中心引力防漂移）
- 边渐变真降级：>300 边全部实线（原注释承诺未实现；canvas 渐变纹理是大图平移缩放的大头）
- 标签按 zoom 裁剪：`min-zoomed-font-size` 内建裁剪 + 按节点数分档（400/800/1400 四档阈值 6/8/10/12）
- 快照签名 $effect 拆分：滑条独立 effect，fnv1a 数字哈希替代 1500 节点 sort+join 巨型字符串；tags/evidence 退出签名
- 涟漪只遍历层内节点（byDepth）、增量点亮本层、深度按规模分档（3~6）、波源半径 O(1) 包围盒近似；对比滑条 7 次样式 update → 链式单次
- 位置缓存扁平化 Float64Array（平移/缩放期间零分配）；chainToElements BFS 头指针 + Set + nodeById（加载时长）
- 父节点选择改搜索式输入（Sidebar/新建对话框：1500 个 `<option>` → 搜索 + 限 20 条）

**架构（UI 逻辑清晰 + 未来扩展预留）**
- 新增 `src/lib/ui/perf.ts` 性能策略中枢：PERF_TIERS 四档（s/m/l/xl，按节点数）集中全部降级阈值 + rAF 帧监控 + fnv1a
- 工具栏注册表化：`toolButtons` 描述符数组驱动渲染，加按钮 = 追加一条
- 图例颜色单一来源（NODE_TYPE_COLOR 派生，消除 3 处重复色表）
- 新增性能浮层（⚡ 工具栏开关）：实时 FPS/帧耗时/规模分档
- 调试钩子 `window.__engramDebug`（cy/snapshot/mode 只读）：CDP 自动化验证与未来扩展的稳定接缝
- 新组件：`src/components/PerfOverlay.svelte`

**验证**：svelte-check 0 错误；CDP 驱动的 FPS 基线/复测 + 视觉引擎截图评审（1500 节点合成工作区）

## [2.10.2] - 2026-09-09

### 图谱代码挂载可视化（代码骨架一眼可见）

- 图谱节点：挂载 `code_map` 的节点青绿描边 + 标签尾缀 `</>` 角标；悬停浮层标注「代码骨架」
- 工具栏新增「代码」筛选开关（`</>` 按钮，激活青绿）：一键高亮所有已挂载节点（辉光），其余压暗；图谱重建/数据原位更新后自动重放
- 图例新增「已挂载代码骨架」说明；选中已挂载节点自动展开信息栏「代码」栏（手动收起后不打扰）
- GUI 增性，MCP/CLI 工具契约不变（v4）

## [2.10.1] - 2026-09-09

### 代码骨架检索集成 + 检索线索面板进安装包（M-Code 收尾）

- **检索集成兑现**（指南 v10 附「检索语义」）：code_map 节点的检索文本 = title + body + 代码骨架——模块名/函数名/签名进入关键词检索（`matched_on: code`）与向量召回
- **骨架哈希绑定**：骨架重建 → 检索哈希变化 → recall 按需重嵌；无挂载节点保持旧口径（文件哈希），存量索引不失效
- 信息栏「检索线索」索引状态与骨架哈希口径一致；`search` 骨架命中显式标注 `code`
- 工具契约不变（v4），golden 18 条全绿；core 191 / cli 13
- 安装包首次包含「检索线索」面板（a60f470）与「代码」栏挂载（b660e77）

## [2.10.0] - 2026-09-08

### 记忆层 M7'/M8'/M-Code + 参数迭代三前置（§8 四阶段全部落地）

**M7'（归档/断边 + stale 热重嵌 + schema 1.1）**
- `archive_node(id, reason?)`（契约 v3 第 11 工具）：`archived: true` + 标题前缀 `[归档]` + 可选 `archived_reason`，先原地原子标记再 rename 移入 `.chain/archive/`（窗口残留由扫描器按标记处理）
- `unlink_nodes(from,to)`（契约 v3 第 12 工具）：parent 置 null + 清理 rel/rel_desc，返回 rel_removed
- 扫描器归档分离（`ChainSnapshot.archived` 与活跃图隔离）；recall `include_archived` 向量/关键词两条路径一致（归档自 L4 起可见）
- stale 热重嵌：写路径标 stale + recall 按需批量重嵌（哈希比对兜底外部编辑）；`rebuild_all` 归档感知
- stats 读触达全覆盖（T3 字面：写也是触达）；watcher archive 扩展
- 错误码 `INVALID_REL:` / `WORKSPACE_MODE_MISMATCH:`（宪法第 7 条）
- schema 1.0 → **1.1**（B 类：派生物格式落地；缺失 .schema 恒为隐式 1.0，migrate 登记 1.0→1.1 步骤）；工具契约 v2 → **v3**（golden 12→16 条）

**M8'（冲突冻结 + 重复检测 + 蒸馏 + 审计）**
- `consolidate(targets?, dry_run=true, k=8)`（契约 v4 第 13 工具；开发为主/分析共享）：BFS 连通分量聚类 → `derived: true` + `[蒸馏]` 骨架节点（逐条来源引用，模板化骨架非 LLM 摘要）；dry_run 默认 true；`CONSOLIDATE_EMPTY:` 错误码
- 冲突即冻结（ADR 0003）：乐观锁 CONFLICT → `[待裁决]` 前缀 + status blocked + frozen 标记（仅元数据写入），冻结期间拒绝一切写入；stats CONFLICT 计数
- 重复检测两阶段（T8）：标题归一化包含关系（零依赖）→ 嵌入余弦 > 0.9 → duplicate_hint + alternative 竞争边；force 跳过
- audit.jsonl（T15 最后派生物落地）：create/update/link/archive/unlink/consolidate/freeze/migrate，append-only、失败不阻断
- 工具契约 v3 → **v4**（golden 16→18 条，13 工具）

**M-Code（代码骨架）**
- `code_map.rs`：tree-sitter-rust 提取公开接口/调用边/Mermaid；骨架派生 `.chain/code_map/<id>.md`；frontmatter `code_map` 挂载；stale 标记生命周期
- `engram-cli sync-code-map` 子命令
- GUI 加性（宪法第 5 条零破坏）：状态徽标（归档/待裁决/蒸馏/代码骨架/stale）、归档视图开关、M-Code 骨架面板（Mermaid 渲染 + 降级）、重嵌索引按钮

**补丁 1（参数迭代方法论三前置）**
- §12 硬伤修复：强度时间轴切换**记忆时钟序数** + 负值 clamp 0（「昨天用过 < 从未用过」根除）
- §16 参数外置：stats.json `params` 区（7 参数先验默认，契约与参数解耦）
- §17 决策留痕：audit recall / dup_detect / dup_force 行
- §18 反馈信号：`feedback` 区 + 有界样本（正负样本/重复真假阳性/归档误判/蒸馏质量）

**UI 流畅度**：涟漪/水面动画优化（位置缓存 + 样式旁路收窄 + 波前 class 增量点亮 + 时长节流 30fps）

**指南 v10/v5（协议补全）**：两份 AI 指南新增「代码骨架内化（M-Code）」章节——骨架**挂理论/概念节点**（信息栏「代码」栏），不另建骨架节点群；`code_map` frontmatter + `sync-code-map` + stale 刷新兜底；MCP 接入的 AI 经 get_guide 自服务获取

### Fixed
- link_nodes 词表外/模式强校验报错无错误码前缀（审计建议）
- 写触达不计入强度触达窗口（T3 字面偏差）
- strength() f32 精度 bug（epoch 秒精度 128s 吞掉 100s 级年龄）
- §12 强度时间轴错用墙钟秒（单位错误）

## [2.9.0] - 2026-09-08
### Added
- 嵌入后端：fastembed 6.0.3 + BGE-small-zh-v1.5 本地模型（安装包内置，`models/bge-small-zh-v1.5` 随包安装到安装目录，exe 旁路优先；`%LOCALAPPDATA%\Engram\models\bge-small-zh-v1.5` 兜底），Embedder trait 可插拔，加载失败走降级链
- 嵌入索引 `.chain/index/`（meta.json + embeddings.bin、content_hash 变更检测、upsert/remove/flush、`engram-cli reindex` 全库重嵌；长驻进程缓存失效自动重载）
- 记忆统计 `.chain/stats.json`（双时钟、TouchKind、ACT-R 强度、gap 截断、calibrate、flush）
- 检索阶梯 `recall` 工具（契约 v2 第 10 工具）：向量 + 强度加成 + 两档阈值（0.35/0.2）+ derived×0.85 + 归档过滤；无索引/无模型降级关键词检索（degraded:true 显式声明）；冷启动按创建时间+图谱度数排序
- `engram-cli reindex --workspace <path>` 子命令
### Changed
- AI 指南 v8 / 开发指南 v3（检索阶梯与 trigger 编码规范、知识库维护与触发线索）
- MCP 工具契约 v1 → v2（新增 recall；golden 12 条）
- 全工具入口接入全局记忆时钟；create_node/update_node/link_nodes 写后触达回写
- search 排序同秒 tie 增加 id 升序次级键（跨平台 golden 确定性）
- golden 契约测试 11 → 12 条
### Fixed
- frontmatter 解析显式剥 BOM（PowerShell Set-Content -Encoding UTF8 写入的节点文件不再被 reindex 静默跳过）
- 嵌入向量二进制写入不再经 UTF-8 转换（新增原子二进制写原语，杜绝向量损坏）

## [2.8.0] - 2026-09-08
### Changed
- cargo workspace 化 + engram-core 下沉（唯一写路径重构，GUI/MCP 入口全部经 core 守门）
- 双模式差异收为 core profile 配置包（rel 词表/指南指针/type·status 词表/校验开关）
- GUI 写路径改走 core 原子写（tmp/rename），与 MCP 写路径同守门
### Added
- golden 契约测试实装（真实 engram-mcp 进程 11 条调用逐条比对，CI golden job）
- 四版本矩阵 + git 短哈希：`engram-mcp --version` / `engram-cli --version` / GUI `get_version_info`
- `.schema` 版本（隐式 1.0）+ `engram-cli migrate` 幂等迁移（detect→backup→transform→verify→write，失败回滚）
- 供应链门禁：cargo-audit + cargo-deny（CI audit/deny job）+ 依赖理由清单
- MCP 打开/GUI 扫描拒绝更高 major 工作区（SCHEMA_TOO_NEW:）
### Planned
- 单实例多工作区（workspace 参数）
- 检索阶梯 L1–L5 + recall 工具

## [2.7.0] - 2026-09-03
### Added
- MCP 阶段一 M1–M4：engram-mcp bin（stdio），9 工具（get_overview / search / read_node / expand / read_path / get_guide / create_node / update_node / link_nodes）
- D2 rel 词表守门 / D3 乐观锁+原子写 / D4 指南下发与返回体提示

## [2.6.0] - 2026-08-31
### Added
- 双击聚焦视图（BFS ≤6 层拉近，再双击退出）
- 常驻节点信息栏（收起为右缘细条，单击切换内容）
### Fixed
- 常驻侧栏遮挡画布控件（margin 动态预留）

## [2.5.0] - 2026-08-29
### Added
- 布局「最小间距」（碰撞力硬保证）与「最大间距」（无关分量上限）主参数
- 滑条默认值圆点（点击恢复默认）
### Fixed
- 单击/双击节点不再触发重布局
- 切换图谱布局压瘪（质心归约保持包围盒面积）

## [2.4.0] - 2026-08-28
### Added
- 两模式交互统一：单击=波源、波纹表达关系
- 布局少交叉（BFS 初始散点 + 交叉惩罚 + 质心归约）
- 关键字搜索、递进关系 rel（contains/solves/alternative）

## [2.0.0] - 2026-08-27 及更早
- 双模式（分析链协议 / 开发自由图谱）、力导向布局、证据系统、折叠与快照、过程日志等
