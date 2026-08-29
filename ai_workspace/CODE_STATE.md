# CODE_STATE — chain-gui

code_baseline: BL-20260814-02
last_update: 2026-08-14
current_status: v1.3 代码完成（开发者设计上下文管理三件套 + chain 修复审查；cargo test 75 全绿 + svelte-check 0/0 + 真实目录端到端通过）；待开发者打包 + 实装验收

## 基线历史
| 基线号 | 日期 | 状态 | 主导 AI | 摘要 |
|--------|------|------|---------|------|
| BL-20260814-02 | 2026-08-14 | active | 开发者设计 + chain 修复 | v1.3 上下文管理三件套（commit af07753，18 files +1360/-19）：active_chain/chain_health/project_persona、snapshot 快照、fold 折叠、指南 v3 §4.9-4.11、前端快照/折叠/活跃链 UI；chain 修复：2 处 UTF-8 切片 panic、fold 红线（failed/blocked 拒折 + _self.md 备份）、快照 id 毫秒防碰撞、persona 词边界匹配 |
| BL-20260814-01 | 2026-08-14 | superseded | chain | M11 AI 工程推进适配（commit 98d9227，18 files +962/-36）：AI_GUIDE v2（9 条守则+版本标记+过程留痕/产物规范/evidence 协议）、指南版本对比刷新、Node.evidence、PROCESS_LOG、前端侧栏/状态栏升级，version 1.2.0 |
| BL-20260812-01 | 2026-08-13 | superseded | ai-coordinator | 初始基线，v3 规划就绪，Init 启动 |
| BL-20260812-02 | 2026-08-13 | superseded | ai-rust | M1 Tauri 2 + Svelte 5 脚手架完成 |
| BL-20260813-01 | 2026-08-13 | superseded | coze | M4 收尾：P0 now_iso8601 civil 重写 + P1 校验修复 + UI 深色重设计（commit 48dfc6f，11 files +560/-242，cargo test 预期 17） |
| BL-20260813-02 | 2026-08-13 | superseded | coze | M6 收尾：UI 视觉升级（圆点+类型色+状态光晕+cose 力导向+现代暗色面板，commit 344456b + compound 修复 6d3e031），开发者 15:01 视觉验收通过 |

## 进行中
- v1.3 上下文管理三件套 — 设计: 开发者 / 实现+修复: chain — 代码完成（cargo test 75 全绿 + svelte-check 0/0 + 真实目录端到端通过），待开发者打包 + 实装验收

