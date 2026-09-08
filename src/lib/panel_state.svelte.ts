// v2.6 信息栏面板共享状态：模块级 $state——Sidebar 组件读写布局尺寸，
// App 读取宽度为画布预留空间（padding-right），保证右下角/右上角悬浮控件不被常驻侧栏压住。
export const panel = $state({
  width: 380,
  bodyH: 300,
  evidenceH: 136,
  logH: 112,
  // v2.12 M-Code 骨架面板（加性）
  codeH: 220,
  bodyOpen: true,
  evidenceOpen: true,
  logOpen: true,
  codeOpen: false,
});

export const SIDEBAR_COLLAPSED_WIDTH = 44;   // 收起态细条宽度（与 Sidebar.svelte CSS 保持一致）
