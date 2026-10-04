---
id: 海面能量谱：JONSWAP
type: note
title: 海面能量谱：JONSWAP
parent: 方案 · FFT 统计波谱法
status: none
rel: contains
tags:
- 图形渲染
- 海洋
- 频谱
revision: 1
updated: 2026-09-08T20:22:49+08:00
---

> 触发：JONSWAP 海面能量谱；Hasselmann

# 海面能量谱：JONSWAP

FFT 法第一步要回答"**每个波数上放多少能量**"。工程用 JONSWAP 全向谱（Hasselmann 等，1973 年北海联合观测计划）：

$$
S_J(\omega)=\alpha\, g^2\, \omega^{-5}\,
\exp\!\left[-1.25\left(\frac{\omega_p}{\omega}\right)^{4}\right]\,\gamma^{r},
\qquad
r=\exp\!\left[-\frac{(\omega-\omega_p)^2}{2\sigma^2\omega_p^2}\right]
$$

- $\sigma=0.07$（$\omega\le\omega_p$）/ $0.09$（$\omega>\omega_p$）
- $\gamma$：峰增强因子（1~7，工程 `peakEnhancement`，默认 3~5），控制谱峰尖度
- $\omega_p$：峰频，$\alpha$：能量尺度——都由**风速 $U$、风区 $F$** 推出（工程 `JonswapAlpha`/`JonswapPeakFrequency`）：

$$
\alpha = 0.076\left(\frac{gF}{U^2}\right)^{-0.22},
\qquad
\omega_p = 22\left(\frac{UF}{g^2}\right)^{-0.33}
$$

## 工程里的三个附加处理

1. **TMA 浅水修正**（Bouws 1985，`TMACorrection`）：$\omega_H=\omega\sqrt{d/g}$ 分段修正谱形，$d$ 为水深（工程 $d=10$m）。
2. **短波衰减**（`ShortWaveFade`）：$e^{-(l\,k)^2/10000}$，砍掉过短波 → 防高频混叠。
3. **方向分布**：$S_J(\omega)$ 还要乘方向扩散函数（见 方向扩散函数 节点）才是二维方向谱。

## 频率谱 → 波数谱换算（关键一步）

IFFT 在**波数** $\mathbf{k}$ 域工作，能量守恒要求 $S(\mathbf{k})\,d^2k = S(\omega,\theta)\,d\omega\,d\theta$，且 $d^2k = k\,dk\,d\theta$，故：

$$
S(\mathbf{k}) = S(\omega,\theta)\,\left|\frac{d\omega}{dk}\right|\,\frac{1}{k}
$$

工程里 $h_0$ 的缩放因子正是它：

$$
h_0(\mathbf{k}) = (\xi_r,\ \xi_i)\cdot\sqrt{S(\mathbf{k})\cdot 2\cdot dk^2}
\qquad\text{（}\xi\text{ 为高斯随机数，dk}=\frac{2\pi}{L}\text{）}
$$

其中 $d\omega/dk$ 由色散关系求导（`DispersionDerivative`），$L$ 为层长度尺度。

## 与 Phillips 谱的关系

Tessendorf 原始论文用 **Phillips 谱** $P_h(\mathbf{k})=A\frac{e^{-1/(kL)^2}}{k^4}|\hat{\mathbf{k}}\cdot\hat{\mathbf{w}}|^2$（带风向余弦），参数少、简单；JONSWAP 更贴近实测海浪能量分布（有 $\gamma$ 峰、可调风区）。本工程选 JONSWAP + 独立方向扩散，真实度更高。

## 参考

- Hasselmann, K. et al. *Measurements of wind-wave growth and swell decay during the Joint North Sea Wave Project (JONSWAP)*, 1973.
- Bouws, E. et al. *Similarity of the wind wave spectrum in finite depth water (TMA spectrum)*, 1985.
