"use strict";
var __defProp = Object.defineProperty;
var __getOwnPropDesc = Object.getOwnPropertyDescriptor;
var __getOwnPropNames = Object.getOwnPropertyNames;
var __hasOwnProp = Object.prototype.hasOwnProperty;
var __export = (target, all) => {
  for (var name in all)
    __defProp(target, name, { get: all[name], enumerable: true });
};
var __copyProps = (to, from, except, desc) => {
  if (from && typeof from === "object" || typeof from === "function") {
    for (let key of __getOwnPropNames(from))
      if (!__hasOwnProp.call(to, key) && key !== except)
        __defProp(to, key, { get: () => from[key], enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable });
  }
  return to;
};
var __toCommonJS = (mod) => __copyProps(__defProp({}, "__esModule", { value: true }), mod);

// src/lib/chain_to_cytoscape.ts
var chain_to_cytoscape_exports = {};
__export(chain_to_cytoscape_exports, {
  NODE_TYPE_COLOR: () => NODE_TYPE_COLOR,
  NODE_TYPE_LABEL: () => NODE_TYPE_LABEL,
  chainToElements: () => chainToElements
});
module.exports = __toCommonJS(chain_to_cytoscape_exports);
var NODE_TYPE_LABEL = {
  goal: "\u76EE\u6807",
  design: "\u8BBE\u8BA1",
  task: "\u4EFB\u52A1",
  verification: "\u9A8C\u8BC1",
  note: "\u7B14\u8BB0"
};
var NODE_TYPE_COLOR = {
  goal: "#a78bfa",
  design: "#60a5fa",
  task: "#22d3ee",
  verification: "#34d399",
  note: "#94a3b8"
};
function displayLabel(type, title) {
  const max = 20;
  const t = title.length > max ? `${title.slice(0, max)}\u2026` : title;
  if (type === "note") return t;
  return `${NODE_TYPE_LABEL[type]} \xB7 ${t}`;
}
function chainToElements(snap) {
  const elements = [];
  const nodes = snap.nodes;
  const n = nodes.length;
  const rootId = snap.manifest.root;
  const R = 180 + n * 5;
  nodes.forEach((node, i) => {
    let x = 0;
    let y = 0;
    if (n > 1 && node.id !== rootId) {
      const ang = i / (n - 1) * Math.PI * 2;
      x = Math.cos(ang) * R;
      y = Math.sin(ang) * R;
    }
    elements.push({
      data: {
        id: node.id,
        label: displayLabel(node.type, node.title),
        nodeType: node.type,
        nodeStatus: node.status,
        chainParent: node.parent
        // 注意：不能用 `parent` 字段名——那是 cytoscape 保留字段（compound 复合节点），会把子节点渲染进父节点内部撑出巨型容器；chain 协议的父子关系由 edge 表达，这里仅保留信息备查
      },
      position: { x, y }
    });
  });
  for (const edge of snap.edges) {
    const src = nodes.find((n2) => n2.id === edge.parent);
    const tgt = nodes.find((n2) => n2.id === edge.child);
    const srcColor = src ? NODE_TYPE_COLOR[src.type] : "#888888";
    const tgtColor = tgt ? NODE_TYPE_COLOR[tgt.type] : "#888888";
    elements.push({
      data: {
        id: `${edge.parent}->${edge.child}`,
        source: edge.parent,
        target: edge.child,
        gradColors: `${srcColor}, ${tgtColor}`,
        tgtColor
      }
    });
  }
  return elements;
}
