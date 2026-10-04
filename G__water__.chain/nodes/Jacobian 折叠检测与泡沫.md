---
id: Jacobian 折叠检测与泡沫
type: note
title: Jacobian 折叠检测与泡沫
parent: 方案 · FFT 统计波谱法
status: none
rel: contains
tags:
- 图形渲染
- 海洋
- 泡沫
revision: 1
updated: 2026-09-08T20:22:48+08:00
---

> 触发：Jacobian 折叠检测；海面泡沫白冠；几何折叠近似

# Jacobian 折叠检测与泡沫

海面泡沫（白冠）的本质是**波破碎**。FFT 统计谱不模拟流体破碎，工程用**几何折叠检测**近似：位移映射 $\mathbf{x}\mapsto\mathbf{x}+\lambda\mathbf{D}$ 的 Jacobian 行列式度量波面是否"折叠"（自交）。

## Jacobian 公式

$$
J = \left(1+\lambda_x\frac{\partial D_x}{\partial x}\right)
    \left(1+\lambda_z\frac{\partial D_z}{\partial z}\right)
  - \lambda_x\lambda_z\left(\frac{\partial D_x}{\partial z}\right)^{2}
$$

（工程代码 `(1+λ·Dxx)(1+λ·Dzz) − λ²·Dxz²`，λ = `_WaveSharp`。）

- $J=1$：无变形；$0<J<1$：波面被压缩；**$J<0$：波面折叠**——真实中此处会破碎成白冠。

## 工程泡沫流水线（`CS_AssembleTextures`）

1. 锐化：$J'=\mathrm{saturate}(J)^{\,FoamPower}$（`_FoamPower` 控制敏感度曲线）；
2. 阈值：$f=\max(0,\ -(J'-FoamBias))$（`_FoamBias` 控制起泡门槛，取负再截断 → 只有"压过头"的部分起泡）；
3. **时间累积 + 衰减**：

$$
foam_t = foam_{t-1}\cdot e^{-FoamDecayRate} + FoamAdd\cdot f
$$

泡沫写进位移纹理的 `.a` 通道，**跨帧残留**（上一帧的泡沫衰减后与新增叠加）→ 泡沫"持续一会儿再消散"，比单帧检测自然得多。

## 渲染端的补充

- 岸边泡沫：深度交叉检测 `sceneDepth − viewDepth`，水与岸/物体交界处 smoothstep 出泡沫带；
- 阴影调制：泡沫量乘 `saturate(shadow + ShadowIntensity)`；
- 泡沫抬高粗糙度：`roughness = _Roughness + foam·_FoamRoughness`，白冠区高光发散。

## 局限

这是**几何近似**，不是流体模拟：泡沫不飞溅、不漂移（只随时间衰减）、不会形成真实白冠形状。真实破碎需要粒子/贴花系统补充。

## 相关节点

- 频域解析求导：位移与斜率（二阶导来源）
- 局限 · FFT 统计波谱的边界
