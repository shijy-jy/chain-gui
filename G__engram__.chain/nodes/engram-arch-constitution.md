---
id: engram-arch-constitution
type: note
title: Engram · 分层宪法与依赖方向
parent: engram-framework
status: none
tags:
- Engram
- 架构
- 宪法
- 依赖方向
code_map: ../test1.x/crates/engram-core/src/ops
revision: 1
updated: 2026-09-09T08:15:27+08:00
---

> 触发：Engram 宪法；Engram 分层；依赖方向；唯一写路径

# Engram · 分层宪法与依赖方向

来源 `ARCHITECTURE.md`（不可逾越条款，摘录自《架构设计终版 v1.0》§2；条款变更必须走 ADR）。

## cargo workspace 依赖方向（宪法第 1 条）

```text
engram-gui（Tauri 桌面壳）─┐
engram-mcp（MCP stdio 服务）─┼─▶ engram-core ─▶ .chain/nodes/*.md
engram-cli（维护工具）      ─┘   （唯一知道规则的地方）
```

`engram-core` 零 Tauri/MCP 依赖，一切入口都是适配器。workspace members：engram-core / engram-mcp / engram-gui / engram-cli。

## 九条宪法速记

1. **依赖方向**：入口 → core，不可逆
2. **事实源与派生物**：节点 md 是唯一事实源；统计/索引/审计/代码骨架全可重建、可删除，绝不写进 YAML 事实源
3. **关系语义**：rel 只三类型（contains/solves/alternative），细节走 rel_desc；词表变更须同步指南 + MCP 校验 + GUI 线型三处
4. **写入守门**：一切写入必经 core（D2 词表 / D3 乐观锁 / D4 提示 / tmp+rename 原子写 / 串行队列）；入口不得直写
5. **GUI 零破坏**：加性改动允许（新开关/视图/参数）；破坏性改动（改默认交互/布局语义/删工具）禁止
6. **检索降级链**：任何检索增强失效必须退化为关键词检索并显式声明 degraded，检索工具永远可用
7. **错误码契约**：稳定错误码 + 文案分离（文案可改码不可改）；清单见「扫描层与写入守门」节点
8. **工具即契约**：MCP 工具增删改 → 更新 golden 契约文件 + CHANGELOG 记录 + 工具契约版本递增
9. **数据 schema 版本**：`.chain/.schema` major.minor；major 变更走幂等迁移；旧软件拒开更高 major

## 落地路径

2.8.0 完成 workspace 化 + engram-core 下沉（唯一写路径重构）；2.10.0 完成 §7 全部错误码 + §9 迁移工具（`engram-cli migrate` 五相幂等）。

