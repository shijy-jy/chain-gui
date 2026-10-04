---
id: 方案 · 蓄水池时空合并与M钳制
title: 方案 · 蓄水池时空合并与M钳制
parent: 局限 · M无界累积冻住画面
rel: solves
tags:
- 方案
- M2
- reservoir
revision: 1
updated: 2026-09-08T20:17:33+08:00
---

> 触发：蓄水池合并；M 钳制偏差；时空复用合并

# 方案 · 蓄水池时空合并与 M 钳制（已展开 · 通往 ReSTIR DI）

> ✅ 已展开（2026-08-31）：reservoir 合并与 M 钳制的完整数学已随 ReSTIR 学习写入 [方案 · ReSTIR时空复用](方案 · ReSTIR时空复用.md)（推导版 §3.2 时间复用与 M-cap 偏差来源、§3.3 空间复用 target-ratio、辩证卡⑮–⑳）。本节点保留递进链上的"承上启下"定位与代价摘要。

## 解决了什么（对应 [局限 · M无界累积冻住画面](局限 · M无界累积冻住画面.md)）

- **合并（merge）**：把另一个 reservoir（邻居像素/上一帧）当作一串候选流式并入：$w_{sum}\leftarrow w_{sum}+w'_{sum}$、$M\leftarrow M+M'$，样本按 $w'_{sum}/(w_{sum}+w'_{sum})$ 概率被替换——时空复用的数学操作。
- **M 钳制（M-cap）**：$M\le M_{cap}$ 时把 $w_{sum}$ 重缩放为 $w_{sum}\cdot M_{cap}/M$——让新候选重新有机会被选中，画面跟上变化。

## 引入的代价（诚实预告，已在 ReSTIR 节点兑现）

重缩放丢弃的那部分权重和 = **偏差**。这是"有偏但一致"的又一实例：$M_{cap}$ 越大偏差越小（极限无偏），但历史权重越顽固（新样本越难进入）。**M-cap 是方差↔偏差账本上的一个旋钮**（辩证卡⑨⑱），偏差的"成对结构断裂"分析见 [方案 · ReSTIR时空复用](方案 · ReSTIR时空复用.md) §3.2。

## 书目

- Bitterli et al. SIGGRAPH 2020（temporal/spatial reuse 与 M-cap）。
- Bitterli et al. *ReSTIR 深入浅出*（2021 课程笔记：M-cap 偏差校正）。
