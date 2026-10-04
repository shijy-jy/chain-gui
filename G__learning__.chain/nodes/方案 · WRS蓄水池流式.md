---
id: 方案 · WRS蓄水池流式
title: 方案 · WRS蓄水池流式
parent: 局限 · 朴素SIR有偏且需两遍
rel: solves
tags:
- 方案
- 采用
- WRS
- reservoir
- 递进链
revision: 1
updated: 2026-09-08T20:17:33+08:00
---

> 触发：WRS 加权蓄水池采样；reservoir 流式算法

# 方案 · WRS 蓄水池流式（Weighted Reservoir Sampling）

## 解决了什么 / 没解决什么（递进链）

- **解决**：[局限 · 朴素SIR有偏且需两遍](局限 · 朴素SIR有偏且需两遍.md)——reservoir 把"两遍、存全部候选"压缩成"一遍、O(1) 空间"，流式 RIS 估计量与离线 SIR 同构无偏。
- **未解决（引出下一环）**：流式并入历史会让 M 无界增长 → [局限 · M无界累积冻住画面](局限 · M无界累积冻住画面.md)

## reservoir 数据结构（ReSTIR 的灵魂，三个字段）

```
reservoir = { y, w_sum, M }      // 当前样本、权重和、候选计数
```

## 算法

对每个新候选 $(y_i, w_i)$：

```
w_sum += w_i;  M += 1
以概率 w_i / w_sum 用 y_i 替换当前样本
```

## 流式不变式（归纳可证）

处理完任意前缀后，`y` 的分布 = **按权重从该前缀中选一个**的分布——与离线 SIR 在同一前缀上的结果**等价**。流式 RIS 估计量：

$$F = \frac{r.w_{sum}}{r.M}\cdot\frac{f(r.y)}{\hat p(r.y)}$$

## 为什么三字段就是 ReSTIR 的地基

时空复用 = **合并 reservoir**：把邻居像素/上一帧的 reservoir 当作"另一串候选"，流式并入自己的 reservoir——空间复用与时间复用在这三个字段上天然成立（M2 的核心操作）。

## 书目

- Chao. *A general purpose unequal probability sampling plan*. Biometrika 1982.（加权蓄水池采样原出处）
- Bitterli et al. SIGGRAPH 2020（ReSTIR 的 reservoir 形式化）。
