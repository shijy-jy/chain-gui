---
id: Jacobian与微分几何
type: note
title: Jacobian 与微分几何：折叠、面积与采样换算
parent: 图形数学基础
status: none
tags:
- Jacobian
- 微分几何
- 折叠检测
- pdf换算
- 面积缩放
created: 2026-09-22T02:00:00+08:00
updated: 2026-09-22T02:00:00+08:00
revision: 1
---

> 触发：Jacobian 行列式；折叠检测；为什么有泡沫；面积换算；pdf 换算

# Jacobian 与微分几何：折叠、面积与采样换算

## 一、是什么

一个映射 $\mathbf{T}:\mathbb{R}^n\to\mathbb{R}^n$ 的 **Jacobian 矩阵**是它的**局部线性化**：

$$J = \frac{\partial \mathbf{T}}{\partial \mathbf{p}},\qquad
\text{二维时 } J=\begin{pmatrix}\partial T_x/\partial x & \partial T_x/\partial z\\[2pt] \partial T_z/\partial x & \partial T_z/\partial z\end{pmatrix}$$

**行列式 $\det J$ 的几何意义：局部的面积（三维是体积）缩放因子。** $\det J > 0$ 保持定向，$\det J < 0$ 翻转定向，$\det J = 0$ 表示该处被压扁成零面积（退化）。

## 二、在渲染里的三类用途（每一类都有你的项目证据）

### 1. 位移映射的折叠检测（`water` 的破碎浪泡沫）

海面位移是 $\mathbf{p}' = \mathbf{p} + \mathbf{D}(\mathbf{p})$，于是 $J = I + \partial\mathbf{D}/\partial\mathbf{p}$：

$$\det J = \left(1+\lambda\frac{\partial D_x}{\partial x}\right)\left(1+\lambda\frac{\partial D_z}{\partial z}\right)-\left(\lambda\frac{\partial D_x}{\partial z}\right)\left(\lambda\frac{\partial D_z}{\partial x}\right)$$

**$\det J < 0$ 意味着曲面在该处自交（局部"翻面"）**——几何上对应"波峰挤压到把自己折叠起来"。工程上把它当作**破碎浪/白冠泡沫的近似判据**：几何折叠 ⇒ 能量集中 ⇒ 该处生成泡沫。
> 这是一个**近似**，不是流体模拟：真实破碎涉及拓扑变化与飞溅，而 FFT 海面是单值高度场（这也是它做不出卷曲浪的根本原因）。

### 2. 采样域的 pdf 换算（路径追踪）

在立体角上采样、在面积上积分（或反过来）时必须乘 Jacobian：

$$p_A = p_\omega \cdot \frac{\cos\theta}{r^2},\qquad \text{即}\quad d\omega = \frac{\cos\theta\, dA}{r^2}$$

**漏掉这个换算 = pdf 错 = 有偏（能量错）**。渲染器的"光源采样是不是有偏"这类 bug，十有八九是这一步。它和 MIS 权重是同一套语言（见「蒙特卡洛与重要性采样」）。

### 3. 参数化的扭曲（纹理密度与 mip 选择）

屏幕空间对 UV 的映射 $J = \partial \mathbf{uv}/\partial \mathbf{screen}$ 决定**一个像素覆盖多少纹理**——这正是 `tex2D` 隐式求导在做的事，也是 mipmap/LOD/各向异性过滤的依据。程序化噪声在远处闪烁（`fwidth` 失效）本质上也是局部频率（Jacobian 的奇异值）过大。

## 三、与"法线用逆转置"的关系（把两个节点连起来）

| 对象 | 变换规则 | 为什么 |
|---|---|---|
| 切向量（位移、速度） | $J$ | 跟随映射 |
| **法线** | $J^{-T}$（逆转置） | 保持与切平面垂直（见「变换与坐标系」推导） |
| **面积/体积** | $\det J$ | 行列式即缩放因子 |

面法线的正确变换矩阵其实是**余子式矩阵** $= \det(J)\cdot J^{-T}$——**同一个 Jacobian 的三个"面"**。能把这三行说清楚，说明你真的理解变换而不只是背公式。

## 四、可能被追问的三层

1. **原理层**：为什么 $\det J$ 是面积缩放（从平行四边形的面积公式推）？为什么 $\det J<0$ 表示翻转而不是"负面积"？退化（$\det J = 0$）在几何上意味着什么？
2. **实现层**：在 shader 里怎么检测 $\det J<0$（用 `ddx/ddy` 的有限差分还是解析求导）？为什么 FFT 海面要用**频域解析求导**而不是有限差分？$\cos\theta/r^2$ 里的 $r$ 是到光源的距离还是到采样点的距离（点光源 vs 面光源的差别）？
3. **边界层**：折叠检测只能给出"折叠程度"，怎么把它变成好看的泡沫分布（阈值 + 跨帧累积 + 岸边交叉，见「FFT 统计波谱海面」）？Jacobian 在非均匀缩放/蒙皮变形下怎么算？什么情况下有限差分比解析求导更合适？

## 五、手写/白板题（自测）

1. 写出二维位移映射的 Jacobian 行列式，并解释 $\det J<0$ 的几何含义。
2. 推导 $p_A = p_\omega\cos\theta/r^2$，并说明漏掉它会带来什么后果（有偏？能量多还是少？）。
3. 说明同一个 Jacobian 如何分别决定"切向量变换""法线变换""面积缩放"。

## 六、证据

- `G:\water\.chain`：`Jacobian 折叠检测与泡沫`（完整推导与工程实现）；`D:\unity\unityProjectLocation\water\Assets\FFTVerify\FFTOcean_Complete.compute:337-363`（频域解析求导）
- `G:\openGL\render_unified_oss`：`t-011`（pdf 选择与偏差）、`MIS + NEE` 实现（`path_trace_gpu_backend.cu`）
- `D:\TA\.chain\nodes\变换与坐标系.md`（法线逆转置的推导）、`FFT统计波谱海面.md`（折叠与泡沫的工程用法）
