---
id: water
type: note
title: 项目 · water（FFT 海面渲染）
parent: 项目深挖
status: none
tags:
- water
- FFT
- 海洋渲染
- JONSWAP
- 大气散射
evidence:
- artifacts/water/water-侦察报告-2026-09-21.md
created: 2026-09-21T20:50:00+08:00
updated: 2026-09-21T20:50:00+08:00
revision: 1
---

> 触发：water 工程；FFT 海面；JONSWAP；海面泡沫；海面渲染怎么做的

# 项目 · water（FFT 海面渲染）

**一句话**：URP 下的**实时海面渲染实验工程**——Tessendorf/FFT 统计波谱海面 + 物理大气散射天空。**不是**流体模拟（无流体求解、无波-物双向耦合），面试时别把它讲成"水体模拟"。

**技术栈**：团结 Unity 2022.3.62f1c1 + URP 14.0.12。Assets 内 `.cs` **181** 个，但**自研只有** `Assets/FFTVerify/`（10 文件）+ `Assets/Atmosphere/`（16 文件），其余是 MapMagic / Pure Poly 第三方资产——被问"181 个脚本"时必须能划清。`.shader` 28、`.compute` 5、`.hlsl` 2、无 ShaderGraph；场景 11 个，Build Settings 只启用 `SampleScene`。

**配套知识库**：`G:\water\.chain`（dev 模式 31 节点，git 3 次提交）——完整的技术路线与踩坑都在那里，本节点是面试向提炼。

## 实现方案（FFT 单线，非 Gerstner）

- **频谱**：固定 1024²，6 kernel 管线（初始化 → 共轭 → 演化 → Stockham IFFT → 组装），`FFTOcean_Complete.compute:8-13`。
- **波源**：4 层频谱叠加，Tile 0.04/0.06/0.12/0.18、贡献 0.8/0.8/0.6/0.4，每层 2 组 JONSWAP ⇒ **8 个涌浪源**（`FFT_SpectrumVerify.cs:91-124`）。
- **位移与法线**：频域**解析求导**（位移/斜率/Jacobian 全在频域算，不用有限差分）——`compute:337-363`。
- **泡沫**：Jacobian 折叠检测 + `.a` 通道跨帧衰减累积 + 岸边深度交叉。
- **SSS**：k1 波峰 / k2 视角 / k3 阴影补光 / k4 SH 四件套（`FFTOcean_Render.shader:388-405`）。
- **反射**：URP `GlossyEnvironmentReflection` + Schlick（`:375-399`）；**无屏幕空间折射**（折射只在水下后处理 `UnderwaterEffect.shader:78`，Snell 48.6° 窗）。
- **LOD**：单张 200 m 平面（`waterMeshRes=10`）+ 距离自适应曲面细分 + 视锥整三角形剔除（`FFTOcean_Render.cs:37-44`、shader `:90-226`）。

> Gerstner 波只存在于工程根的 `_Backup_WaterSplit/`（不在 Assets、不参与编译）——它是**被淘汰的旧路线**，递进链的证据。

## 关键难题 → 实现办法

**递进链 A（海面）**：问题 → **方案 · Gerstner 波求和** → **局限 · O(N波×N顶点) 与谱采样失真** → **方案 · FFT 统计波谱法**（solves）→ **局限 · FFT 的边界**（不模拟破碎、200 s 精确循环、平铺重复、无波物耦合、风场全局）。

**递进链 B（大气）**：问题 → **方案 · 单次散射体积积分** → **局限 · 嵌套步进 32×64≈2000 次/像素** → **方案 · 透射率 LUT（256×128）**（solves）→ **局限 · 缺多次散射（地平线/日落失真）** → **方案 · 多散射 LUT 不动点迭代（32×32、2 轮）**（solves）→ **局限 · 各向同性源 + 未计地面**。

