/// <reference types="svelte" />
/// <reference types="vite/client" />

// d3-force-3d 未随包提供类型声明（开源库 vasturiano/d3-force-3d）：
// 本项目只使用 forceSimulation/forceLink/forceManyBody/forceCenter 四个入口，
// 且在 Graph3D 内以 any 形式调用 —— 这里给出宽松声明即可。
declare module 'd3-force-3d';
