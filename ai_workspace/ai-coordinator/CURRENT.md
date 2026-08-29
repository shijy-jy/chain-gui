# ai-coordinator (coze) — 当前任务

code_baseline: BL-20260813-02
last_update: 2026-08-13
ai: coze
current_task: M10 验收通过（coze 20:27，reviews/M10.md）：AI_GUIDE.md 内嵌 + init 写盘 + 复制按钮 + 子 goal 合法化；v1.1.0 安装包 chain-gui_1.1.0_x64-setup.exe 2.16MB 已交付开发者内测（20:25 出包，构建后回归 40 全绿）
status: M10_done
note: 打包实录：NSIS 下载再遇网络坑，5 趟直连全挂 → 改 curl 手动下载 + 缓存布防 zip/dll + 离线 build，盯梢 25 次抓窗口成功（教训：tauri 缓存缺件会整体重下，须双件布防）；GitHub push 挂起——GCM 旧 token 失效已被开发者删除，下次 push 需开发者终端跑一次重新授权；commit 2a1f285（M10 代码）+ 验收 commit 待一并推

## 近期任务

- [x] v3 规划书交付（chain_protocol_gui_impl_plan_v3.html）
- [x] M1 任务卡草拟（task_cards/M1.md）
- [x] Init 任务卡草拟（task_cards/Init.md）
- [x] Init 验收通过
- [x] M0 ai-frontend 自注册
- [x] M0 ai-rust 自注册 + coze 验收（reviews/ai_join_ai-rust.md）
- [x] v3 §3 / §5 AI 工具链表格回填（ai-frontend / ai-rust 两行实填）
- [x] M2 任务卡发布到协作区（task_cards/M2.md）
- [x] ai-rust 跑完 M2 9 步（18 分钟，9/9 + 单测 8 + 集成 1 + 0 build error）
- [x] coze 验收 M2（reviews/M2.md 通过）
- [x] M3 任务卡发布到协作区（task_cards/M3.md）
- [x] ai-frontend 跑完 M3 9 步（8 分钟，9/9 + 代码 + 配色 + dagre + IPC 全通）
- [x] coze 验收 M3 代码层（reviews/M3.md 通过 8/9，视觉 1/9 待开发者）
- [x] 开发者手动验 M3 视觉（08:18 验证 5 节点 4 边 + 4 NodeType 配色 + dagre LR 布局，通过）
- [x] 修复 M3 黑屏 bug（App.svelte 缺 onMount/onDestroy import，复盘 reviews/M3_bugfix_onMount_import.md）
- [x] M3 完整验收（代码层 8/9 + 视觉 1/1 → 总 9/9 通过）
- [x] 派 M4（ai-frontend 主导 + ai-rust 加 update_node command）
  - [x] 派 M4a ai-rust 后端（task_cards/M4a_backend.md）— 8 分钟 9/9 步 + 5 单元测试
  - [x] coze 验收 M4a（reviews/M4a.md 通过 9/9 + 5/5 硬指标，cargo test 13/13 全过）
  - [x] 派 M4b ai-frontend 节点侧栏 UI（task_cards/M4b_frontend.md）— 完成 + self_review
- [x] 08-13 全量代码审查 + 全量修复（coze 直改）
  - [x] 修复 App.svelte 文件重复段（edit_file 追加重复 bug，write_file 重写）
  - [x] 全量代码审查输出 P0×1 / P1×7 / P2×3 清单
  - [x] P0：now_iso8601() 重写为手写 civil 算法（合法 RFC3339 UTC+8，不引 chrono）+ 2 新测试
  - [x] P1：body 空校验顶层化（trim 版）+ 删 apply_update 错位 body 分支 + 防回归测试 → cargo test 预期 17
  - [x] UI 重设计落地（VSCode 深色风）：48px 工具栏 + 满铺画布 + 深色侧栏 + 空状态 + 点空白关侧栏 + resize fit + `{#if selectedNode}` 挂载 Sidebar
- [x] 开发者 10:04 视觉验证新 UI 通过（深色主题 + 工具栏统计 + 5 节点嵌套图谱正常显示）
- [x] M4 入库收尾（48dfc6f 代码 11 files +560/-242；b9c6dda 台账登记）
- [x] 派 M5（task_cards/M5_file_watcher.md，08-13 发布）— 文件监听 + 自动重载，ai-rust 全包；**开工前需开发者批准 notify 依赖**
- [x] M6 UI 视觉升级（coze 直改，开发者 14:20 指令"现在就开始照着这一版改"）：圆点+类型色+状态光晕+cose 力导向+现代暗色面板；commit 344456b（4 files +198/-124）；vite build + svelte-check 全绿；顺带修复 3 个 TS 类型错误（Stylesheet→StylesheetJson / parent null→undefined / $props 泛型→注解式）；任务卡 task_cards/M6_ui_visual_overhaul.md；6d3e031 修复 compound 巨型容器 bug（data.parent 撞 cytoscape 保留字段，改名 chainParent）
- [x] M6 开发者视觉验收（15:01 通过；验收中暴露 data.parent 撞 cytoscape compound 保留字段 → 巨型紫色容器，6d3e031 修复改名 chainParent 后复验通过）
- [x] M7 schema 严格校验（开发者指派 ai-rust 实施）：字段级 9 + 结构级 5 + 18 测试矩阵 + walker 容错集成；cargo test 35 全过；顺带修复 M4 遗留 civil_from_days 测试期望值；coze 15:33 验收通过（reviews/M7.md）
- [x] M5 任务卡更新放行（notify 批准登记 + 基线翻 BL-20260813-02）+ M8 任务卡发布（task_cards/M8_status_panel.md，2026-08-13）
- [x] M5 实施 + coze 验收（ai-rust 16:13 完成 commits 269cd4b/1adc9f8/29a50ea；coze 16:35 验收通过 reviews/M5.md，cargo test 38 全过实跑 + 依赖 git 实证仅 notify v8.2.0）
- [x] M8 实施 + coze 验收（ai-frontend 17:01 完成 commits e0c976b/e6f1b51；coze 17:04 验收通过 reviews/M8.md：39 测试 + vite build + svelte-check 三连全绿 + 零新依赖实证 + init_chain 幂等单测扎实）
- [x] M9 打包发布（coze 直改，17:51 出包 chain-gui_1.0.0_x64-setup.exe 2.2MB + reviews/M9.md 验收通过；5 趟 build 踩坑实录见报告；实装留开发者）
- [x] M10 AI 使用指南内嵌（coze 直改，19:22 三连全绿：cargo test 40 / svelte-check 0/0 / vite build；task_cards/M10_ai_guide.md；指南经开发者 4 条规则修正定稿 resources/AI_GUIDE.md）

## 待清理（v3 规划书更新时登记）

- P1：update_node 每次重扫全目录（M5 监听后优化）；chainToElements label `\n` 换行待实测；parse_node_file round-trip 浪费
- P2：emoji 状态标签（M6 侧栏已去 emoji 化改纯文字，画布 label 待实测）；并发锁；cytoscape-dagre 死依赖（M6 后代码不再 import，待 npm uninstall）
- 布局：M6 已切 cose 力导向（dagre 分层废弃）；cose randomize:true 每次刷新位置洗牌，若体验烦人后续改 preset 记忆位置
