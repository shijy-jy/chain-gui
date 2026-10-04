---
id: SDF
type: note
title: 项目 · SDF（系列 shader 特效开发）
parent: 项目深挖
status: none
tags:
- SDF
- 火焰溶解
- GPU粒子
- URP
- ShaderGUI
evidence:
- artifacts/SDF/SDF-侦察报告-2026-09-21.md
created: 2026-09-21T20:40:00+08:00
updated: 2026-09-21T20:40:00+08:00
revision: 1
---

> 触发：SDF 工程；火焰溶解特效；GPU 火星漩涡粒子；元素球；shader 工具链

# 项目 · SDF（系列 shader 特效开发）

**一句话**：Unity URP 上的**系列 shader 特效开发**——主线是 `L17` 火焰溶解特效（溶解家族 3D 主用 + 5 个 2D 变体、GPU 火星漩涡粒子、元素球、后处理件：CAS 锐化/热扭曲/地面光斑/烟）。工程内另有 `Assets/SDF/Shaders/L01~L16` 的 raymarching 课程线，与本特效无关，**面试讲的时候要分开**。

**技术栈**：团结 Unity 2022.3.62f1c1 + URP 14.0.12；shader 类 35 个（30 `.shader` + 3 `.cginc` + 2 `.compute`）；脚本 19 个 `.cs`（L17 相关 17 个）。核心量级：`L17_EmberVortexFX.cs` 550 行、compute 340 行、`L17_FireCore.shader` 331 行、`L17_Dissolve_3D.shader` 308 行。

**已有工程图谱**：`D:\unity\unityProjectLocation\SDF\.chain\`（analysis 模式 104 节点：goal 1 + design 16 + task 53 + verification 34）。本节点只做面试向提炼，**细节去原图谱查**（`t-*` 任务节点、`d-013` 重构设计、`d-011` 视觉优化方案）。

## 关键难题 → 实现办法

| 问题 | 办法 | 代价 / 备注 | 证据 |
|---|---|---|---|
| 立方体跨棱溶解断裂（六面 UV 各自独立） | 三方案比选后**弃 UV**，改对象空间 3D Perlin | 图案随物体移动、无法与贴图对齐 | `t-002`、`NoiseLib.cginc` |
| 溶解边缘锯齿 / shimmer | 分析式 AA `fwidth(d)*1.5` + 八度自适应淡出 + CAS 锐化 Feature | 四层体系需自挂；TAA 与 MSAA 互斥 | `t-011`、`L17_Dissolve_3D.shader:217,253,272,304` |
| 粒子 `orbCurrentY` 暴涨到 **12700+** | 从量级反推（1000 粒 y≈3 应得 ≈6147）定位到加权平均**漏乘权重**，`InterlockedAdd` 补权重，一行修复 + 溢出核验 | 需数值溢出核验 | `t-046`、`L17_EmberVortex.compute:320-321` |
| 元素球全白过曝 | 四层叠加 ≈9.3 vs Bloom 阈值 1.0 → 分层归因：贴图定形 / 色带定色 / HDR 只提最热处，并删掉"内核实心"叠加层 | 默认值不覆盖材质，须同步改 `.mat` 并备份 | `t-037`、`L17_FireCore.shader:239` |
| 重构后立方体不溶解 | `MaterialPropertyBlock` 两次 `Get` 覆盖式丢弃 → 合并为单次提交 | 静态验收漏检此类"顺序敏感容器" | `t-052` |
| 团结引擎不支持带字符串 drawer 的 ShaderGUI | 自写 C# `ShaderGUI` 两级查找去重（263 属性 → 105 条提示） | 须查大括号配平 / CRLF / BOM | `t-039`、`L17ShaderTipsGUI.cs` |
| 双会话（两个 AI 会话）改同一文件冲突 | Director + 黑板 + 5 模块拆分、分文件、写前重读 | 场景序列化值需人工迁移 | `d-013`、`t-050` |
| 静态验收存在盲区（截图不能作 GPU 粒子判据） | 运行时结构化采样 + 独立 Dispatch 对照隔离 | 需要写测试脚手架 | `v-015`、`PROCESS_LOG.md` |
| Additive 软粒子失效 | `softFade` 要乘 rgb 不乘 alpha；插入 frag 前须扫短名重名 | linter 不覆盖 HLSL | `t-032`、`L17_EmberParticle.shader:187-194` |

## 工程化与协作（这块最有说服力）

- **模块化重构**（`d-013` 是施工级文档：现状盘点 801 行/11 分组 → 目标瘦身 ≤600 行 → 模块接口契约（只读属性 + `PhaseEntered` 事件、禁写回）→ S0~S6 分步与回退 → **场景序列化迁移方案** → 并发协作规范 → 风险表）。
  实际落地：Director 543 行 + `Modules/L17_FXModule.cs` 黑板 + 5 模块（Ember 239 / Orb 64 / GroundGlow 73 / HeatDistort 76 / Smoke 86 行），支持 solo/weight/tier。
  **为什么做**：状态机独立成可观察核心、表现模块各自开关、**降低双会话同文件冲突**。
- **验收方法演进**：18 个 `v-*` 静态核对 → 被 `t-052` 实证暴露盲区 → 改为运行时结构化采样。
- **工具链**：通用中文化 ShaderGUI（263 → 105 条去重提示）。

## 可能被追问的三层

1. **原理层**：对象空间 3D Perlin 与 UV 噪声的区别？`fwidth` 为什么能给分析式 AA？GPU 粒子为什么不能靠截图验收？
2. **实现层**：`InterlockedAdd` 在加权平均里的正确用法？`MaterialPropertyBlock` 的"顺序敏感"是什么机制？CAD/CAS 锐化的代价？
3. **边界层**：对象空间噪声的图案漂移怎么规避？这套溶解在移动端/半精度下会怎样？模块化拆分的性能代价（黑板通信、间接调用）？

## 风险与缺口（面试前必须处理）

- **归属要说清**：图谱多处记录由**并行会话（另一 AI 会话）**实施（`d-013` 设计、`t-050` 补档）。面试前请逐项标注"哪些是我亲手写的 / 哪些是我审的 / 哪些是协作产出"——否则被追问细节会露。
- **实机结论缺失**：9 个 task 仍 `in_progress`，多处注明"待 Game 视图裁决"；`v-*` 虽全 success 但多为静态核对。建议把主线效果跑一遍并录屏，补上"最终画面 + 帧率"。
- `v-005/v-006` 缺号（v-004 后跳 v-007），原因未知；`PROCESS_LOG.md` 末条停在 09-14。
- 下一轮已验证但未做的候选：爆裂重生 `enableBurst`、漩涡中心观感、发射密度。

## 证据索引

- 图谱：`D:\unity\unityProjectLocation\SDF\.chain\nodes\`（`d-013` 重构设计、`d-014` 评审、`t-046` 加权平均、`t-037` 过曝分层、`t-039` ShaderGUI、`t-050` 重构补档、`v-015` 运行时采样）
- 代码：`Assets/shader/`（`L17_Dissolve_3D.shader`、`L17_FireCore.shader`、`L17_EmberParticle.shader`、`NoiseLib.cginc`）、`Assets/SDF/Scripts/`（`L17_EmberVortexFX.cs`、`Modules/`、`Editor/L17ShaderTipsGUI.cs`）、`Assets/VFX/`