## 已完成
- Init 工程初始化 — 主导: ai-rust — 完成于 2026-08-13
- M1 项目脚手架 — 主导: ai-rust — 完成于 2026-08-13
- M0 ai-frontend 自注册 — 主导: coze / 接收: ai-frontend — 完成于 2026-08-13
- M0 ai-rust 自注册 — 主导: coze / 接收: ai-rust — 完成于 2026-08-13（验收：ai-coordinator/reviews/ai_join_ai-rust.md）
- M2 核心数据结构 + 目录扫描 — 主导: ai-rust — 完成于 2026-08-13
- M3 图谱可视化（cytoscape.js）— 主导: ai-frontend — 完成于 2026-08-13（开发者验过视觉，5 节点 4 边 + 4 NodeType 配色 + dagre LR 布局 + onMount 缺 import bug 已修）
- M4a update_node command — 主导: ai-rust — 完成于 2026-08-13（coze 验收 reviews/M4a.md 9/9 步 + 5/5 硬指标 + cargo test --lib 13/13 全过）
- M4b 节点侧栏 UI — 主导: ai-frontend — 完成于 2026-08-13
- M4 全量修复 + UI 重设计 — 主导: coze — 完成于 2026-08-13（P0 now_iso8601 重写为手写 civil 算法不引依赖 + P1 body 校验顶层化/删错位分支/防回归测试；VSCode 深色风 UI：48px 工具栏 + 满铺画布 + 深色侧栏 + 空状态 + 点空白关侧栏 + resize fit；开发者 10:04 视觉验证通过）
- M6 UI 视觉升级 — 主导: coze — 完成于 2026-08-13（圆点+类型色+状态光晕+cose 力导向+现代暗色面板；344456b 4 files +198/-124；6d3e031 修复 data.parent 撞 cytoscape 保留字段导致的 compound 巨型容器；vite build + svelte-check 全绿；开发者 15:01 视觉验收通过）
- M7 schema 严格校验 — 主导: ai-rust — 完成于 2026-08-13（字段级 9 条 + 结构级 5 条 + 18 条测试矩阵；walker 集成容错解析；cargo test 35/35 全过；顺带修复 M4 遗留 civil_from_days 测试期望值 10957→11016）；**coze 15:33 验收通过（reviews/M7.md）**
- M5 文件监听 + 自动重载 — 主导: ai-rust — 完成于 2026-08-13（notify v8.2.0 + 300ms 去抖 + chain-changed event + 前端 listener + 编辑保护 + 3 单测；commits 269cd4b/1adc9f8/29a50ea）；**coze 16:35 验收通过（reviews/M5.md，cargo test 38 全过实跑复核）**
- M8 校验状态面板 + 初始化向导 — 主导: ai-frontend — 完成于 2026-08-13（StatusBar.svelte 底部状态条 + 校验详情抽屉 + 重新扫描按钮 + 初始化向导；后端 init_chain command + 2 单测；cargo test 39/39 全过；vite build + svelte-check 全绿；commits e0c976b/e6f1b51）；**coze 17:04 验收通过（reviews/M8.md，三连实跑复核 + 零新依赖 git 实证）**
- M9 Windows 打包发布 — 主导: coze — 完成于 2026-08-13（tauri.conf.json 5 处定稿：identifier com.chaingui.desktop / version 1.0.0 / targets ["nsis"] / devtools false；chain-gui_1.0.0_x64-setup.exe 2.2MB 产出；零新依赖；构建后 cargo test 39 全过）；**coze 17:51 验收通过（reviews/M9.md），v1.0 收官，实装链路留开发者**
- M10 AI 使用指南内嵌 — 主导: coze — 完成于 2026-08-13（resources/AI_GUIDE.md 单源 + include_str! 内嵌 + init 幂等写盘 + get_ai_guide 命令 + 工具栏复制按钮 + validator 子 goal 合法化删 warning；cargo test 40 全绿；version bump 1.1.0；chain-gui_1.1.0_x64-setup.exe 2.16MB；开发者 4 条结构规则修订入册）；**coze 20:27 验收通过（reviews/M10.md），内测留开发者**
- M11 AI 工程推进适配 — 主导: chain — 完成于 2026-08-14（AI_GUIDE v2：9 条守则 + CHAIN_GUIDE_VERSION 标记 + §4.7 过程留痕 + §4.8 产物规范 + evidence 可选字段；init 指南版本对比刷新；scan 陈旧告警；PROCESS_LOG append/get 命令；侧栏只读元信息 + evidence 编辑 + 日志追加；状态栏指南版本 + 日志抽屉；cargo test 56 全绿 + 真实目录端到端验证；commit 98d9227；version 1.2.0）；待开发者打包 + 实装验收
- v1.3 上下文管理三件套 — 设计: 开发者 / 实现+修复: chain — 完成于 2026-08-14（active_chain 活跃链树状摘要 + chain_health 状态计数 + project_persona 项目画像；snapshot 快照三命令；fold 折叠归档；指南 v3 §4.9-4.11；前端快照按钮/折叠两段式确认/活跃链与快照抽屉；chain 修复审查：2 处 UTF-8 切片 panic → truncate_utf8、fold 红线 failed/blocked 拒折 + _self.md 备份、快照 id 毫秒防碰撞、persona 词边界匹配、前端抽屉互斥；cargo test 75 全绿；commit af07753）；待开发者打包 + 实装验收

## 阻塞
（暂无）

## 遗留验证
- 写回交互链路：改 status/tags → 保存 → 图谱刷新 + git diff 看写回，开发者新 UI 上随手确认后回滚测试数据
- ~~cargo test 待实跑确认~~ 已闭环：M7 验收时 coze 实跑 35 全过（含 M4 修复的 civil_from_days 期望值修正）
- 里程碑编号顺延：原 M6 schema 校验 → M7，原 M7 状态面板 → M8，原 M8 打包 → M9

## 已加入 AI 名单
| AI 角色 | 工具 | 版本 | 加入基线 | 加入日期 | operator | profile |
|---------|------|------|----------|----------|----------|---------|
| ai-frontend | Codely CLI | Claude Sonnet 4.5 | BL-20260812-02 | 2026-08-13 | 瑾瑜 | ai-frontend/ai_self_profile.md |
| ai-rust | Codely CLI | Claude Sonnet 4.5 | BL-20260812-02 | 2026-08-13 | 瑾瑜 | ai-rust/ai_self_profile.md |
| ai-qa | （coze 兼任） | — | — | — | 瑾瑜 | — |
| chain | DSH（DeepSeek Harness） | deepseek-v4-pro | BL-20260814-01 | 2026-08-14 | 瑾瑜 | chain/ai_self_profile.md |

> 已加入 AI 名单由 M0 阶段各 AI 按 v3 §6.5 自注册协议自行填写。
