---
id: URP-RenderFeature插入点与依赖
type: note
title: URP RenderFeature 的插入点与依赖
parent: 引擎与管线
status: none
tags:
- URP
- RenderFeature
- RenderPass
- ConfigureInput
- RT生命周期
created: 2026-09-21T21:50:00+08:00
updated: 2026-09-21T21:50:00+08:00
revision: 1
---

> 触发：RenderFeature 怎么写；renderPassEvent 选哪个；拿不到深度；RT 泄漏；Blit 画面镜像

# URP RenderFeature 的插入点与依赖

## 结构：Feature 只是"注册器"，真正的活干在 Pass 里

一个自定义全屏效果由两块组成：
- `ScriptableRendererFeature`：持有一个 `ScriptableRenderPass`，在 `AddRenderPasses` 里按条件 `EnqueuePass`。**它不做渲染**。
- `ScriptableRenderPass`：在 `Execute` 里做 Blit / 设置 RT / 派发 compute。

## 插入点（`renderPassEvent`）与它决定的一切

| 时机 | 你能拿到什么 | 典型用途 |
|---|---|---|
| `BeforeRenderingTransparents` | 不透明色 + 深度 | 需要挡住半透明的效果 |
| `AfterRenderingOpaques` | 不透明色 + 深度（**不含半透明**） | 雾、SSAO、屏幕空间反射 |
| `AfterRenderingSkybox` | 天空盒已画 | 大气/天空相关的合成 |
| `AfterRenderingTransparents` | 全场景颜色 | 后处理：锐化、色调映射、热扭曲 |
| `BeforeRenderingPostProcessing` | HDR 颜色（未被 tonemap） | 需要在线性 HDR 空间做运算的效果 |

**顺序即语义**：`renderPassEvent` 用错了，效果会"看起来对但物理上错"（比如在 tonemap 之后做线性空间的运算）。

## 依赖必须显式声明：数据不会自己出现

- `ConfigureInput(ScriptableRenderPassInput.Depth | Color | Normal | Motion)` ——**告诉管线"我要深度，请先把它准备好"**。
- 真实事故：雾 pass 之所以能工作，是因为**恰好**开了 SSAO，SSAO 顺带产生了深度；一旦关掉 SSAO 或换管线，雾就坏。修法是**显式声明依赖**（`TA_show` 的 `AtmosphereFogRendererFeature.cs:74`）。
→ 面试可说："**隐藏耦合比显式报错更危险**——它只在别人改配置时才暴露。"

## 全屏 Blit 的正确姿势

- 自己写全屏三角形/四边形顶点着色器时，**要处理平台差异**（DX 的 UV 起点、`UNITY_UV_STARTS_AT_TOP`、`_ProjectionParams.x`）。真实事故：自写 `Vert` 导致画面**左右镜像** → 改用 core `Blit.hlsl` 里的 `Vert` 就正确。
- **RT 生命周期**：`RenderTexture.GetTemporary` 必须配 `ReleaseTemporary`（不是 `.Release()`）。参考实现里用 `.Release()` 归还临时 RT 会造成泄漏/句柄错乱（`windtest` 的 `BilateralBlur.cs:49-58` 就是这条的修正）。
- 需要跨帧保存的（TAA 历史、反射 RT）才自建 `RenderTexture`，并且要处理分辨率变化与格式。

## 在你项目里的落点

- `TA_show`：`AtmosphereFogRendererFeature.cs`（显式声明 Depth 依赖）、`AtmosphereFog.shader`（全屏雾 + 采样反射 RT 保持天地缝合）、Renderer 资产里注册 AtmosphereSky / SSAO / AtmosphereFog 三个 Feature（顺序即语义）。
- `windtest`：云的 RenderFeature 注册在 `URP-HighFidelity-Renderer.asset`，`renderPassEvent = 600`（不透明之后）；**注意**：切到 Performant 档时该 Feature 未注册 → **云直接消失**（隐藏地雷，面试可讲"配置分档要有一致的特性开关"）。
- `water`：`UnderwaterEffect` 注册在 `URP-Balanced-Renderer.asset`，`renderPassEvent = 450`（0.4.5 版本对应 AfterRenderingTransparents 区间）。
- 同一坑的变体：`water` 的雾 pass 用 core `Blit.hlsl` 的 `Vert` 修掉左右镜像。

## 可能被追问的三层

1. **原理层**：URP 的渲染顺序与 Built-in 的差别？为什么后处理要在 tonemap 之前做线性运算？`ScriptableRenderPassInput` 背后管线做了什么（RT 分配与拷贝）？
2. **实现层**：`ConfigureInput` 到底让谁在什么时候产生深度？`GetTemporary` / `ReleaseTemporary` 的配对规则与池化机制？`Blitter` 与手写 `Graphics.Blit` 的差异？怎么在 SceneView 与 GameView 都正确（相机类型判断）？
3. **边界层**：多个 Feature 争同一张 RT 怎么办？移动端 (TBDR) 上频繁切 RT 的代价（tile 内存 flush）？如何做"特性开关 + 质量档"而不出现 windtest 那种"切档少一个 Feature"？

## 手写/白板题（自测）

1. 写出一个最小 `ScriptableRendererFeature` + `ScriptableRenderPass` 的骨架，并说明每部分职责。
2. 说明"我要深度"应该写在哪、为什么不能依赖别人恰好产生深度。
3. 画出一次全屏后处理里 RT 的分配/释放与 UV 翻转发生在哪几步。

## 证据

- `D:\unity\unityProjectLocation\TA_show`：`Assets/Atmosphere/Scripts/AtmosphereFogRendererFeature.cs:74`、`Assets/Atmosphere/Shaders/AtmosphereFog.shader`、`Assets/Settings/URP-HighFidelity-Renderer.asset:13,29,64`
- `D:\unity\unityProjectLocation\windtest`：`Assets/Settings/URP-HighFidelity-Renderer.asset`（云 Feature，renderPassEvent 600）、`Module09_BilateralBlur/BilateralBlur.cs:49-58`
- `D:\unity\unityProjectLocation\water`：`Assets/Atmosphere/`、`URP-Balanced-Renderer.asset`（Underwater，renderPassEvent 450）、`G:\water\.chain` 的 `大气模块工程踩坑`