| 问题 | 办法 | 备注 |
|---|---|---|
| 循环动画不闭合 | 角频率量化到 $2\pi/200\text{s}$（`RepeatTime=200`） | `FFT_SpectrumVerify.cs:44` |
| 平铺重复感 | 4 层不同 Tile 错开 | **缓解未根除**（要承认） |
| 白冠泡沫没有形状 | Jacobian 几何近似；泡沫只衰减不漂移 | 物理上是近似 |
| 逐像素采样天空 cubemap 太贵 | 128³ Cubemap 按需更新（太阳 >0.5° 或高度 >0.2 km） | 脏检查式更新 |
| 雾 pass 画面左右镜像 | 改用 core `Blit.hlsl` 的 `Vert` | 自写全屏 Vert 的翻转坑 |
| `RWTexture2D` 不能 `SampleLevel` | 另建只读孪生纹理 | HLSL 类型约束 |
| 单位 m⁻¹ ↔ km⁻¹ 漏乘，**无任何报错** | 逐项对量纲，靠数值验证发现 | 这条最有面试价值：静默错误 |

## 工程侧工作（TA 加分项）

- `apply_lod_origin_v5_min.py`：把 Gerstner 循环拆成"基础波 8 + 按 LOD 指定细节波区间"，新增 `_BaseWaveCount / _DetailWaveStart / _DetailWaveEnd`，实现 LOD 同源化（方案 B）。
- `fix_disp.py`：修位移符号（`-Q·A·dir·sinθ`）。
- `modify_shader.py`：P0 消噪（关毛细波坐标扭曲、关 ddx/ddy 法线）+ Gerstner 距离 LOD 淡出 + 细节法线 + 涟漪开关。
- **数值验证工具链**：`FFTVerify/Output/*.png|exr`、根目录 `SpectrumDebug_*.png`，用 `CopyTexture → ReadPixels` + `_DebugOutput` 做频谱统计——"先能看见数值，再谈调参"。
- `Assets/Atmosphere/AtmosphereArtBridge_美术桥接设计方案.md`：美术面板反解物理系数，含验收标准（**未实现**）。

## 风险与缺口（面试前处理）

- **自研边界**：参考实现是 GitHub **ChenHanMK1/FFT-Ocean-Code**，工程注释自承"照搬参考实现的渲染逻辑"。→ 表述应为"按参考实现复刻管线，并在 X / Y 上做了自己的改造"（X/Y 需你确认：数值验证工具链、4 层频谱与 Tile 参数、LOD 脚本、大气链路）。
- **三个 python 脚本已失效**：路径指向 `G:\unityProjectLocation\water\Assets\WaterSplit`，该目录与目标 `OceanMeshCompute.compute` 均已不存在——**别在面试里说"现在能跑"**，要说"当时用它们改的资产"。
- **未接线的一部分**：`OceanAtmosphereBridge.hlsl` 没有被任何 shader include（海-气桥"已写未接"）；FFT 水面用自带指数雾，`_SkyCubemap` 只被备份里的旧 `WaterSurface` 使用。
- **KB 未收录**：`UnderwaterEffect.shader` + `UnderwaterEffectRendererFeature.cs`（已注册在 `URP-Balanced-Renderer.asset`，renderPassEvent 450），`PROCESS_LOG.md` 明写"尚未研读"。
- **无性能/显存数据**；工程本身非 git 仓库（只有 KB 有 3 次提交）；主场景为二进制序列化，无法逐对象核对组件。

## 可能被追问的三层

1. **原理层**：JONSWAP 谱各参数物理含义？色散关系 $\omega=\sqrt{gk\tanh(kh)}$ 的深水近似？Jacobian 折叠为什么代表破碎？$p(\omega)$ 与 $h(x)$ 的傅里叶对偶关系？
2. **实现层**：Stockham IFFT 与 Cooley-Tukey 的区别、为什么 GPU 上更适合？共轭打包省了什么？频域解析求导比有限差分好在哪？4 层 Tile 的贡献权重怎么定的？
3. **边界层**：FFT 海面为什么做不出卷曲破碎浪？200 s 循环对游戏意味着什么？换到移动端先砍哪一层？这套能接浮力/物理交互吗（KB 里 `_BuoyancyData` 提到过）？

## 证据索引

- 知识库：`G:\water\.chain\nodes\`（`FFT海洋渲染` 主题中心、`方案 · FFT 统计波谱法` 及其 10 个子节点、`大气模块工程踩坑`）
- 代码：`D:\unity\unityProjectLocation\water\Assets\FFTVerify\`（`FFTOcean_Complete.compute`、`FFT_SpectrumVerify.cs`、`FFTOcean_Render.cs/.shader`）、`Assets/Atmosphere/`、`Assets/Scenes/SampleScene.unity`
