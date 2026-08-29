import type { ElementDefinition } from 'cytoscape';
import type { ChainSnapshot } from './types';

export function chainToElements(snap: ChainSnapshot): ElementDefinition[] {
  const elements: ElementDefinition[] = [];

  for (const node of snap.nodes) {
    elements.push({
      data: {
        id: node.id,
        label: `${node.id}\n${node.title}`,
        nodeType: node.type,
        nodeStatus: node.status,
        chainParent: node.parent,  // 注意：不能用 `parent` 字段名——那是 cytoscape 保留字段（compound 复合节点），会把子节点渲染进父节点内部撑出巨型容器；chain 协议的父子关系由 edge 表达，这里仅保留信息备查
      },
    });
  }

  for (const edge of snap.edges) {
    elements.push({
      data: {
        id: `${edge.parent}->${edge.child}`,
        source: edge.parent,
        target: edge.child,
      },
    });
  }

  return elements;
}
