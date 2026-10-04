---
id: 方案 · 多次散射 LUT 不动点迭代
type: note
title: 方案 · 多次散射 LUT 不动点迭代
parent: 局限 · 单次散射缺失多次散射能量
status: none
rel: solves
tags:
- 方案
- 采用
- 图形渲染
- 大气
revision: 1
updated: 2026-09-08T20:22:49+08:00
---

> 触发：多次散射 LUT；不动点迭代；Bruneton

# 方案 · 多次散射 LUT 不动点迭代

**解决**：单次散射在地平线/日落处缺失的多次散射能量。Bruneton 2017 改进版 + 工程做的**各向同性简化**（`MultiScatterLUTGen.compute`）。

## 不动点方程

多次散射辐射亮度满足（源项里含自身 → 不动点问题）：

$$
L_{ms}(p)=\frac{1}{4\pi}\iint \sigma_s(q)\,\Big[I_{sun}\,T_{sun}(q) + 4\pi\,L_{ms}(q)\Big]\,T(p\to q)\,ds\,d\omega
$$

- 源 = 单次散射的太阳项 + 上轮 $L_{ms}$ 的各向同性积分（$4\pi$ 系数来自"把方向辐射亮度折合成各向同性源"）；
- 迭代收敛：**第 1 轮只放太阳源（= 单散射），第 2 轮把第 1 轮结果当源**……工程做 **2 轮**。

## 工程实现

- **LUT 参数化**：32×32，$x=\mu_s$（太阳天顶余弦，sqrt 映射同透射率 LUT）、$y=\rho/H$；
- **积分采样**：方向半球 8×16（θ×φ）× 路径 16 步，每步查透射率 LUT 得太阳项、查上轮 LUT 得多散射项；
- **两个 kernel**：`CSMultiScatterInit`（只放太阳源 → 写入 Prev）、`CSMultiScatterIterate`（加入上轮 $4\pi L_{ms}$ → 写入最终 LUT）；
- **乒乓缓冲**：`_MultiScatterLUTPrev` 可写 + `_MultiScatterLUTPrevRead` 只读孪生纹理（`RWTexture2D` 不支持 `SampleLevel`，见 大气模块工程踩坑）；
- 存储 $L_{ms}/I_{sun}$（单位太阳辐照度），渲染端乘回 $I_{sun}$。

## 渲染端接入（`SingleScattering` 每步加一项）

$$
\text{inscatter}\ +\!=\ I_{sun}\cdot \sigma_s(h)\cdot LUT_{ms}(h,\mu_s)\cdot T_2\cdot ds
$$

各向同性 → 不需要乘相位函数，直接加散射系数 × LUT 值。`_Atm_MultiScatterStrength` 可调（1=物理）。

## 解决了什么 / 没解决什么

- 解决：地平线辉光、日落雾霭、太阳晕显著增强，天空"活"起来。
- 没解决：各向同性近似与地面缺失 → 见 局限 · 各向同性近似与未计地面。

## 参考

- Bruneton, E. *Precomputed Atmospheric Scattering: improved implementation*, 2017.
