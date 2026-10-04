---
id: 方案 · FFT 统计波谱法
type: note
title: 方案 · FFT 统计波谱法
parent: 局限 · Gerstner 波求和的瓶颈
status: none
rel: solves
tags:
- 方案
- 采用
- 图形渲染
- 海洋
revision: 1
updated: 2026-09-08T20:22:49+08:00
---

> 触发：FFT 统计波谱法；Tessendorf；Gerstner 替代

# 方案 · FFT 统计波谱法

**解决**：Gerstner 逐波求和的性能与真实度瓶颈。源头是 Tessendorf, *Simulating Ocean Water*（SIGGRAPH 2001）。

## 核心思想

把海面当成**高斯随机场**，不逐波求和，而是：

1. 在**频域**用能量谱 $S(\mathbf{k})$ 构造初始频谱 $h_0(\mathbf{k})$（高斯随机数 × 谱的开方）；
2. 按色散关系 $\omega(\mathbf{k})$ 旋转相位得到 $h(\mathbf{k},t)$；
3. **IFFT** 一次变换回空间域，得到整个高度/位移场。

$$
h(\mathbf{x},t) = \sum_{\mathbf{k}} h(\mathbf{k},t)\, e^{i\mathbf{k}\cdot\mathbf{x}}
$$

复杂度 $O(N\log N)$，**与波数无关**：$1024^2$ 的频谱一次 IFFT 相当于"无穷多个波"的叠加，频谱里每个 bin 就是一个波分量。

## 本工程每帧管线（6 个 kernel，固定 1024×1024）

| 步骤 | Kernel | 内容 |
|---|---|---|
| 1 | `CS_InitializeSpectrum` | 4 层 × 2 组 JONSWAP 谱 → $h_0(\mathbf{k})$ |
| 2 | `CS_PackSpectrumConjugate` | 共轭打包（Hermitian 对称） |
| 3 | `CS_UpdateSpectrum` | 相位旋转 + 解析推导位移/斜率频谱 |
| 4 | `CS_HorizontalIFFT` | Stockham IFFT 按行（1×1024 线程组） |
| 5 | `CS_VerticalIFFT` | 按列 |
| 6 | `CS_AssembleTextures` | 组装位移/斜率/泡沫纹理 + 法线 |

## 关键副产品（全部解析推导，无差分）

- **位移场**：$D_x=-i\frac{k_x}{k}h,\ D_z=-i\frac{k_z}{k}h$（顶点变形、浮力）
- **斜率场**：$\partial h/\partial x=ik_x h,\ \partial h/\partial z=ik_z h$（法线重建）
- **Jacobian**：二阶导组合，折叠检测 → 泡沫

## 实现文件

- `Assets/FFTVerify/FFTOcean_Complete.cs` + `.compute`（驱动 + 管线）
- `Assets/FFTVerify/FFT_SpectrumVerify.cs` + `.compute`（管线 + 验证输出）
- `Assets/FFTVerify/FFTOcean_Render.cs` + `.shader`（渲染侧）

## 解决了什么 / 没解决什么

- 解决：$O(N\log N)$ 全谱变换；位移/斜率/泡沫一体化产出；统计真实性。
- 没解决：静态统计谱的固有边界 → 见 局限 · FFT 统计波谱的边界。

## 参考

- Tessendorf, J. *Simulating Ocean Water*. SIGGRAPH Course Notes, 2001.
- ChenHanMK1/FFT-Ocean-Code（GitHub 参考实现）
