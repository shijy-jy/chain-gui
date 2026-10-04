# HANDOFF · 记忆层与 M-Code 实现（新对话从此继续）

> 2026-09-08 交接。仓库：G:\test1.x（git）。**§8 全部四阶段 + 补丁 1 + UI 优化 + 2.10.0 发布部署已全部完成。**

## 〇、总状态：✅ 全部完成（2026-09-08）

| 阶段 | 提交 | 状态 |
|---|---|---|
| M6' 记忆层 L2（嵌入/索引/recall，契约 v2） | 18dc419 | ✅ 已审核部署 |
| M7' 归档/断边 + stale 热重嵌 + schema 1.1（契约 v3） | ffe737a | ✅ |
| M8' 冻结/重复检测/蒸馏/审计（契约 v4） | 5e07f6d | ✅ |
| M-Code 代码骨架（tree-sitter + CLI + GUI） | ad75a56 | ✅ |
| 补丁 1 参数迭代三前置（时间轴修复/参数外置/留痕/反馈） | 835b12b | ✅ |
| UI 涟漪流畅度优化 | 07014c0 | ✅ |
| 2.10.0 定版（版本矩阵/CHANGELOG） | 0669fa0 | ✅ |
| 安装包 + 部署 | — | ✅ Engram_2.10.0_x64-setup.exe（60.4MB 含模型）静默安装 D:\AIworkspace\Engram；DSH MCP 已换 2.10.0（4 进程 18:54 重生，get_overview 端到端验证） |
| **推送** | ✅ 3f26e2c..abcab3c 已全部推送（含 7b98878 指南 v10/v5、7b6f81a 60fps、b660e77 代码栏、a60f470 检索线索）；本轮 bd13b59 + c5fd304 待推（GitHub 网络故障，后台 60s×30 重试中） |

**遗留（非阻塞）**：GUI 截图视觉验证在本会话虚拟桌面不可用（窗口恒 15×15），涟漪流畅度与 2.10.0 GUI 需交互式会话复核；子代理审核基建会话内故障（8 次尝试），各阶段为内联审核；迭代循环处于采集期（§19 只采集不调，人确认后生效）；T14 动态代码目录 watcher 留后续。

## 一、任务目标（给新对话建 goal 用）

按《docs/Engram_记忆层与MCode_实现框架_v1.md》§8 顺序实现记忆层与 M-Code：
①M6'（指南 v8/v3 + embed/index/stats/retrieval + recall 工具，golden 10 工具、契约 v2）✅ 已完成
②M7'（archive_node/unlink_nodes + stats 读触达覆盖 + watcher archive 扩展，golden 12 工具、契约 v3）← 实现中
③M8'（冲突冻结[待裁决] + 重复检测 + consolidate 蒸馏 + audit.jsonl，golden 13、契约 v4）
④M-Code（tree-sitter Rust + code_map + CLI sync-code-map + GUI Mermaid/徽标）
⑤schema 1.1 落地 + 记忆图网络 4.2 接口预留
每阶段独立子代理审核（提示词模板见各阶段记录）。拍板已定（框架 §9）：模型随包内置、试点 Rust、90天/0.9/50次、[归档]/[蒸馏]/[待裁决] 前缀、schema 1.1。

## 二、已完成的基线（前面各阶段）

- 落地层 §10 ②-⑥ 全部完成并审核 APPROVED：提交链 0e3d5d8(②) 99cf5c5(③) 205140e(④) 682fc1a(⑤) ae6d3f8(⑤观察) aaf81fe(⑥) 9d34930(版本2.8.0) 2b96181(架构审核整改) + UI 打磨 5 批(a088d39/6dace84/e2af761/bc41be4/845a6d7) + 框架文档 6b084e1。
- 关键产物：cargo workspace（crates/engram-core/engram-mcp/engram-gui/engram-cli）；golden 契约测试（engram-mcp/tests/golden_contract.rs，11 条调用）；CLI 契约测试（engram-cli/tests/cli_contract.rs，10 用例）；宪法 9 条（ARCHITECTURE.md）；安装包 Engram_2.8.0_x64-setup.exe 本地可打。
- DSH 安装版 MCP（D:\AIworkspace\Engram\engram-mcp.exe）已是 2.9.0（git 43a5541）；M6' 完成后需重打 release 并重新部署（改名+紧环复制技巧，进程会按需重生）。

## 三、M6' 状态：✅ 已提交并部署（2026-09-08，提交 18dc419）

**提交内容**：嵌入后端 + 索引 + 双时钟统计 + recall 阶梯 + CLI reindex + 指南 v8/v3 + 契约 v2 + golden 12 条 + 4 模块单测 + real_model_recall 真实模型回归（#[ignore]，CI 无模型自动跳过）。

**本会话新增修复（18dc419 内含，审计前未发现）：**
- frontmatter::parse 显式剥 BOM（PS Set-Content UTF8 陷阱——不剥则 reindex 静默跳过 BOM 节点，实测四象限定位）
- IndexStore 缓存失效检测（meta.json mtime/len 指纹）：长驻 MCP 进程在外部 CLI reindex 后立即可见（此前缓存空索引导致向量模式永不启用）
- RebuildReport.skipped 计数 + CLI 打印跳过数
- 真实模型端到端回归：无索引关键词降级 → reindex → 向量命中（BGE-small-zh 512 维，图谱同义查询 0.522 命中）
- MCP 进程级冒烟结论：PS 5.1 调 MCP 必须 UTF-8 字节直写 stdin（默认 GBK 会把中文查询写坏，传输静默死亡）；golden 全 ASCII 不受影响

**部署**：D:\AIworkspace\Engram\engram-mcp.exe 已换新（改名+紧环复制；旧版备份 engram-mcp.exe.old_20260908_120530）；旧进程已杀，DSH 已重生 4 个新进程（12:05:40）。模型目录 %LOCALAPPDATA%\Engram\models\bge-small-zh-v1.5 已就位。

**审核**：独立子代理 APPROVED（10 项清单全过、零破坏逐条验证）。8 条非阻塞建议，其中 2 条 M7'/M8' 前优先处理：
1. schema 仍 1.0，未联动 1.1（派生文件全落地时 bump 并同步三处+CHANGELOG）
2. is_stale 未接入 recall 热重嵌、写路径未标 stale（缓存失效已做，逐节点 stale→按需重嵌未做）
其余：rebuild_all 硬编码 archived/derived=false（M7'/M8' 必改）、强度负值反转建议 clamp、Write/RecallMiss 不计 touches（T3 字面偏差）、§5.2 upsert 签名漂移、cold_start_rank 实例化静态、文档 nits。

