---
id: 天空 Cubemap 按需生成
type: note
title: 天空 Cubemap 按需生成
parent: 大气散射渲染
status: none
rel: contains
tags:
- 图形渲染
- 大气
- 反射
revision: 1
updated: 2026-09-08T20:22:49+08:00
---

> 触发：天空 Cubemap 生成；水面反射

# 天空 Cubemap 按需生成

水面反射不能用逐像素天空（FFT 海面每个片元反射方向都不同），工程用 `SkyCubemapGen.compute` 生成**低分辨率天空 Cubemap**作为反射源（`_SkyCubemap`）。

## 生成内容

- 6 面 `Tex2DArray`（分辨率 64~256 可配，默认 128，ARGBHalf + mip）；
- 每像素：16 步球形散射（`SCATTER_STEPS`，与 `AtmosphereLib.SingleScattering` 同构）+ 各向同性多散射 LUT 项 + **太阳盘**（保留太阳 → 水面太阳高光）；
- 面方向约定 = Unity/D3D 标准（`BoxProjectedCubemapDirection`），逐面 switch；
- 一次性 `CommandBuffer`：dispatch 6 面 + `GenerateMips`（mip 供粗糙度感知的 `GlossyEnvironmentReflection` 用）。

## 按需更新（关键设计）

Cubemap 不需要每帧生成：

- 太阳方向变化 **> 0.5°** 或观察者高度变化 **> 0.2 km** 才重算（`cubemapSunAngleThresholdDeg` / `cubemapAltitudeThresholdKm`）；
- 太阳在白天缓慢移动 → 实际几分钟才重算一次，开销可忽略；
- 这是"**动态太阳接口**"的预留：阈值调小即可支持实时日夜循环。

## 观察者高度

`_ObserverAltitudeKm` 取主相机高度（clamp ≥0），Cubemap 以该高度为观察点生成——水面反射的天空与相机高度一致（飞机视角与地面视角反射不同）。

## 相关节点

- 天空渲染与太阳盘（同一散射的逐像素版本）
- 水面材质渲染（FFT 侧的反射消费方）
