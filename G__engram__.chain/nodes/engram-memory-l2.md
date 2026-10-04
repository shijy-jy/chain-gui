---
id: engram-memory-l2
type: note
title: Engram · 记忆层 L2 六件套
parent: engram-framework
status: none
tags:
- Engram
- 记忆层
- recall
- 嵌入
code_map: ../test1.x/crates/engram-core/src/retrieval.rs
revision: 1
updated: 2026-09-09T08:09:11+08:00
---

> 触发：Engram 记忆层；recall 检索阶梯；嵌入索引；ACT-R 强度

# Engram · 记忆层 L2 六件套

M6'/M7'/M8' + 补丁 1 落地（2.9.0 / 2.10.0）。全部派生物：可重建、可删除（宪法第 2 条）。

| 模块 | 机制 |
|---|---|
| `embed` | fastembed 6.0.3 + BGE-small-zh-v1.5（512 维）；安装目录 models 旁路优先 → LOCALAPPDATA 兜底；加载失败走降级链 |
| `index` | `.chain/index/`（meta.json + embeddings.bin）；content_hash 变更检测 + 缓存失效指纹（外部 CLI reindex 后长驻 MCP 立即可见）；upsert/remove/flush |
| `stats` | `.chain/stats.json`：双时钟（全局记忆时钟序数 + 墙钟）+ TouchKind + ACT-R 强度 + gap 截断 + calibrate |
| `retrieval` | recall 阶梯：向量（余弦 top-k，两档阈值 0.35/0.2）→ 冷启动（创建时间 + 图谱度数）→ 关键词降级（degraded:true 显式声明）；derived ×0.85；归档默认过滤，include_archived 找回；stale 候选按需热重嵌 |
| `consolidate` | BFS 连通分量聚类 → [蒸馏] 骨架节点（模板化，逐条来源引用；非 LLM 摘要）；dry_run 默认 true |
| `audit` | audit.jsonl append-only：create/update/link/archive/unlink/consolidate/freeze/migrate + recall/dup 决策留痕；失败不阻断 |

## ACT-R 强度（补丁 1 关键修复）

`S = ln(Σ (now−t_j)^(−d))`，**时间轴 = 记忆时钟序数**（不是墙钟秒——旧版「昨天用过 < 从未用过」硬伤），负值 clamp 0（触达永不惩罚），d 先验 0.5；每节点触达窗口 50。

## 参数与反馈（补丁 1）

7 参数外置 `stats.json params` 区（recall 阈值 0.35/0.2、dup_cosine 0.9、derived_weight 0.85、actr_d 0.5、archive_days 90、calibrate_window 50）；反馈区有界样本（正负样本 / 重复真假阳性 / 归档误判 / 蒸馏质量）。**迭代循环只采集不自动调参，参数变更需人确认（治理权在人）**。