**2.9.0 发布（提交 43a5541）**：版本矩阵 2.8.0→2.9.0（四 crate+tauri.conf）；模型随包内置（embed.rs 双路解析：安装目录 models 旁路优先→LOCALAPPDATA 兜底；tauri resources 增 resources/models，本地打包 _build_v290.bat 拷贝模型入包；CI/release 工作流加资源占位修干净 checkout 假红）。

**本机更新完成（2026-09-08 15:52）**：安装包 Engram_2.9.0_x64-setup.exe（55.9MB 含模型压缩，位于 target\release\bundle\nsis，2.8.0 旧包同目录留存）静默安装到 D:\AIworkspace\Engram（/S /D=）；app.exe 2.9.0 已运行并截图验证渲染正常；模型已装入 D:\AIworkspace\Engram\resources\models\bge-small-zh-v1.5（90.4MB）；bundled MCP resources\engram-mcp.exe 与根目录 DSH MCP 均为 2.9.0 git 43a5541，DSH 已重生 4 进程且 get_overview 端到端可用（guide v3）。注意：tauri v2 资源装进安装目录 resources\ 子目录——模型旁路解析对 bundled MCP（resources\ 下）有效，根目录 MCP 走 LOCALAPPDATA 兜底（本机已存在模型）。
- 环境变化：G 盘曾满盘（LNK1108）——已删 _spike_embed（1.3GB）与 target\debug（29.3GB）腾出约 26GB；下次 cargo test 需重建 debug（数分钟）。bat 脚本须纯 ASCII（cmd 按 GBK 读 bat，中文注释会被误执行）。

**下一步：M7'**（archive_node/unlink_nodes + stats 读触达全覆盖 + watcher archive 扩展；golden 12、契约 v3；审核+提交）。

## 三·五、M7' 实现状态（2026-09-08，进行中，未提交）

**已完成代码**（engram-core 164 测试 + cli 10 契约测试全绿）：
- schema 1.0→1.1 联动（审核建议 #1）：`CURRENT_SCHEMA_STR=1.1` + `IMPLICIT_SCHEMA_STR=1.0`（缺失 .schema 恒按 1.0 处理）；migrate 登记 1.0→1.1 B 类一步（派生物落地，重建靠 reindex）；version.rs/文档/CHANGELOG 同步
- stale 热重嵌（审核建议 #2）：IndexEntry.stale + mark_stale（写路径 create/update/link/archive/unlink 成功后标 stale）；recall 对 stale/哈希不符/索引缺失候选按需批量重嵌并落盘；Node.content_hash（serde skip）由 walker 回填做逐节点比对
- archive_node/unlink_nodes：MCP 注册（12 工具）+ ops 实现；归档 = archived:true + [归档] 前缀 + archived_reason + 移入 .chain/archive/<id>.md（先原子标记再 rename，扫描器按标记处理残留）；断边 = parent null + 清 rel/rel_desc + rel_removed
- 扫描器：ChainSnapshot.archived 与活跃图分离；archive/ 递归扫描只收 archived:true（fold 原始文件自动跳过）；同 id 去重
- recall 可见性：include_archived 两条路径（向量候选集门控 + 关键词降级追加归档命中 id 升序）；rebuild_all 扫 archive/ 且 archived/derived 读自 frontmatter
- stats 读触达全覆盖：read_node/search/expand/read_path 回写（search 内部走 search_impl(false) 防 recall 双计）；写触达计入强度窗口（T3）；**修复 strength() f32 精度 bug**（epoch 秒转 f32 精度 128s，100s 级年龄被吞成 ln(1)=0——改 i64 域做差）
- watcher：nodes/（非递归）+ archive/（递归）双监听（core create_nodes_watcher 签名改为收 root；GUI 调用点已改）
- 错误码：INVALID_REL:（link 词表外）/ WORKSPACE_MODE_MISMATCH:（create/link/archive/unlink 模式门禁）；ARCHITECTURE §7 已更新
- 契约 v3：TOOL_CONTRACT_VERSION=3；golden 流 12→16 条（+unlink/archive/recall×2 归档可见性）；_collect_golden.ps1 与 golden_contract.rs 已同步待重固化

## 三·五、M7' 状态：✅ 已提交（2026-09-08，提交 ffe737a）

**提交内容**：archive_node/unlink_nodes（契约 v3，12 工具）+ 扫描器归档分离（ChainSnapshot.archived）+ recall include_archived 双路径 + stale 热重嵌（审核建议 #2）+ rebuild_all 归档感知 + stats 读触达全覆盖 + strength() f32 精度修复 + watcher archive 扩展 + schema 1.1（审核建议 #1，migrate 1.0→1.1 B 类）+ INVALID_REL/WORKSPACE_MODE_MISMATCH + golden 16 条 + _collect_golden.ps1 字节级 UTF-8 I/O。

**验证全过**：core 164 / cli 10 / golden 16；GUI check；真实模型回归 0.522；真实模型端到端（向量召回 0.421→归档过滤→include_archived 找回→stale 热重嵌）。零破坏机械比对：旧 12 条 golden 仅 link bogus 错误码前缀（INVALID_REL，预期）变化。

**审核说明**：本轮会话子代理基建连续失败（5 次尝试均无输出），独立审核改为内联完成：清单 A–H 逐项过（接口一致性/降级链/golden 确定性/测试不依赖模型/零破坏/健壮性/schema 1.1/实现缺陷），附机械 golden 比对证据。下一轮若子代理恢复，可补一轮正式独立审核（提示词要点见 §五）。

**下一步：M8'**（冲突冻结[待裁决] + 重复检测两阶段 + consolidate 蒸馏 + audit.jsonl；golden 13 工具、契约 v4；审核+提交）。

## 三·六、M8' 状态：✅ 已提交（2026-09-08，提交 5e07f6d）

**提交内容**：consolidate（13 工具：BFS 连通分量聚类 + 模板化骨架 derived:true/[蒸馏]/逐条来源引用，dry_run 默认，CONSOLIDATE_EMPTY:）+ 冲突即冻结（ADR 0003：CONFLICT → [待裁决]+blocked+frozen，冻结拒绝一切写入，stats CONFLICT 计数）+ 重复检测两阶段（标题包含启发式 + 嵌入余弦 >0.9 → duplicate_hint + alternative 边，force 跳过）+ audit.jsonl（append-only，失败不阻断）+ recall 重嵌带 derived（×0.85 降权生效）+ 契约 v4（golden 18 条）。

