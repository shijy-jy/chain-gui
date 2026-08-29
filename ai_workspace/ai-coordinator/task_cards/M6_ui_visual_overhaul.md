# M6 —— UI 视觉升级：力导向星团 + 现代暗色（coze 直改）

> **M6 目标**：图谱从"流程图"升级为"可以看清联系的数据库"——Obsidian Graph 式的点线网络，但更有秩序、更现代。
> **本工单**：coze 直接实施（开发者 2026-08-13 14:20 指令"现在就开始照着这一版改"），不走派单。
> 工单位置：`G:\test1.x\ai_workspace\ai-coordinator\task_cards\M6_ui_visual_overhaul.md`

---

## 任务卡元信息

- **code_baseline**：BL-20260813-01（commit `01f898a` 之后）
- **主导 AI**：coze（开发者直接指派）
- **协作 AI**：无
- **实际耗时**：约 20 分钟（含 3 个 TS 类型错误修复）
- **任务状态**：✅ 验收通过（开发者 2026-08-13 15:01 视觉验收；含 compound 修复 commit `6d3e031`）

---

## 背景

M4 的 UI 是 VSCode 深色风（矩形节点 + dagre 严格分层），开发者 14:08 反馈："完全就是个流程图，我的应该更像一个可以看清联系的数据库，而且很土，并不现代"。
参考稿 v2（现代风：圆点 + 光晕 + 力导向星团 + 细曲线 + 纯黑星空背景）通过开发者评审，14:35 定稿"暂时可以用这一版"。

**视觉规范（决策 D-29）：**

| 维度 | 规则 |
|------|------|
| 节点形状 | 圆点（ellipse），禁用矩形/菱形等几何框 |
| 类型 = 颜色 | goal 紫 `#a78bfa` / design 蓝 `#60a5fa` / task 青 `#22d3ee` / verification 绿 `#34d399` |
| 重要性 = 大小 | `16 + degree * 6`（连接数越多点越大） |
| 状态 = 光晕 | pending 半透明 / in_progress 白色光晕 / success 微光 / failed 红光晕 / blocked 虚线框 |
| 边 | 1px 半透明白 bezier + 小三角箭头 |
| 背景 | 纯黑 `#0a0a0a` |
| 布局 | cose 力导向（cytoscape 内置，零新依赖） |

## 目标

1. 画布：dagre 严格分层 → cose 力导向星团；矩形节点 → 圆点
2. 状态表达从"边框颜色"升级为"光晕 + 透明度"
3. 工具栏 / 侧栏 / 全局样式统一现代暗色风
4. 零新依赖（cose 布局 + shadow 光晕都是 cytoscape 内置能力）

## 实施清单（已完成）

### App.svelte
- [x] 删除 cytoscape-dagre import 与注册逻辑（npm 依赖清理登记 P2）
- [x] layoutConfig → cose（idealEdgeLength 110 / nodeRepulsion 45000 / animate 600ms）
- [x] 布局动画结束后才 fit（`promiseOn('layoutstop')`），避免光晕被取景裁掉
- [x] cytoscape style 数组全部重写（圆点 + 类型色 + 状态光晕 + 细边）
- [x] label 只显示 id（圆点放不下双行；title 由侧栏展示）
- [x] 工具栏现代风（胶囊按钮 / 细体 logo / 半透明分隔线）

### Sidebar.svelte
- [x] 现代暗色面板（`#111` + 半透明白分隔线，宽度 400→360px）
- [x] 大写小字标签 + 胶囊按钮（保存 = 白底黑字高对比）
- [x] header 加类型色点（与画布节点配色呼应）
- [x] 状态标签去 emoji 化（纯文字）
- [x] 修复 `$props<T>()` 泛型不生效 → 改注解式 `let { ... }: Props = $props()`

### app.css
- [x] 背景 `#1e1e1e` → `#0a0a0a`，加抗锯齿

### chain_to_cytoscape.ts
- [x] 修复 `parent: string | null` 不匹配 `NodeDataDefinition.parent?: string`（`?? undefined`）

## 验收标准

**硬指标（已全过 ✅）：**
1. `npx vite build` ✅ 116 modules transformed，0 error
2. `npx svelte-check` ✅ 0 errors 0 warnings
3. 无新依赖（package.json 未动；cytoscape-dagre 成为死依赖，登记 P2 待清理）

**手动链路（待开发者验）：**
1. 启动 → 选 `G:\test1.x\test-data` → 5 圆点星团散布，g-001 明显大于叶子节点
2. 节点颜色：g-001 紫 / d-001 d-002 蓝 / t-001 青 / v-001 绿
3. 边是 1px 细白微曲线，带小箭头
4. 点节点 → 侧栏弹出（现代风 + 类型色点）→ 改 status 为 in_progress → 保存 → 该节点出现白色光晕
5. 改 failed → 节点变红 + 红光晕
6. 整体观感对照参考稿：像"数据库网络"，不像"流程图"

## 交付物

- commit `344456b`（4 files，+198/-124）
- 本任务卡

## 已知风险 / 遗留

| 风险 | 应对 |
|------|------|
| cose `randomize: true` 每次刷新位置会重新洗牌 | 短期可接受（动画过渡平滑）；若体验烦人，后续里程碑改 preset 记忆位置 |
| cytoscape-dagre 成死依赖 | P2，下次动 package.json 时 `npm uninstall` |
| 节点多时光晕 shadow 性能 | 5-50 节点无感；>200 节点再评估 |
| label 只显示 id，title 在画布不可见 | 侧栏可见；tooltip（qtip 插件）属新依赖，暂不引 |
