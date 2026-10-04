---
id: FFT海洋渲染
type: note
title: FFT海洋渲染
parent: 知识库索引
status: none
tags:
- 图形渲染
- 海洋
- Unity
- 主题中心
revision: 2
updated: 2026-09-09T11:44:42+08:00
code_map: D:/unity/unityProjectLocation/water/Assets/FFTVerify
---

> 触发：FFT 海洋渲染主题；unity 海面；主题中心

# FFT海洋渲染

FFT 海洋渲染主题中心（hub）。汇总从 `D:\unity\unityProjectLocation\water` 工程（`Assets/FFTVerify/`）研读得到的所有知识点。

## 工程实现组成（三件套）

| 层 | 文件 | 职责 |
|---|---|---|
| 模拟 | `FFTOcean_Complete.cs` + `.compute` | 驱动脚本 + 6 kernel 的频谱管线（初始化→共轭→演化→IFFT→组装） |
| 模拟/调试 | `FFT_SpectrumVerify.cs` + `.compute` | 同管线 + 数值验证输出（`_DebugOutput`/`_BuoyancyData`/`_VariationMask`） |
| 渲染桥 | `FFTOcean_Render.cs` | 建网格、建材质、每帧把纹理与参数喂给材质 |
| 材质 | `FFTOcean_Render.shader` | 曲面细分 + 位移采样 + Beckmann/Smith/Schlick 光照 + 泡沫 + 雾 |

参考实现：**ChenHanMK1/FFT-Ocean-Code**（GitHub），工程注释反复声明"照搬参考实现的渲染逻辑"。

## 子节点

- 问题：实时渲染逼真大尺度海面
- 方案 · Gerstner 波求和 → 局限 · Gerstner 波求和的瓶颈
- 方案 · FFT 统计波谱法（递进主线，solves 上述局限）
  - 海面能量谱：JONSWAP
  - 方向扩散函数
  - 色散关系与时间演化
  - 共轭打包与 Hermitian 对称
  - Stockham IFFT
  - 频域解析求导：位移与斜率
  - Jacobian 折叠检测与泡沫
  - 多层频谱叠加与平铺
  - 水面材质渲染
  - 方案 · 频谱数值验证工具链
- 局限 · FFT 统计波谱的边界

## 主要参考书目

- Tessendorf, J. *Simulating Ocean Water*. SIGGRAPH Course Notes, 2001.（FFT 统计波谱法源头）
- Hasselmann, K. et al. *Measurements of wind-wave growth and swell decay during JONSWAP*, 1973.（JONSWAP 谱）
- Finch, M. *Effective Water Simulation from Physical Models*. GPU Gems 1, Chapter 1, 2004.（Gerstner 波）
- ChenHanMK1/FFT-Ocean-Code（本工程参考实现，GitHub）

