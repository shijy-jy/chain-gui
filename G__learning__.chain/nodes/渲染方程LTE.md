---
id: 渲染方程LTE
title: 渲染方程LTE
parent: 蒙特卡洛渲染基础
tags:
- 渲染
- 渲染方程
- 基础
revision: 1
updated: 2026-09-08T20:17:34+08:00
---

> 触发：渲染方程 LTE；LTE 渲染方程

# 渲染方程 LTE（Light Transport Equation）

渲染的数学心脏：表面点出射的光 = 自发光 + 反射来的光。

## 方程形式

$$L_o(x,\omega_o) = L_e(x,\omega_o) + \int_{H^2} f_r(x,\omega_i\to\omega_o)\, L_i(x,\omega_i)\,\cos\theta_i\,\mathrm{d}\omega_i$$

其中 $\cos\theta_i = \max\big(\langle n(x),\ \omega_i\rangle,\ 0\big)$（负值钳 0 = 只收上半球）。

## 各项含义

- $L_e$：自发光项，**已知输入**（灯 emission=15 就是它）
- $f_r$：BRDF，材质属性（朗伯面 $f_r = \rho/\pi$，$\rho$ 是 albedo）
- $L_i$：入射 Radiance——**它本身又等于别的点的 $L_o$**，这是方程的迭代本质
- 传输项 $T(x,\omega_o) = \int f_r\,L_i\cos\theta_i\,\mathrm{d}\omega_i$：要解的未知量

## 直接光照与间接光照的分解

把入射项按"光从哪来"拆开：$L_i = L_i^{dir} + L_i^{ind}$，

$$T(x,\omega_o) = \underbrace{\int_{H^2} f_r\, L_e\big(y(x,\omega_i)\big)\, V(x,y)\,\cos\theta_i\,\mathrm{d}\omega_i}_{\text{直接光照：光从光源直达 } x} + \underbrace{\int_{H^2} f_r\, L_i^{ind}\,\cos\theta_i\,\mathrm{d}\omega_i}_{\text{间接光照：光至少弹跳过一次}}$$

- **直接**：光在 $x$ 处第一次接触表面就进相机（光路只含 1 段光照传输）
- **间接**：光此前至少反射过 1 次（≥2 段传输）
- $V(x,y)\in\{0,1\}$：可见性（阴影射线）

> **📌 概念辩证卡①：直接光照 ↔ 间接光照**
> - **直接光照**：光路 $L\,D^?E$ 只含一段光照传输——能量来自光源本身，是画面亮度与阴影的主宰；采样上它是"点对点"的（NEE 直接连线光源），方差相对可控。
> - **间接光照**：光路至少弹跳两次——携带表面间的能量交换（色溢、柔光），是画面"质感"的来源；采样上它是"链式"的（BSDF 采样递归），方差天然更高、路径更长。
> - **辩证关系**：二者同属传输项 $T$，是**互补分解**（$L_i = L_i^{dir} + L_i^{ind}$，多算一份则双重计数）；直接光"亮而硬"，间接光"暗而柔"，真实画面 = 两者的统一。**它们也是 M2/M3 的分工线：M2（ReSTIR DI）攻直接光，M3（ReSTIR GI）攻间接光——分开攻克，正是因为它们难在不同维度。**

## 路径形式与四类路径

定义传输算子 $\mathcal{T}$：$(\mathcal{T}L)(x,\omega_o) = \int f_r\,L\,\cos\theta\,\mathrm{d}\omega$，方程 $L = L_e + \mathcal{T}L$ 的解展开成 Neumann 级数：

$$L = L_e + \mathcal{T}L_e + \mathcal{T}^2 L_e + \mathcal{T}^3 L_e + \cdots$$

第 $k$ 项 = 恰好 $k$ 次弹跳的光。Heckbert 路径记号（L=光源，E=眼睛，D=漫反射，S=镜面）：

| 记号 | 含义 | 四分类 |
|---|---|---|
| $LDE$ | 灯→漫反射→眼 | 直接漫反射 |
| $LSE$ | 灯→镜面→眼（镜中灯影） | 直接镜面反射 |
| $L(D\|S)D^+E$ | 至少两次漫/镜反射 | 间接漫反射 / 间接镜面反射 |

## 朗伯面的简化

朗伯 BRDF 与方向无关：$f_r = \rho/\pi$，

$$L_o(x,\omega_o) = L_e(x,\omega_o) + \frac{\rho}{\pi}\int_{H^2} L_i(x,\omega_i)\,\cos\theta_i\,\mathrm{d}\omega_i$$

这是后续所有推导的工作形式（墙/地板都是朗伯面）。
