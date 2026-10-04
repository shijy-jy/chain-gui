---
id: 光照模型与微表面BRDF
type: note
title: 光照模型与微表面 BRDF
parent: 光照与PBR
status: none
tags:
- BRDF
- GGX
- 微表面
- 能量守恒
- KullaConty
created: 2026-09-21T21:40:00+08:00
updated: 2026-09-22T12:16:50+08:00
revision: 2
---

> 触发：BRDF 是什么；GGX 公式；为什么白炉测试要等于 1；微表面理论；能量补偿

# 光照模型与微表面 BRDF

## 从简单模型到微表面

| 模型 | 形式 | 适用与代价 |
|---|---|---|
| Lambert | $f_d=\rho/\pi$ | 只做漫反射，无高光；$\pi$ 是为了能量归一化，不是笔误 |
| Phong / Blinn-Phong | $(\mathbf{r}\cdot\mathbf{v})^{\alpha}$ / $(\mathbf{n}\cdot\mathbf{h})^{\alpha}$ | 经验公式，参数好调；**不守恒、没有物理意义**，风格化项目里依然合法 |
| 微表面（Cook-Torrance） | $f=\dfrac{D(\mathbf{h})G(\mathbf{l},\mathbf{v})F(\mathbf{v},\mathbf{h})}{4(\mathbf{n}\cdot\mathbf{l})(\mathbf{n}\cdot\mathbf{v})}$ | 物理正确的基础；面试主战场 |

三件套各自的角色：$D$ 决定**高光形状**（GGX 长尾 vs Beckmann 高斯），$G$（几何遮蔽，常用 Smith）决定**掠射角**表现，$F$（菲涅耳，Schlick 近似 $F=F_0+(1-F_0)(1-\mathbf{v}\cdot\mathbf{h})^5$）决定**边缘反射与材质辨识度**。

## 能量守恒：为什么白炉测试必须等于 1

把粗糙度设为任意值、光照设为均匀白环境时，一个**保守**的 BRDF 应把入射能量原样反射回去——渲染结果应是**均匀的白**（能量 ≈ 1.0）。单次散射 GGX 做不到：微表面之间互相遮挡的多次反弹被丢掉，**粗糙度越高中能量损失越多**（实测只剩 **0.335**）。

- 补偿方法：**Kulla-Conty** 多散射项——预计算一张 $E(\mu,\alpha)$ 的表（LUT），把丢失的 $\int f$ 补回来（32×32 足够）。
- 自洽陷阱：**如果用同一个采样器既生成 LUT 又做验证，就测不出共享偏差**——必须用独立方法（白炉 + 已知解析解）仲裁。这条是数学正确性验证的通用教训。

## 采样：VNDF 为什么更省

按 $D(\mathbf{h})(\mathbf{n}\cdot\mathbf{h})$ 采样法线（NDF 采样）会把样本浪费在背面半球。Heitz 2018 的 **VNDF** 只在"从视线方向可见的法线分布"上采样，测到的方差 **0.747 → 0.136（5.5×）**——同一积分、同一 N，只改 pdf。
跨坐标系移植有坑：y-up 约定下 $T_2=V_h\times T_1$ 会让法线分量变负，改 $T_2=T_1\times V_h$ 才对齐。

## 什么时候**不**该用 PBR

你的项目里两种路线并存，讲的时候要分清：
- **物理正确**：`render_unified_oss`（`microfacet.h`、`bsdf.cpp`、`ggx_energy_lut.h`，白炉 ±1%）；`water` 海面用 Beckmann/Smith/Schlick（`FFTOcean_Render.shader:375-405`）——水面是物理材质，值得用。
- **风格化**：`SDF` 的火焰溶解与元素球走"贴图定形 / 色带定色 / HDR 提亮"的艺术控制路线（`t-037`），**刻意不做能量守恒**；`TA_show` 的雨夜氛围同样以观感为目标。
→ 面试回答："PBR 是默认选择，但**风格化特效要的是可控而非守恒**，我会明确放弃物理约束并说明代价。"

## 可能被追问的三层

1. **原理层**：$D/G/F$ 各自解决什么？Smith 遮蔽为什么用"高度相关"假设？菲涅耳在掠射角趋近 1 意味着什么？金属/非金属的 $F_0$ 有什么差别（金属度工作流）？
2. **实现层**：写出 GGX 的 $D$ 与 $G$；为什么分母有 $4(\mathbf{n}\cdot\mathbf{l})(\mathbf{n}\cdot\mathbf{v})$？Kulla-Conty 的补偿项怎么和 $F$ 耦合（$\bar F$ 的平均）？粗糙度怎么从美术的"感知粗糙度"映射到 $\alpha$（通常 $\alpha=r^2$）？
3. **边界层**：能量损失在粗糙金属上最明显——怎么在项目里验证？（白炉）如果不用 LUT，还有什么近似（解析多散射近似、Sheen/清漆层）？清漆、薄片、SSS 各自往模型里加什么？

## 手写/白板题（自测）

1. 写出 Cook-Torrance 微表面 BRDF 与 GGX 的 $D$、Smith 的 $G$、Schlick 的 $F$。
2. 证明/说明白炉测试的理想结果为什么是"均匀白、能量 1.0"，以及单次散射为什么达不到。
3. 写出 VNDF 相对 NDF 采样"不浪费样本"的直觉。

## 证据

- `G:\openGL\render_unified_oss`：`t-010`（白炉 0.335→0.9962、VNDF 方差 5.5×）、`include/shading/microfacet.h`、`src/shading/bsdf.cpp`、`include/shading/bsdf_device.h`（CPU/GPU 同式）、`ggx_energy_lut.h`
- `D:\unity\unityProjectLocation\water`：`Assets/FFTVerify/FFTOcean_Render.shader:375-405`（Beckmann/Smith/Schlick）
- `D:\unity\unityProjectLocation\SDF`：`t-037`（风格化路线对照）
- 延伸阅读：Kulla & Conty 2017（多散射补偿）、Heitz 2018（VNDF）、Walter 2007（玻璃 BTDF）
