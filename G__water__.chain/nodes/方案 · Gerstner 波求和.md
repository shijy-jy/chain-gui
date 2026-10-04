---
id: 方案 · Gerstner 波求和
type: note
title: 方案 · Gerstner 波求和
parent: 问题：实时渲染逼真大尺度海面
status: none
rel: contains
tags:
- 方案
- 图形渲染
- 海洋
- 弃用
revision: 1
updated: 2026-09-08T20:22:49+08:00
---

> 触发：Gerstner 波；摆线波；次摆线

# 方案 · Gerstner 波求和

Gerstner（1804）摆线波：水质点绕圆周运动，波形是**次摆线**，波峰尖、波谷平——比正弦波更接近真实海面。叠加 $N$ 个波：

$$
\mathbf{x}' = \mathbf{x} - \sum_i A_i\,\hat{\mathbf{k}}_i \sin(\mathbf{k}_i\cdot\mathbf{x} - \omega_i t + \varphi_i)
$$
$$
y = \sum_i A_i \cos(\mathbf{k}_i\cdot\mathbf{x} - \omega_i t + \varphi_i)
$$

（$\hat{\mathbf{k}}_i$ 为波方向单位向量；水平位移让波峰处顶点聚集。）

## 为什么曾是主流方案

- 参数直观（振幅/波长/方向/相位），美术好调。
- 在**顶点着色器**逐顶点求和即可，无需任何纹理或预处理，管线极简。
- 经典实现：GPU Gems 1 第一章 *Effective Water Simulation from Physical Models*（Finch, 2004）。

## 在本工程的历史地位

- 早期水系统是 Gerstner 方案：根目录 `_Backup_WaterSplit/` 是旧方案备份，`fix_gerstner.py` 是当时调试 Gerstner 位移归一化的脚本。
- 后来迁移到 FFT 统计波谱法（`Assets/FFTVerify/`），Gerstner 被替代但仍在备份中留痕。

## 解决了什么 / 没解决什么

- 解决了：小水体、少量波的实时渲染；波峰尖/波谷平的形态。
- 没解决：大场景 + 高频细节的性能与真实度问题 → 见 局限 · Gerstner 波求和的瓶颈。
