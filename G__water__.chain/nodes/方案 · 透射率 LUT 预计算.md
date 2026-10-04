---
id: 方案 · 透射率 LUT 预计算
type: note
title: 方案 · 透射率 LUT 预计算
parent: 局限 · 嵌套光线步进的性能瓶颈
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

> 触发：透射率 LUT 预计算；Bruneton Neyret

# 方案 · 透射率 LUT 预计算

**解决**：嵌套光线步进的 O(N²) 瓶颈。Bruneton & Neyret, *Precomputed Atmospheric Scattering*（EGSR 2008）的核心思想。

## 关键洞察：透射率是 2D 函数

大气球对称 ⇒ 空间任意一点 $p$（高度 $r$）沿任意方向 $\mathbf{d}$ 到大气层顶的透射率，只取决于：

$$
T(r,\ \mu),\qquad \mu = \mathbf{d}\cdot\hat{\mathbf{r}}
$$

**4D 问题（点×方向）塌缩成 2D**。于是：

1. **预计算**：`TransmittanceLUT.compute` 把 $(r,\mu)$ 网格（256×128）每个 texel 用 256 步 ray march 算一次 $T=\exp(-\tau)$，存 ARGBFloat 纹理（`_Atm_TransmittanceLUT`）；
2. **运行时**：$T_{sun}(p)$ 变成**一次纹理查表**（`TransmittanceToAtmosphere`），视线积分从"每步嵌套 64 步"降到"每步一次查表"；
3. **惰性重算**：LUT 只依赖大气参数（不依赖太阳方向、相机位置）→ 参数不变永远不用重算。

## 渲染端单次散射（`SingleScattering`）

视线 32 步中，每步：

- $T_1$ = 查表得太阳透射率（被行星遮挡时置 0）；
- $T_2$ = 沿视线累积的光学深度指数（Beer-Lambert）；
- 累加 $I_{sun}\cdot T_1\cdot\sigma_s P\cdot T_2\cdot ds$。

## 工程实现要点

- LUT 生成在 `AtmosphereController.Start()` 一次（参数同步后）；`SetConfig` 或参数变化时重生成；
- 单位：km、系数 km⁻¹（C# 端 ×1000）；
- LUT 经 `Shader.SetGlobalTexture` 全局共享，天空盒/雾/Cubemap/海洋桥全部读同一张。

## 解决了什么 / 没解决什么

- 解决：内层积分变成查表，实时天空成为可能；数学与物理基线一致。
- 没解决：只有**单次散射** → 见 局限 · 单次散射缺失多次散射能量。

## 参考

- Bruneton, E. & Neyret, F. *Precomputed Atmospheric Scattering*. EGSR 2008.