**验证全过**：core 176 / cli 10 / golden 18；GUI check；真实模型 E2E（重复检测 0.94 触发 → consolidate 计划/执行 → 冲突冻结/拒绝写入）。机械零破坏：前 12 条与 unlink/archive 条目仅 jsonrpc id 偏移/时间戳差异。

**审核说明**：子代理基建持续故障（6 次尝试无输出），审核内联完成（清单 A–H + 机械比对 + E2E）。已知参数存疑（记录在案）：余弦 0.9 对中文偏严（近复制 0.94 才触发），属拍板值待真实数据校准；MCP 无摘帽/解冻通道（治理权在人，GUI/编辑文件处理；[待裁决]/[蒸馏] 徽标随 M-Code 交付）。

**下一步：M-Code**（code_map.rs tree-sitter-rust + CLI sync-code-map + GUI 加性：Mermaid/徽标/归档开关/重嵌按钮；golden 13 不变、无契约 bump；审核+提交）。

## 三·七、M-Code 状态：✅ 已提交（2026-09-08，提交 ad75a56）

**提交内容**：code_map.rs（tree-sitter-rust 提取 pub 接口/调用边/Mermaid + 骨架派生 + stale 标记生命周期 + frontmatter code_map 挂载）+ CLI sync-code-map + GUI 加性（徽标/归档视图开关/Mermaid 面板/重嵌按钮，全部默认关闭零破坏）+ 无契约 bump（golden 18 不变）。

**验证全过**：core 182 / cli 13 / golden 18；svelte-check 0；vite build；GUI check；§7 验收实测——提取 engram-core 自身 183 导出 / 437 调用边，人读骨架可还原模块职责。**§8 四个实现阶段（M6'/M7'/M8'/M-Code）至此全部完成**（记忆图网络 4.2 框架明确不展开）。

**审核说明**：子代理基建持续故障（7 次尝试无输出），审核内联完成。非阻塞遗留：T14 动态代码目录 watcher（mark_stale API 已就绪，动态注册留后续）；GUI 截图视觉验证并入收尾发布阶段。

**下一步：收尾**（版本矩阵 2.9.0→2.10.0 + CHANGELOG 定版 + 安装包重打（模型随包内置）+ GUI 截图视觉验证 + DSH MCP 重新部署 + 推送重试）。

## 三·九、UI 流畅度优化：✅ 已提交（2026-09-08，提交 07014c0）

涟漪/水面动画卡顿优化（src/App.svelte）：位置缓存（静止大图零矩阵开销）+ 每帧样式旁路收窄到波源呼吸（原全图节点每帧写 width/height 是卡顿主因，礁石颤改由 canvas 环承担）+ 波前 class 增量点亮 + 固定描边色/globalAlpha + 闲置零绘制 + 时长节流 30fps（120Hz 不翻倍）。

**注意**：本会话桌面上下文虚拟化——GetWindowRect 恒报 15×15（含此前验证过的 2.9.0 实例），GUI 截图视觉验证在此不可用，需交互式会话复核涟漪流畅度。

**下一步：收尾**（同上）。

## 三·八、补丁 1 状态：✅ 已提交（2026-09-08，提交 835b12b）

**提交内容**（《记忆层理论整理与评估·补丁 1》§15–§22 三前置 + 例外项）：
- §12 硬伤修复：强度时间轴切换记忆时钟序数 + 负值 clamp 0（触达永不惩罚）
- §16 参数外置：stats.json params 区（7 参数，先验默认；契约与参数解耦）
- §17 决策留痕：audit recall/dup_detect/dup_force 行
- §18 反馈信号：feedback 区 + 有界样本（正负样本/重复真假阳性/归档误判/蒸馏质量）

**验证全过**：core 186 / cli 13 / golden 18 全绿（无契约 bump）；真实模型回归 0.522 命中。迭代循环（§19 观察窗→校准）进入采集期：只采集不调，参数变更需人确认（治理权在人）。

**下一步：收尾**（同上）。

## 四、环境备忘

- cargo：`$env:PATH="C:\Users\jcm20\.cargo\bin;$env:PATH"`（每次新 pwsh 都要）
- PowerShell 5.1 陷阱：Set-Content -Encoding UTF8 会加 BOM/GBK 误读——中文文件用 edit/write 工具改；PS 脚本需 UTF-8 BOM；npm 用 npm.cmd（npm.ps1 被策略禁）
- cargo stderr → NativeCommandError exit-1 是假警报，看 stdout 内容
- 视觉引擎：C:\Users\jcm20\ollama\start_vision.bat（127.0.0.1:8080）；仓库 _shot.ps1（窗口截图）、_vision.ps1（直连判图，英文提示词）；modlens 结构化输出会让 3B 模型退化循环，用直连
- git push 目前网络不稳；本地提交清单用 `git log --oneline`
- 未跟踪垃圾别 add（.chain/、_*.cjs、_push_*.txt 等）；git add 只加指定路径

## 五、任务列表（新对话 todo 基线）

已完成：M6' 全部（提交 18dc419 + 审核 APPROVED + 部署完成，见 §三）；M7' 全部（提交 ffe737a，见 §三·五）；M8' 全部（提交 5e07f6d，见 §三·六）；M-Code（ad75a56）、补丁 1（835b12b）、UI-perf（07014c0）、2.10.0（0669fa0）、指南协议（7b98878）、60fps（7b6f81a）、代码栏（b660e77）、检索线索 UI（a60f470）、is_safe_id 放宽（abcab3c）均已提交并推送。

## 六、检索触发句全量整理（本轮完成，2026-09-08）

**目标**：为全部 10 个注册工作区的每个节点正文首行写入「> 触发：同义话术（；分隔）」，并重嵌向量索引让召回吸收触发句。

**结果**：**303/303 节点全部覆盖**（RESTRI 9、learning 32、alive_data 44、ta 59、story 33、water 44、particle 10、render_unified_oss 53、demo/dev 11、demo/analysis 8），10 区 reindex 全绿（CLI 逐区跑）。

**ta 数据修复**（15 个节点因分析模式严格协议被扫描器跳过、MCP 报「不存在」，属既有数据瑕疵）：
- 13 个 `status: completed`（非法枚举）→ `success`：d-009、t-027~t-038、v-007
- d-011：frontmatter 重复 `status:` 行 → 去重
- t-026：标题含「冒号+空格」（资料驱动: HZD）致 YAML 解析失败 → 单引号包裹
- 修复后扫描验证：Analysis 59 节点 / 0 错误（临时 example 诊断，已删）

