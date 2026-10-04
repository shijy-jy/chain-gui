---
id: 透射率 LUT 的参数化映射
type: note
title: 透射率 LUT 的参数化映射
parent: 方案 · 透射率 LUT 预计算
status: none
rel: contains
tags:
- 图形渲染
- 大气
- 实现细节
revision: 1
updated: 2026-09-08T20:22:49+08:00
---

> 触发：透射率 LUT 参数化；r mu 到 UV

# 透射率 LUT 的参数化映射

把 $(r,\mu)$ 参数域映射到 LUT UV，映射质量决定精度分布。工程实现（`GetTransmittanceLutUV` / `GetTransmittanceLutRMu` 互为逆映射）。

## 正向映射（写入/采样）

$$
x_r = \frac{\rho}{H},\quad \rho=\sqrt{r^2-R^2},\quad H=\sqrt{R_{atm}^2-R^2}
$$

$$
x_\mu = 0.5+0.5\,\mathrm{sign}(\mu)\sqrt{|\mu|}
$$

- $x_r\in[0,1]$：从地表（$\rho=0$）到大气层切点高度（$\rho=H$）线性分布；
- $x_\mu$ 用 **sqrt 非线性**：把采样密度向 **$\mu\approx0$（地平线方向）**集中——透射率在 $\mu$ 过零处变化最剧烈（光程从"最短"跳变到"贴地最长"），分辨率要花在这里。

## 逆映射（生成端）

$$
\mu = \mathrm{sign}(x_\mu-0.5)\cdot\big(2|x_\mu-0.5|\big)^2,\qquad
r = \sqrt{(x_r H)^2 + R^2}
$$

生成端取像素中心 `(id+0.5)/Size` 反解 $(r,\mu)$，构造射线位置与方向后 ray march。

## 部分路径透射率近似（`TransmittanceToDistance`）

雾效需要"相机到距离 $d$ 处"的透射率，而 LUT 只存"到大气层顶"：

- $d\ge$ 到大气层顶距离：直接用 LUT 值；
- 否则 `lerp(1, T_atm, d/distToAtm)`——线性混合近似（非 Bruneton 改进版对部分路径的专用参数化，但工程上足够）。

## 一致性纪律

UV 映射在 `TransmittanceLUT.compute`（生成）、`AtmosphereLib.hlsl`（采样）、`MultiScatterLUTGen.compute`/`SkyCubemapGen.compute`（再采样）中**逐字复制**——参数化不统一会立刻产生地平线处难以排查的色带。

## 相关节点

- 方案 · 透射率 LUT 预计算（LUT 本体）
- 大气雾与海洋-大气桥（部分路径透射率的使用方）
