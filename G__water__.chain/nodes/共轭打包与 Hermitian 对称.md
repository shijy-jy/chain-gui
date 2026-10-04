---
id: 共轭打包与 Hermitian 对称
type: note
title: 共轭打包与 Hermitian 对称
parent: 方案 · FFT 统计波谱法
status: none
rel: contains
tags:
- 图形渲染
- 海洋
- FFT
- 算法
revision: 1
updated: 2026-09-08T20:22:49+08:00
---

> 触发：共轭打包；Hermitian 对称；实数频谱

# 共轭打包与 Hermitian 对称

## 原理

高度场 $h(\mathbf{x})$ 是**实数**函数，其频谱满足 **Hermitian 对称**：

$$
h(-\mathbf{k}) = h^*(\mathbf{k})
$$

即负波数的频谱完全由正波数决定（冗余一半）。时间演化公式

$$
h(\mathbf{k},t) = h_0(\mathbf{k})\,e^{i\omega t} + h_0^*(-\mathbf{k})\,e^{-i\omega t}
$$

正好需要一对：$h_0(\mathbf{k})$ 和 $h_0^*(-\mathbf{k})$。

## 工程的打包方式（`CS_PackSpectrumConjugate`）

在一个 texel 里存四个通道：

$$
\text{texel}(\mathbf{k}) = \big[\ \underbrace{\Re h_0(\mathbf{k}),\ \Im h_0(\mathbf{k})}_{.rg},\ \underbrace{\Re h_0(-\mathbf{k}),\ -\Im h_0(-\mathbf{k})}_{.ba}\ \big]
$$

`.ba` 存的是 $h_0^*(-\mathbf{k})$（虚部取反 = 共轭）。镜像采样用 $(N-x)\bmod N$ 索引。

## 收益

1. **时间演化只读一个 texel**：`CS_UpdateSpectrum` 里 4 通道一次拿全，无需跨 texel 采样镜像点；
2. **省内存/带宽**：不用为负波数单独存一张镜像频谱；
3. **零开销**：对称信息本来就是冗余的，打包只是换存储布局，不丢信息。

## 注意事项

- 频谱网格以 $\mathbf{k}=(id-N/2)\Delta k$ 为中心（$id=0$ 对应 $-N/2$），空间域会产生 $(-1)^{x+y}$ 交替符号，`CS_AssembleTextures` 里的 `Permute` 乘 $(1-2((x+y)\bmod2))$ 正好抵消。
- 直流分量 $\mathbf{k}=0$ 与 Nyquist 频率处的对称性退化，工程对 $k<0.0001$ 时 $1/k$ 特殊处理（置 1），避免除零。

## 相关节点

- 色散关系与时间演化（打包数据的使用方）
- Stockham IFFT（变换载体）
