---
id: windtest
type: note
title: 项目 · windtest（体积云 + 大气散射）
parent: 项目深挖
status: none
tags:
- windtest
- 体积云
- 大气散射
- RayMarching
- 学习模块
evidence:
- artifacts/windtest/windtest-侦察报告-2026-09-21.md
created: 2026-09-21T21:00:00+08:00
updated: 2026-09-21T21:00:00+08:00
revision: 1
---

> 触发：windtest；体积云怎么做的；云光照；云和大气怎么接；参考实现踩坑

# 项目 · windtest（体积云 + 大气散射）

**一句话**：URP 的「**体积云 + 大气散射**」渲染技术 Demo。**注意：目录名 `windtest` 与内容无关**（全 Assets 检索 wind/植被/布料 0 命中）——面试开场先纠正名字，否则会被当成"风场项目"追问。

**技术栈**：团结 Unity 2022.3.62f1c1 + URP 14.0.12；`.cs` 13、`.shader` 7、`.hlsl` 2、`.compute` 3；1 个场景（`SampleScene`，云容器 `Cloud0` 是内置 Cube 缩放 200×20×150）；无预制体。

## 内容构成（要说清自研 / 复刻的边界）

| 部分 | 性质 | 证据 |
|---|---|---|
| **大气散射** | **自研**：透射率 LUT + Bruneton 多散射 LUT 迭代 + 天空 cubemap + 天空盒 + 太阳光大气衰减 | `Assets/Atmosphere/Scripts/AtmosphereController.cs`、`Assets/Atmosphere/Shaders/AtmosphereLib.hlsl`、`Atmosphere/Shaders/*.compute` |
| **体积云**（11 个渐进模块） | **复刻学习**：拆分自 GitHub `MagicStones23/Unity-Volumetric-Clouds`，原工程完整放在 `参考文件/`（不参与编译），Assets 内是逐模块对照版 | `Assets/LearningModules/README.md`、`Module05_CloudDensity/CloudMarching.shader`、`Module06_CloudLighting/VolumetricCloudsRenderPass.cs` |
| **工具** | 自研：30 参数中文分区实时调参面板、相机环绕、帧率显示 | `Module11_Final/CloudParamsController.cs` |

→ 面试表述用「**按参考实现逐模块复刻并定位/修正了它 4 类缺陷**」，不要讲成"我从零写了体积云"。修正的部分才是自研含金量。

## 关键难题 → 实现办法（对参考实现的修正，均有 diff 对照）

| 问题 | 参考实现的做法 | 我的办法 | 证据 |
|---|---|---|---|
| 光线平行于包围盒轴时出现 **NaN** | 无处理（`1/0 = inf`、`0×inf = NaN`） | 平行轴检测 `isPar`，把 t 区间置 `±1e30` 剔除 | `Module04_RayBoxDistance/RayBoxDistance.hlsl:20-29` vs `参考文件/.../RayBoxDistance.hlsl` |
| 云**没有厚度感** | `cloudDensity += stepCloudDensity` 累加快速饱和 | 改光学深度积分 `cloudOD += ρ·ds`，密度 `1-exp(-OD·σ)` | `Module05_CloudDensity/CloudMarching.shader:234-236` |
| 光照量纲错 + 曲线查表开销 | `_LightAttenuationCurve` 贴图查表 | `od = Σρ·ds` + 早退阈值 + HZD `Beer × powder` 解析式 | `CloudMarching.shader:238-261` |
| **RT 泄漏** | 用 `tempRT0.Release()` 归还 `GetTemporary` 的 RT | 改 `RenderTexture.ReleaseTemporary` | `Module09_BilateralBlur/BilateralBlur.cs:49-58` |
| 日落之后地面反射仍亮 | CPU 太阳衰减缺地面遮蔽 | `sunDir.y <= 0` 直接返回 0，与 GPU 天空一致 | `AtmosphereController.cs:386-387` |
| **云暗部与天空脱节** | 云环境光用固定色 | 生成天空 cubemap（Tex2DArray 6 面 + mip），CloudRendering 采样 `_SkyCubemap`，日落自动联动 | `Module07_CloudRendering/CloudRendering.shader:97-119,154-160` |

**这条"云环境光采样大气天空 cubemap"是项目里最有价值的一步**：它把两个独立模块真正耦合成一条链路，也是"我理解两个系统怎么互相影响"的证据。

## 完成度（如实说）

- **云：基本完成** —— `GameView_2026-07-17_16-54-06.png` 已是完整云海 + 太阳；RenderFeature 已注册进 `URP-HighFidelity-Renderer.asset`，4 个材质全连线（`renderPassEvent 600`）。
- **大气：天空单独可用** —— `GameView_2026-08-18_17-29-39.png` 蓝天空 + 日盘。
- **合成未完成** —— 最后一张 `GameView_2026-08-18_18-00-35.png` 云是黑白密度形式、背景全黑；场景序列化 skybox 仍是 Module01 的 `Learning/Skybox`，`Atmosphere/Skybox` 只在运行时被 `Update()` 强制覆盖（`AtmosphereController.cs:131-133`）。→ **面试要主动说"云与大气尚未合成，这是我下一步要做的"**，比被问出来好。

## 风险与缺口

- **无 git 仓库** → 改动时间线不可考（只有截图时间戳与 `mem-log`）。
- **大气代码疑从他工程回植**：`MultiScatterLUTGen.compute:2`、`SkyCubemapGen.compute:2,4` 注释写「water 工程新增」并引用 `WaterSurface.shader`（本工程**无此文件**）→ 说明大气代码来自 water/TA_show 那条线。面试讲的时候按"我在多个工程里迭代同一套大气实现"来讲（这是加分项），但别声称"在 windtest 里首次实现"。
- 未做性能量化；`URP-Performant-Renderer.asset` 未注册云 Feature → 切 Performant 档云直接消失（隐藏地雷）。
- 噪声是离线 PNG/3D 资产（`4MB×2` 三维噪声 .asset 来自参考工程），**不是自研生成**。
- 未运行引擎，无法确认当前零报错。

## 可能被追问的三层

1. **原理层**：Beer-Lambert 与 powder 效应各自的物理含义？为什么 `1-exp(-OD·σ)` 比累加更能表现厚度？多散射 LUT 的迭代为什么用不动点？
2. **实现层**：Ray-box 求交的 NaN 从哪一步来的、为什么用 `±1e30` 而不是 `FLT_MAX`？`GetTemporary` / `ReleaseTemporary` 的配对规则？双边模糊的核怎么定？
3. **边界层**：这套云在移动端怎么砍？为什么不用 `RenderTexture` 的体积云而用屏幕空间 ray-marching？云与大气合成时正确的顺序与深度怎么处理？

## 证据索引

- 代码：`Assets/LearningModules/Module04_RayBoxDistance/`、`Module05_CloudDensity/`、`Module06_CloudLighting/`、`Module07_CloudRendering/`、`Module09_BilateralBlur/`、`Module11_Final/CloudParamsController.cs`、`Assets/Atmosphere/`
- 参考（非自研）：`参考文件/VolumetricClouds/`、`参考文件/Library/`、`参考文件/Skybox/`（GitHub `MagicStones23/Unity-Volumetric-Clouds`）
- 截图：`screenshots/GameView_2026-07-17_16-54-06.png`（云完成）、`GameView_2026-08-18_17-29-39.png`（天空可用）、`GameView_2026-08-18_18-00-35.png`（合成未完成）