**提交**：bd13b59（chore：Cargo.lock 同步 2.10.0 + tree-sitter 依赖、models/.gitkeep 清理）；c5fd304（demo 工作区节点触发句，19 文件）。

**工具**（未跟踪，勿提交）：_trigger_run.ps1（通用批量触发句写入，MCP 乐观锁）、_triggers.json（当前只剩 ta 表）、_digest.ps1（紧凑摘要）。

**遗留**：推送待 GitHub 网络恢复（后台重试中）；GUI 侧无动作（检索线索面板 a60f470 已上线，直接读 frontmatter/正文即可见新触发句）。

## 七、代码内化（M-Code 挂载）执行记录（2026-09-08 后段）

**背景**：代码内化协议（指南 v10 附 / DEV v5 §7）与「代码」栏 UI（b660e77）早已就绪，但**挂载从未实际执行**。本轮按协议把 Rust 代码骨架挂到开发模式工作区的理论节点上。

**执行（attach_code_map 官方写路径，7/7 成功）**：
| 工作区 | 节点 | code_map 路径 | 骨架 |
|---|---|---|---|
| water | engram-core-map | `../test1.x/crates/engram-core` | 205 exports / 466 edges / 86KB |
| water | engram-mcp-contract | `../test1.x/crates/engram-mcp` | 2 / 10 |
| water | engram-gui-cli | `../test1.x/crates/engram-gui` | 29 / 8 |
| water | engram-scanner-ops | `../test1.x/crates/engram-core/src/scanner` | 8 / 50 |
| water | engram-model-profile | `../test1.x/crates/engram-core/src/model` | 18 / 0 |
| water | engram-memory-l2 | `../test1.x/crates/engram-core/src/retrieval.rs` | 2 / 19 |
| demo/dev | 工具 | `../../crates/engram-cli` | 0 exports / 44 edges（bin crate 无 pub 项不导出） |

**关键机制**：GUI 选择器（evidence_rel_path）拒绝工作区根外文件——跨工程挂载（water ← G:\test1.x）只能**手工写 frontmatter +「刷新骨架」/CLI sync-code-map**，core 的 refresh 只 join 不设边界，`..` 跨目录可行。

**已知缺口（待办候选）**：
1. 检索集成未兑现：指南称「模块名/函数名/签名进入 recall」，但 retrieval.rs 只搜/嵌节点 title/body，**骨架文件不进检索**——需补 search_impl 扫 `.chain/code_map/`（关键词）或嵌骨架文本，改完要 rebuild+redeploy。
2. 部署版 GUI（19:54）含「代码」栏 ✓，但**「检索线索」面板（a60f470，19:59）未进安装包**——需重打安装包 + 静默重装。
3. learning/story 无 Rust 代码可挂（用户工程是 Unity C#/HLSL，试点语言仅 rust）；engram-frontend 是 Svelte 同因不可挂。

**提交**：b1785db（demo/dev 工具节点挂载演示，仅 frontmatter；`.chain/code_map/` 派生物不提交，ADR 0004）。

## 八、2.10.1：框架代码挂载补齐 + 检索集成兑现（2026-09-09）

**框架挂载补齐**（用户指出「工程框架代码没进挂载区」，3/3）：
- water/engram-framework → `../test1.x/crates`（236 exports / 528 edges，整个 Rust 工程框架）
- water/engram-arch-constitution → `../test1.x/crates/engram-core/src/ops`（49 / 200，唯一写路径实现）
- water/engram-schema-migrate → `../test1.x/crates/engram-core/src/migrate.rs`（10 / 20）
- 跳过：engineering（CI 脚本/yml 非 rs）、frontend（Svelte）、知识库索引（纯索引页）

**检索集成实现**（此前指南承诺未兑现，2.10.1）：
- `code_map::node_retrieval_text(root,id,title,body,has_code_map)` → (检索文本=title+body+骨架, 检索哈希)
- search_impl：骨架命中 → `matched_on: "code"`（关键词 L1 与 recall 降级路径共享）
- retrieval.rs 按需重嵌：code_map 节点哈希绑定检索文本 → 骨架重建自动触发重嵌；无挂载节点保持文件哈希旧口径（存量索引不失效）
- index.rs rebuild_all + node_memory_info 同口径
- 测试：search_hits_code_map_skeleton / recall_vector_reembeds_on_skeleton_change；core 191 / cli 13 / golden 18 全绿
- 版本 2.10.1（4 crate + tauri.conf + CHANGELOG）；提交 280b1f4

**部署**：_build_v2101.bat → release + 模型内嵌 + NSIS；静默装 /S /D=D:\AIworkspace\Engram；MCP 换新（杀 engram-mcp 进程 → 覆盖）；此包首次含「检索线索」面板（a60f470）。

**部署实况（2.10.1）**：安装包成功（app.exe 8:28 换新）；NSIS 未覆盖 mcp/cli（DSH 2 秒重生锁文件）→ 改名旧文件（.old210 留存可回滚）+ 拷新，MCP --version = 2.10.1 (git 280b1f4)，4 进程 8:32:50 从新文件重生；GUI 已重启。water/demo-dev 已 reindex（新口径哈希）。冒烟：search "content_hash"/"MigratePlan" → matched_on code 命中骨架；recall "代码骨架提取 公开接口签名" → vector 命中 engram-framework(0.651)+core-map。提交：280b1f4（集成+版本）、9f25152（Cargo.lock）。

## 九、开发模式工作区改名（2026-09-09，用户拍板）

注册表 workspaces.json 显示名（仅 name 字段，路径不动，DSH MCP 路径绑定不受影响）：
- learning → **蒙特卡洛渲染**（G:\learning 内容 = MC 渲染理论 + 学习卡片）
- story → **遗忘异录**（G:\story\story = 该作品世界观/人物/设定）
- water → **engram+water**（双主题如实标注）
- dev → 保持 dev（demo/dev 示例区）

⚠️ 注册表现在只剩 **7 条**（RESTRI/learning/ta/story/water/render_unified_oss/dev）：alive_data、particle、demo/analysis 三条已不在注册表（用户 GUI 侧删除或 2.10.1 归一化所致，待用户确认）——磁盘数据未动，DSH MCP 仍按路径可访问，GUI 不再显示。

## 十、Engram 独立成库 + 三个废弃工作区数据删除（2026-09-09，用户拍板）

