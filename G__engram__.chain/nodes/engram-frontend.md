---
id: engram-frontend
type: note
title: Engram · 前端图谱与涟漪交互
parent: engram-framework
status: none
tags: [Engram, 前端, Svelte, cytoscape]
---

> 触发：Engram 前端；Svelte 图谱；涟漪交互；App.svelte

# Engram · 前端图谱与涟漪交互

技术栈：Svelte 5（runes）+ TypeScript + Vite 5 + Cytoscape.js + markdown-it + KaTeX + Mermaid。

## 结构（`src/`）

- `App.svelte`：主组件——图谱、波纹水面层、力导向布局、搜索、工具栏、编辑侧栏
- `lib/ripple.ts`：涟漪 BFS 分层（单击节点 = 波源，同心环向全场扩散，按层深亮度衰减 d0=1.0 → d1=0.8 → d2=0.4，滑条可调）
- `lib/chain_to_cytoscape.ts`：ChainSnapshot → cytoscape 图数据（edges rel 线型：contains 实线 / solves 虚线 / alternative 点线）
- `lib/body_render.ts`：正文渲染 markdown-it + KaTeX
- `lib/Sidebar.svelte`：节点编辑侧栏（标题/状态/标签/正文/证据/日志；双击节点打开，点空白收起为右缘细条）
- `lib/panel_state.svelte.ts`：抽屉/面板状态（互斥）
- `components/`：WorkspaceSidebar（多工作区管理）/ StatusBar（校验状态 + 详情抽屉）/ CreateNodeDialog
- `types.ts`：与 Rust Node/ChainSnapshot 同构的 TS 类型（archived/derived/frozen/code_map 同步演进）

## 流畅度要点（提交 07014c0）

位置缓存（静止大图零矩阵开销）→ 每帧样式旁路收窄到波源呼吸（全图节点每帧写 width/height 是卡顿主因）→ 波前 class 增量点亮 → 固定描边色 + globalAlpha → 闲置零绘制 → 时长节流 30fps。

## GUI 零破坏纪律（宪法第 5 条）

新开关/视图/参数可加；徽标 / 归档视图开关 / Mermaid 面板 / 重嵌按钮全部默认关闭。M-Code 骨架面板 Mermaid 渲染失败时降级纯文本。
