---
id: TA_show
type: note
title: 项目 · TA_show（雨夜实时氛围场景）
parent: 项目深挖
status: none
tags:
- TA_show
- URP
- 大气散射
- 平面反射
- 作品集
evidence:
- artifacts/TA_show/TA_show-侦察报告-2026-09-21.md
created: 2026-09-21T20:40:00+08:00
updated: 2026-09-21T20:40:00+08:00
revision: 1
---

> 触发：TA_show；雨夜场景；我正在做的项目；大气散射怎么做的；光柱；湿地倒影

# 项目 · TA_show（雨夜实时氛围场景）

**一句话**：一个**雨夜实时氛围场景的技术演示**（作品集性质，非小游戏、无玩法 UI）：画面由五层构成——大气散射天空、全屏雾、能量网格地面、40 根接地天光柱、雨与湿地倒影。构建场景只有 `SampleScene`。

**技术栈**：团结引擎 Unity 2022.3.62f1c1 + URP 14.0.12；自研 18 个 `.cs`，10 `.shader` + 1 `.hlsl` + 4 `.compute` + 1 `.shadergraph`；HDR 开、Opaque Texture 开、Depth Texture **关**；RendererFeature = AtmosphereSky / SSAO / AtmosphereFog。

**文档与产出**：`Docs/SceneOverview.md`（481 行，逐层说明 + 参数表 + 已知问题）；`screenshots/` **182 PNG + 7 MP4**，按模块与迭代版本命名，构成完整调试轨迹（`pillars_v2~v7`、`glass_v1~v3`、`ground_v1~v3`、`planar_v1~final`、`diag_*` 12 张逐层拆解、`cmp_baked/cmp_realtime` 烘焙 vs 实时 A/B、`MainCamera_2026-09-16~09-21` 连续 6 天）。

## 关键难题 → 实现办法

| 问题 | 办法 | 代价 / 备注 | 证据 |
|---|---|---|---|
| 横向网格线整组消失 | >65535 顶点时 16 位索引被静默截断 → 显式 `IndexFormat.UInt32` | 场景 extent=80 ⇒ 412,804 顶点 | `Assets/Scripts/InteractiveGridLines.cs:97-99`、`SampleScene.unity:1165` |
| 倒影随视角错位旋转 | `LookRotation(镜像fwd, 镜像up)` 手性翻转 → 改严格反射矩阵 `V_main × Scale(1,-1,1)` + `GL.invertCulling` | — | `WetPlanarReflection.cs:96-116` |
| 反射 RT 二次采样成"错位假网格 + 地平线硬分界" | 镜像渲染期间临时禁用贴地网格线 | 属"渲染顺序耦合"，非贴图问题 | `WetPlanarReflection.cs:25-31,108-119` |
| 雾在地面画出"钉在屏幕上的横带"、天空像素误判 | 去掉 `min(uv.y,0.58)` 夹取；反向 Z 写反 → 改 `UNITY_REVERSED_Z` 分支 | — | `AtmosphereFog.shader:78-90,115-120` |
| 雾 pass 里每像素两次全步数行星积分（纯浪费） | 删 ray-march：烘焙 cubemap 承担天空美感 + 仰角驱动解析雾；雾色逐像素采样反射 RT 保持天地缝合 | 牺牲"雾内看太阳"的实时散射 | `AtmosphereFog.shader:95-99,126-128` |
| LUT 只在 `Start()` 生成，改参数不更新 | 参数 hash 脏检查 + cubemap 100 ms 节流 | — | `AtmosphereController.cs:132,229,466-504,986` |
| 雾依赖 SSAO 才拿到深度（隐藏耦合） | 显式 `ConfigureInput(ScriptableRenderPassInput.Depth)` | 换管线/关 SSAO 就会坏 | `AtmosphereFogRendererFeature.cs:74` |

**性能架构**：41 万顶点程序化网格用 GPU 形变 + 双保险 LOD（顶点退化 `positionCS=(2,2,2,1)` + 片元早退），实测砍掉窗口内约 **36%**；反射 RT 提供 512/1024/2048 与隔帧选项；CPU 只推参数、几何在 GPU 算——40 根光柱共用单个 38,280 顶点 Mesh，世界位置由顶点着色器反算，6 组全局数组协议（8/40/48 槽）手工对齐。

**最长的一块**：实时大气散射 LUT 链——透射率 256×128 → 多散射 32×32 迭代 → 天空 cubemap 128²×6 面 ray-march（`AtmosphereLib.hlsl:87-400` + 4 个 `.compute` + `AtmosphereController.cs:567-974`），带参数 hash 脏检查与节流。

## 可能被追问的三层

1. **原理层**：透射率 LUT 为什么是 2D（高度 × 视角天顶角）？多散射迭代为什么收敛？平面反射矩阵为什么不能用 LookRotation 构造？
2. **实现层**：`IndexFormat.UInt32` 的开销是什么？脏检查用什么做 key？为什么关 Depth Texture 还能拿到深度（`ConfigureInput` 的机制）？
3. **边界层**：删掉雾内 ray-march 后哪些画面效果回不来了？4096 RT 下反射的带宽代价？移动端这套能不能跑？

## 取舍与"现在会怎么改"

- 立了"**确定性测量优先**"的工作方式：手机拍屏/肉眼判断只作最后一道，数值与分层拆解（`diag_*` 12 张逐层诊断图）作主判据。
- 已知妥协：`Assets/AtmosphereScattering/` 空目录（旧实现残留）、`_GradientRadius=0.16` 疑为误设但被判定为美术决策保留（`SceneOverview.md:458-464`）、质量档切到 Balanced/Performant 时玻璃折射可能静默失效（文档推断，未实测）。

## 风险与缺口（面试前要知道）

- **无 git 仓库** → 时间线只能用文件 mtime 与截图时间戳（`MainCamera_2026-09-16~09-21`）证明；建议尽快 `git init` 并补一次提交。
- 天空盒的 AI 生成工作流只有产物图（`Assets/TJGenerators/History/Skybox_20260917_083543.png`），**未见被场景引用的证据** → 别当作卖点讲。
- 未见运行时性能数字（帧时间/GPU 耗时）→ 建议补一次 Profiler 截图，否则"性能优化"只能讲手段不能讲收益。

## 证据索引

- 大气：`Assets/Atmosphere/Scripts/AtmosphereController.cs`、`Assets/Atmosphere/Shaders/AtmosphereLib.hlsl`、`Assets/Atmosphere/Shaders/*.compute`、`AtmosphereFog.shader`、`AtmosphereFogRendererFeature.cs`
- 地面与光柱：`Assets/Shaders/InteractiveGrid.shader`、`InteractiveGridLines.shader`、`SkyPillars.shader`、`Assets/Scripts/SkyPillarField.cs`
- 雨与反射：`Assets/Scripts/RainField.cs`、`RainImpactField.cs`、`WetPlanarReflection.cs`、`Assets/Shaders/RainDrop.shader`
- 编辑器工具：`Assets/Atmosphere/Editor/AtmosphereConfigEditor.cs`（美术调色面板 + compute 预览色带）、`Assets/Scripts/Editor/ThirdPersonSceneSetup.cs`（Tools 菜单一键搭场景）