**用户确认**：alive_data / particle / demo/analysis 是其主动删除 → 本轮连带删除数据：
- 整目录删除 `G:\deepseek\alive_data`、`G:\deepseek\particle`、`G:\test1.x\demo\analysis`（均为纯 .chain 数据）
- git 提交 2de8aa3（demo/analysis 8 个受跟踪节点删除），已推送

**Engram 从 water 独立**：
- 新工作区 `G:\engram`（dev，注册名 engram）：13 个 Engram 节点 + 9 个 code_map 骨架 + AI_GUIDE.md 拷贝
- water 保留 31 个海洋/大气渲染节点；知识库索引留在 water（两个渲染 hub 的根），正文删除 Engram hub 条目；water 注册名改回 **water**（不再双主题）
- `engram-framework` parent 置 null，成为新库根 hub；code_map 相对路径 `../test1.x/...` 在新根下依旧解析正确（同为 G: 盘一级目录）
- 两区已 reindex；验证：G:\engram get_overview 根=Engram 工程框架、search extract_skeleton 命中 code、recall vector 正常；G:\water 根=知识库索引→海洋渲染链

## 十一、图谱代码挂载可视化（2.10.2，用户反馈「看不出哪些节点有代码」）

前端增性（src/lib/chain_to_cytoscape.ts + App.svelte + Sidebar.svelte）：
- 挂载节点：**青绿描边**（node[codeMap] 选择器）+ 标签尾缀 **`</>`** 角标；悬停浮层加「代码骨架」
- 工具栏右下角新增 **`</>` 筛选按钮**：一键高亮所有代码节点（辉光）、其余压暗；图谱全量重建/原位数据更新后自动重放筛选类
- 图例新增「已挂载代码骨架」说明；选中已挂载节点**自动展开信息栏「代码」栏**
- svelte-check 0 错误；版本 2.10.2；提交 544237b + bb072a3；安装包 Engram_2.10.2_x64-setup.exe 已静默部署（MCP 换新 --version 2.10.2）

## 十二、2.11.0：1500 节点承载力 + 视觉引擎全面优化（2026-09-09）

**重大发现：此前所有 UI 部署从未生效**——`tauri.conf.json` 的 frontendDist 指向 ../../dist 但没有 beforeBuildCommand，构建链从不跑 vite，一直内嵌 2026-09-08 19:49 的陈旧 dist。2.10.1/2.10.2 的检索线索面板/代码栏/徽标实际都没进用户 GUI（用户"看不到代码挂载"的真正原因）。修复：构建管道显式跑 npm build（_build_run.ps1）；tauri.conf 不依赖 beforeBuildCommand 的 cwd 语义（实测其 cwd 是仓库根而非配置目录，--prefix 相对路径失效）。

**性能改造（审计子代理 ×2 + 自研，P0/P1 全落）**：
- 力导向空间哈希网格：斥力+碰撞合一 3×3 邻胞近似 O(n)，n>400 不再跳过布局（1500 节点按档迭代 80-120 代铺开），拖拽 Int8Array 标记；>800 跳过跨分量上限
- 边渐变真降级（>300 边实线，此前注释未实现）；标签 min-zoomed-font-size 按节点数分档（6/8/10/12）
- 快照签名 $effect 拆分（滑条独立）+ fnv1a 哈希；涟漪 byDepth 遍历/增量点亮/深度分档/波源半径 O(1) 包围盒；位置缓存扁平 Float64Array；chainToElements BFS 头指针+Set+nodeById；父节点搜索式输入（Sidebar/CreateNodeDialog）
- 新架构：src/lib/ui/perf.ts（PERF_TIERS 四档策略中枢 + 帧监控 + fnv1a）；工具栏注册表 toolButtons；图例颜色单一来源（NODE_TYPE_COLOR 派生）；PerfOverlay（⚡ 开关）；window.__engramDebug 调试钩子（CDP 自动化接缝）

**验证（CDP 自动化 + 视觉引擎）**：
- 基线（2.10.2 旧前端）1500 节点：缩放尖峰 12.2ms / 平移尖峰 24.3ms
- 2.11.0：缩放尖峰 6.4ms（−48%）、平移尖峰 12.4ms（−49%）、空闲/涟漪 165fps vsync 满帧；1500 节点布局真正运行（图幅 46k）
- 视觉引擎评审 9/10（标签清晰、无重叠破损、涟漪动画正常）
- engram 工作区：9 个 codeMap 节点在图中（青绿描边+</> 角标）、点击代码节点代码栏自动展开（Mermaid 渲染）、检索线索面板 + 代码骨架徽章全部工作
- 提交 70dd33c + c68d264；安装包 Engram_2.11.0_x64-setup.exe 已部署（MCP 2.11.0，GUI 首次真正带新前端）

**会话事故教训（重要）**：杀进程清理命令曾按名字杀 node.exe——DSH 就跑在 node.exe（dsh/lib/bin.js web），导致会话两次中断、后台任务注册表清空。此后禁止按名杀 node；构建用 harness 后台任务（pwsh run_in_background）+ 日志轮询；脱壳 Start-Process 进程会随工具调用 Job Object 关闭被连带杀死，不可用；PowerShell EAP=Stop 时原生命令 stderr（cargo 进度行）会被 *>> 重定向成异常，Step 内需临时降 EAP。

## 十三、2.12.0：多语言代码内化 + 代码栏滚动浏览（2026-09-09）

**用户痛点**：①好几个工作区没做代码内化（此前只支持 Rust；用户的工程是 C++ 与 Unity C#/HLSL）②代码栏 220px 固定高度不可滚动，大骨架看不全。

**多语言提取（code_map.rs v2.16）**：
- 语言矩阵：rust / csharp / cpp（hlsl·glsl·cuda 同解析器）/ **unity**（.cs→C# 解析器 + .shader/.hlsl/.compute/.cginc→C++ 解析器，混合目录一次提取）
- 语言自动检测 detect_lang：单文件扩展名；目录浅层占比 + 零计数深扫兜底（≤深度5/600文件）；cs+shader 并存→unity
- tree-sitter 0.22→0.23（LanguageFn 统一；c-sharp 锁 =0.23.1——0.23.5 是 ABI15 不兼容）；C#/C++ 提取器 + 调用边
- **跨盘绝对路径**：code_map 允许绝对路径（G 盘工作区 ← D 盘 Unity 工程，Path::join 绝对路径取自身）；CLI --lang auto|rust|csharp|cpp|hlsl|glsl|cuda|unity
- 指南 v11/v6（语言矩阵+绝对路径入协议）；golden 18 条再生成；core 194 / cli 13 / golden 全绿

**挂载落地（16 个新节点）**：render_unified_oss d-015..d-021（C++，src 全景 311 exports）、RESTRI g-001(126)/t-002(63)、water FFT海洋渲染(94,unity)/大气散射渲染(33)/水面材质渲染(19,hlsl)、ta d-002(48)/d-010(20)/d-011(138,SDF shaders)/t-026(20)。d-022 工程基建跳过（tools 为 python/ps1、benchmarks 为场景数据，无支持语言）。注意 attach_code_map 先写 frontmatter 后提取——提取失败会留半挂载，需 detach/手动清字段（本轮已清 d-022）。

**代码栏浏览修复（Sidebar.svelte）**：flex 子项 flex-shrink:0 + .code-md overflow-y:auto 撑满剩余高度（原 shrink 把超长内容压进固定高度导致滚动失效）；⤢/⤡ 一键展开（220↔600px+）。

**验证（CDP+视觉引擎）**：render_unified_oss 8 codeMap 节点在图中；点 d-015 代码栏自动展开（已挂载：src、⤢ 按钮）、.code-md overflow auto + scrollHeight>clientHeight；视觉评审 9/10。MCP 2.12.0（guide v11/v6）已部署；water/render_unified_oss/RESTRI/ta 已 reindex。
**提交**：0583f2b + cfa1ad4（已推送）。

## 十四、2.13.0：连线交叉最小化（2026-09-09，用户要求布局硬规则）

**规则**：图谱布局「尽量不交叉」——三件套落地：
1. 首帧层内质心排序（chainToElements）：BFS 每层按「已就位邻居角度质心（向量和 atan2）」排序——子节点贴父节点（Sugiyama 两层归约），树/链首帧近零交叉
2. 交叉计数网格分桶（countCrossings）：边中点分桶 + 5×5 邻域（cell=最大边长不漏对），O(E·k) 替代 O(E²)
3. 质心交叉归约（polishCrossings）**全规模启用**（原 ≤200 边）+ 逐帧交叉惩罚分桶化上限 800 边

**实测**：perf1500（1500 节点/1499 边）交叉数 **72,595 → 296（−99.6%）**；视觉评审确认连线基本不交叉、结构树状化；图例加「布局自动减少连线交叉」说明。提交 2.13.0（feat + lock）已推送；svelte-check 0 错误；安装包 Engram_2.13.0_x64-setup.exe 已部署。

## 十五、2.13.1：代码栏滚动根因修复（2026-09-09，用户反复反馈「无法上下滑动看全部内容」）

**三层根因（前两轮修复都没打中）**：
1. `.pane` 基类 `overflow: hidden` 在文件后段**同特异性覆盖**了 `.code-pane` 的 `overflow-y: auto` → 面板从未可滚
2. 侧栏自身 `overflow: hidden` + 各面板固定高度叠加 → 代码栏被挤到**视口下方**（y≈1091 > 窗高 800），连面板都看不到
3. `.code-md` 内嵌滚动被 flex 压成 60px 小条，mermaid 区域完全不滚

**修复**：
- `.pane.code-pane` 提特异性压回 overflow-y: auto；`code-md` 改 `overflow: visible; flex: 0 0 auto`——**文档式单滚动**（mermaid+接口+调用边一个滚动条）
- 侧栏加 `.sidebar-scroll` 可滚动内容层（flex:1 + overflow-y:auto；宽度拖拽条留在外层不受滚动影响）
- 选中代码节点：自动展开代码栏 + `scrollIntoView({block:'nearest'})` 滚入可视区；默认高度 320px
- 功能验证（CDP 真实滚轮事件）：滚轮 8 次 → pane.scrollTop 0→960（120px/次，51,406px 内容全程可滚）
- 提交 2.13.1（fix + lock）已推送；安装包 Engram_2.13.1_x64-setup.exe 已部署

## 十六、2.14.0：代码骨架全屏展开页（2026-09-09，用户要求「点展开打开新页面看全部内容」）

**方案**：代码栏头部 ⧉ 按钮 → **覆盖整个窗口的全屏页**（`src/lib/CodeViewer.svelte`：大字体正文 14px/代码 13px + Mermaid + 接口 + 调用边，markdown-it html:false 防注入），Esc/✕ 关闭（Esc 优先级最高）。

**弯路记录（重要教训）**：先按「独立 Tauri 窗口」实现（WebviewWindowBuilder::new + WebviewUrl::App("index.html") + initialization_script 传参）——本环境 WebView2 **运行时创建的窗口导航恒失败**（目标恒 about:blank，连 CDP 手动 location.href 导航都失败；API 用法与 tauri 2.11 文档一致，URL 解析源码无误）。原因未定位（疑似 tauri.localhost 主机解析仅注册于主窗口）。改为应用内全屏页后全部验证通过。后端 open_code_window 命令已移除。

**验证（CDP + 视觉引擎）**：⧉ → 覆盖层渲染 69k 字符骨架 + Mermaid + 可滚动 + ✕；Esc 关闭 ✓；视觉评审 9/10。
**提交**：4ae429e + 79f628f + 2d13ed9（已推/推送中）；安装包 Engram_2.14.0_x64-setup.exe 已部署。另：**部署只走安装包**——直接复制 `cargo build --release` 的 app.exe 是缺 custom-protocol 特性的裸构建，会回退 devUrl（localhost:1420），曾造成一次假部署。

## 十七、指南 v12/v7：代码内化原则落库 + 副本全量同步（2026-09-09，用户问「是否记录到最新指南」）

**结论与修复**：代码内化原则**已记录在仓库指南**（v11/v6 起），但**各工作区 .chain/AI_GUIDE.md 副本严重滞后**——开发模式副本停在 v2、分析模式 v4/v7、ta/demo-dev 无指南。根因：`init_chain` 只在初始化时刷新，且**开发模式「缺省才写、不刷新」**（v2.1 注释明说）——指南承诺的「初始化/扫描时自动刷新」只实现了一半。

本轮：
- 指南补「骨架浏览：⧉ 全屏展开页」→ 版本 v12/v7（guide.rs 常量同步 + golden 再生成）
- **开发模式改为「过期即刷新」**（ops/chain.rs init 分支：版本比对后覆盖旧副本）
- 8 个活跃工作区副本全量同步（dev → v7 含代码内化；analysis → v12 含代码内化）
- 部署 MCP 2.14.0（--version：guide analysis v12/dev v7）；core 194 / cli 13 / golden 18 全绿
- 提交已推送。遗留注意：扫描路径仍不刷新（刷新只在 init），未来可考虑在 GUI/MCP 命令层加 scan 时刷新。

## 十八、2.15.0/2.16.0：外部实测三修 + 支链闭环（2026-09-09）

**2.15.0 外部 AI 实测三修**（用户转交的「检索式接手」测试报告，24% 阅读换 80% 接手能力）：
- ① recall 挂死修复：共享嵌入器状态机（Loading/Ready/Failed）+ 后台预热（MCP 启动预热 + recall 自暖）——未就绪立即关键词降级（显式原因），实测首调 157ms（原 120s 超时）
- ② read_node 增 `include_code_map` → code_map_md（骨架全文 83,933 字符）+ code_map_stale——MCP 骨架通道打通
- ③ get_overview 增 `entry_hubs`（标题命中索引关键词/度数 ≥2 的前 5 入口）——d-015 自动排第一
- 契约 v4→v5；指南 v13/v8；core 197/cli 13/golden 18

**2.16.0 支链闭环**（用户反馈：分析模式支链走到任务就断）：
- 协议（指南 v14）：task 置 success 前必须挂 verification 子节点；例外=正文「自验收：」注明
- UI：无子无自验收的 task → 琥珀虚线框开环标记 + 悬停 + 图例（修了 parentSet/childSet 误用 bug——原实现把所有非根节点误判为有子）
- **存量闭环执行**：47 个 success 任务补建 v-* 节点（ta 15、RESTRI 1、render_unified_oss 31），13 个非 success 保持开环；三区已 reindex（ta 74 / RESTRI 10 / render_unified_oss 86 节点）
- 工具 `_close_chains.ps1`（可复用批量闭环脚本，未跟踪）
- 验证：RESTRI 开环 6→4（t-003 闭环 ✓ t-001 原有 ✓）；t-002 琥珀虚线框渲染确认（#fbbf24 dashed 2px）；提交已推送

审核提示词要点（j）：框架 §5 接口与实现一致性、降级链 degraded 显式声明、golden 确定性、测试不依赖模型、零破坏（原 9 工具输出除 get_guide 内容外不得变化）。

## 十九、2.17.0：阅读模式（图结构 ↔ 节点文件树，人专用视图；2026-09-21）

**用户痛点原话**：「大量节点时阅读文件十分不方便，希望增加一个阅读模式（只有人可以用，AI 不能识别不能使用）……切换到这个模式后，文件结构会按照节点图结构梳理，整理出文件树，然后人可以在软件内部打开节点直接阅读这个节点，或者其实就是加一个节点文件树显示方式可以从图结构显示方式切换」。

**方案（纯前端加性改动；Rust 零行为变更，只动版本号 + 指南副本不碰）**：
- `src/lib/node_tree.ts`（新，纯逻辑无副作用）：图结构 → 文件树。以 frontmatter `parent` 为准（walker 据此派生 edges，天然单父树），同层确定性排序（类型序 goal→design→task→verification→note → created → id）；迭代式 DFS（1500 节点深链不爆栈）+ 环检测（环提升为根标「环」）+ 悬空 parent 容错（标「悬」）；导出 flattenVisible（按展开集压平可见行）/ readingOrder（DFS 前序＝阅读顺序）/ defaultExpanded / breadcrumbOf / searchNodes（标题/id/标签/**正文命中**带片段）/ isTaskOpenLoop
- `src/lib/ReaderMode.svelte`（新）：覆盖全窗口的阅读视图（z-2600，低于全屏代码页 3000）。左＝节点文件树（标题行「节点文件树 · 按图结构 · 一个节点 = 一个 .md」、折叠展开/展开全部/折叠全部/含归档、类型色点、id、状态字形 ✓◐✕⛔○、归档/待裁决/蒸馏/代码骨架/未闭环标记、树宽拖拽、检索去抖 160ms）；右＝全文阅读（渲染 Markdown+KaTeX / **原文**、字号三档、复制正文、面包屑、子节点与证据跳转、⧉ 代码骨架全屏页、**在图谱中定位**、上一篇/下一篇沿树前序 + n/总数）
- `src/App.svelte`：+`readMode` 视图状态 + 工具栏「📖 阅读模式 / ◧ 图谱视图」切换（Esc 退出）；Esc 优先级＝全屏代码页 > 阅读模式 > 双击聚焦 > 侧栏；阅读模式下吃 watcher 实时更新（不吃"编辑中不覆盖"的保护，AI 写入实时进文件树）；覆盖层下停水面渲染循环（`stopWaterLoop` + `drawWater` 早退）；`clearGraph` 一并退出

**人专用不变量（用户硬要求；写死在 ReaderMode.svelte 头部注释）**：不写工作区任何文件（偏好只落 localStorage）、不新增后端命令、不注册 MCP 工具、**不改 AI 指南副本**（resources/AI_GUIDE*.md 零改动——本轮唯一没动的"接口"）、不进 `window.__engramDebug`（CDP/自动化脚本看不到也驱动不了）。实测断言：`Object.keys(__engramDebug)` = `[cy, snapshot, mode]`。

**实测（CDP 驱动真实应用；`_verify_read.cjs` + 截图 `_shots/read-*.png`）**：
- ta（74 节点/73 边 · 分析模式）：进入 21–178ms（首屏折叠到当前路径 11–13 行）；展开全部 34–40ms / 74 行；点 d-002（代码节点）→ 面包屑 3 级 + 正文 + ⧉ 代码骨架按钮；下一篇 3/74→4/74；原文 1254 字符（渲染层确实切换）；字号 16→19px；检索「渲染」36 命中（标题 + 正文片段）；**在图谱中定位** → 覆盖层关 + 目标节点 search-hit 高亮 + 侧栏同步；Esc 退出后图谱 74 节点原样
- perf1500（1500 节点 · 开发模式）：进入 175–200ms（首屏 7 行＝总览）；展开全部 302–320ms / 1500 行 / 7576 DOM / 滚到底 72–85ms；全库正文检索 < 400ms（含 160ms 去抖，0 命中场景＝最坏全扫）
- 运行期 0 异常 0 控制台错误；svelte-check 0 错误 0 警告
- 部署：`Engram_2.17.0_x64-setup.exe`（61,247,571 B）静默安装 → 注册表 2.17.0；安装后实例 CDP 复验：app 2.17.0 / `📖 阅读模式` 按钮 / 树标题行 / 「在图谱中定位」/ Esc 关闭全部就位；engram-mcp.exe + engram-cli.exe 同步 2.17.0（改名备份 `.old2160b`，`--version` 复验）

**弯路记录（下次省时间）**：① 验证脚本首轮撞上"用户窗口与新实例共享 WebView2 profile"——localStorage 两边共用，启动即带着用户的上次工作区；脚本加归一化（先 Esc 回图谱 + 按 `.dir` 精确等待目标工作区）。② 收尾必须清理自己写进共享 profile 的偏好（`engram-read-*`、`chain-gui-last-dir`），否则用户下次打开就是测试残留（本轮已复位）。③ NSIS 首轮报 `os error 32 另一个程序正在使用此文件`（新生成的 61MB 安装包被 Defender 扫）→ 删残留重跑即过。④ `cargo tauri build` 与 `cargo build --release` 特性不同（custom-protocol）会重编 engram-gui/core；且**必须先关掉正在运行的 `target\release\app.exe`**（否则 `failed to remove file … os error 5`）。⑤ 安装包里的 app.exe 与 target\release\app.exe **哈希不同**（bundler 事后 patch bundle type），但内容一致——用"安装后再 CDP 复验 UI 元素"判定，别用哈希。

## 二十、2.18.0：文件树模式 = 人的编辑面（新建/编辑/删除/改挂载；2026-09-22）

**用户原话**：「给阅读模式增加新建文件功能和编辑功能，新建的文件在那个文件树下就自动在节点图中生成节点挂载在下面，就是文件夹模式和节点图模式都是同一套系统的两种显示方式和编辑方式，节点图适用于 AI 使用和编辑，文件夹适合人来编辑和操作」。

**产品决策（用户当面拍板：要放开）**：分析模式也允许人编辑结构——新增 **GUI 专属「人用通道」** `create_node_human` / `delete_node_human` / `set_parent_human`（core `ops::node_edit` 内实现，内部 `human: bool` 开关，`false` 即原行为）。**MCP 契约与 AI 行为零变化**：`create_node` / `delete_node` / `set_parent` 在分析模式仍一律拒绝（golden 契约 18 条照旧全绿）。分析模式人用护栏（全部在 core）：
- 新建：必须挂**已存在**父节点（严格单根树，不许新增根；根 goal 由初始化创建）、类型限 goal/design/task/verification、状态限五态（拒绝 note/none）
- 删除：禁删根、禁删还有子节点的节点（防悬空分支）
- 改链接：禁断成根、禁成环（新父节点在本节点子树里 → 拒绝）

**前端**：`ReaderMode.svelte` 三态操作面（read / edit / new）——＋新建（标题 / id 自动建议 `g-/d-/t-/v-` 下一个空闲号 / 类型 / 状态 / 父节点（默认＝当前选中，可搜索改）/ 关系 / 标签 / 正文）、✎编辑（标题/状态/标签/正文/父节点+关系，Ctrl+S 保存、Esc 取消）、🗑删除（两段式确认）；编辑中切节点被拦截（草稿不静默丢）；已归档节点只读。`App.svelte` 四个写回调 → 后端 core 守门；`create_node_human` 后按 id 差集定位并选中新节点；正文/标签在建完后补一次 `update_node`（`CreateNodeInput` 与 MCP 同构，不含 body/tags）。按钮更名 `🗂 文件树模式 ⇄ ◧ 图谱模式`。

**实测（CDP 驱动 + 隔离的临时工作区 `_scratch/ws_dev`、`_scratch/ws_ana`，0 异常 0 控制台错误）**：
- 开发模式：新建 → 图谱节点数 +1、挂载边正确、树行出现，**磁盘文件 parent/rel/tags/正文逐字核对通过**；编辑（标题/正文/标签）→ rev+1、磁盘一致；改挂载 A→B → `parent: rp-b` + 边 `rp-b->rp-a` + 树里嵌套 ✓；两段式删除 ✓
- 分析模式：新建 design（in_progress）挂到根 goal → 图谱 +1、`validation.valid`、磁盘一致；**删根** → 「根节点不能删除（链协议要求保留唯一根 goal）」；**删有子节点的节点** → 「该节点还有 1 个子节点（t-001）……链不能出现悬空分支」；**成环**（把根挂到自己的后代）→ 「这会形成环（新父节点在本节点自己的子树里）——链协议禁止环」且根 parent 不变；**合法改挂** t-001→g-001 ✓
- core 197 → **203** 测试全绿；golden 契约全绿；svelte-check 0 错 0 警；截图 `_shots/v21-*.png`、`_shots/v22-*.png`

**本轮抓到的真 bug（已修，教训值得记）**：v2.19 让文件树模式吃 watcher 实时推送（`if (selectedNode && !readMode) return`）后，**写入期间** watcher 会推来"写入之前"的旧扫描结果，把刚写回的 snapshot 覆盖——表现是新建节点的正文/标签"闪回旧值"（首轮实测新建节点的 body 只有 16 字符）。修法：`markSelfWrite()` 自写窗口 1200ms，窗口内丢弃 watcher payload，窗口外的外部写入照常实时刷新。实测：新建后 1.5s 复读，正文/标签保持新值 ✓。
> 排查要点：**别只看前端 snapshot 判定写入结果**——那次磁盘文件一直是对的（`t-007.md` 正文完整），是前端状态被旧 payload 覆盖。

**弯路记录**：① 临时工作区用 `__TAURI_INTERNALS__.invoke('add_workspace')` 注册后 **App 的工作区列表不会刷新**（只在启动时拉取）→ 测试必须写 `localStorage(chain-gui-last-dir/mode)` + `Page.reload` 才能打开它；② 验证脚本早期版本因工作区没开起来，**把测试节点建进了用户真实工作区**（learning 32 节点 / RESTRI 10 节点）——test-* 节点已全部清理，两区节点数与内容已核对无残留；RESTRI 的 `g-001.md` 被一次「保存」测试重写（内容语义不变：tags 由内联变块状列表、revision/updated 递增）——如需旧格式可手工改回；③ 编辑面的父节点搜索**故意排除"正在编辑的节点自己"**，测试时别拿当前节点当查询词（会误判为"下拉不出候选"）；④ `_verify_edit3.cjs` 里模板字符串内的 `\n` 必须写 `\\n`（否则注入的 JS 源码换行 → SyntaxError）。


