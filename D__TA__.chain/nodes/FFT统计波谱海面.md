---
id: FFT统计波谱海面
type: note
title: 方案 · FFT 统计波谱海面
parent: 渲染效果
status: none
tags:
- FFT
- 海洋渲染
- JONSWAP
- Tessendorf
- 频域求导
created: 2026-09-21T23:00:00+08:00
updated: 2026-09-22T12:16:50+08:00
revision: 2
---

> 触发：FFT 海面怎么做；Tessendorf；JONSWAP 谱；海面法线怎么算；为什么海面要用 FFT

# 方案 · FFT 统计波谱海面

## 它解决什么（递进链）

```
问题：实时渲染逼真的大尺度海面
 └─ 方案 · Gerstner 波求和
     └─ 局限 · O(N波 × N顶点) 与"谱采样失真"（用几个正弦波硬凑真实海谱，波数覆盖不足、方向性假）
         └─ 方案 · FFT 统计波谱法（Tessendorf 2001）      ← solves
             └─ 局限 · 不模拟破碎、200 s 精确循环、平铺重复、无波物耦合、风场全局
```

**核心思想**：真实海面是**无数随机正弦波**的叠加，其统计特性由海浪谱描述。直接在时域逐波求和是 $O(N_{\text{波}}\times N_{\text{顶点}})$；改成在**频域**用一次 IFFT 把整片海面算出来，复杂度变成 $O(N^2\log N)$ 且与顶点数无关——**这是"用变换换复杂度"的经典案例**（面试可类比：为什么卷积用 FFT 做）。

## 算法管线（每帧一次，GPU compute）

1. **初始频谱** $h_0(\mathbf{k})$：由海谱（JONSWAP）× 方向扩散函数采样得到，配合高斯随机数；**共轭打包**满足 Hermitian 对称（$h(-\mathbf{k})=h^*(\mathbf{k})$），这样 IFFT 出来的高度场才是实数、且只需存一半数据。
2. **时间演化**：$\tilde h(\mathbf{k},t)=h_0(\mathbf{k})e^{i\omega t}+h_0^*(-\mathbf{k})e^{-i\omega t}$，其中 $\omega=\sqrt{gk}$（深水色散关系，$g$ 重力加速度、$k=|\mathbf{k}|$）。**波浪按各自相速度传播**——这就是海面"活起来"的全部来源。
3. **IFFT**：Stockham 算法（原地、无位反转，适合 GPU 并行）。
4. **组装**：从高度谱求位移场 $D(\mathbf{x})$，并在**频域解析求导**得到法线与 Jacobian。

## 三个让它成立的关键实现点

| 点 | 做法 | 为什么 |
|---|---|---|
| **频域解析求导** | $\partial h/\partial x = i k_x \tilde h$ 后再 IFFT，而不是对高度场做有限差分 | 一次乘加 vs 采样邻域；无差分噪声、精度高（`FFTOcean_Complete.compute:337-363`） |
| **Jacobian 折叠检测** | $J=\left(1+\lambda\frac{\partial D_x}{\partial x}\right)\left(1+\lambda\frac{\partial D_z}{\partial z}\right)-\left(\lambda\frac{\partial D_x}{\partial z}\right)\left(\lambda\frac{\partial D_z}{\partial x}\right)$，$J<0$ 处视为波峰破碎 → 生成泡沫 | 用几何压缩量近似"能量集中的白冠"，是**近似**不是物理模拟 |
| **多层频谱叠加** | 本项目 4 层，Tile 0.04/0.06/0.12/0.18，贡献 0.8/0.8/0.6/0.4，每层 2 组 JONSWAP（共 8 个涌浪源） | 单层 FFT 的波数覆盖不足 → 大浪/细纹无法共存；不同 Tile 错开缓解平铺重复（**缓解，未根除**） |

## 在项目里的落地与踩坑（`D:\unity\unityProjectLocation\water`）

- 固定 **1024²**、**6 kernel** 管线（初始化 → 共轭 → 演化 → IFFT → 组装），`Assets/FFTVerify/FFTOcean_Complete.compute:8-13`。
- **循环动画不闭合** → 角频率量化到 $2\pi/T$（`RepeatTime = 200` s）：让每个波的周期都是 $T$ 的整数分之一，海面就能无缝循环。**代价**：频谱被离散化，只能近似目标海谱（`FFT_SpectrumVerify.cs:44`）。
- **数值验证工具链**：`CopyTexture → ReadPixels` + `_DebugOutput` 统计，把频谱/位移/Jacobian 画成图（`SpectrumDebug_*.png`、`FFTVerify/Output/*.exr`）——**海面是最容易被"看着还行"骗过去的效果，所以先建数值可观测性**。
- **渲染侧**：SSS 四件套（波峰 / 视角 / 阴影补光 / SH），`GlossyEnvironmentReflection` + Schlick 反射，**无屏幕空间折射**（折射只在水下后处理）；网格是单张 200 m 平面 + 距离自适应曲面细分 + 视锥整三角形剔除。
- **LOD 同源化**：`apply_lod_origin_v5_min.py` 把 Gerstner 循环拆成"基础波 8 + 按 LOD 的细节波区间"——**同一个波谱、不同精度**，避免 LOD 切换时波浪结构跳变。
- **性能**：逐像素采样天空 cubemap 太贵 → 128³ cubemap 按需更新（太阳 >0.5° 或高度 >0.2 km 才重建）。

## 与其它节点的关系

- 表面着色（Beckmann/Smith/Schlick、SSS）属「光照模型与微表面 BRDF」；
- 泡沫的 Jacobian 行列式属「图形数学基础」（待建：几何与微分）；
- 大气/天空部分见「方案 · 实时大气散射链路」——同一工程里海与天是两条独立链路。

## 可能被追问的三层

1. **原理层**：JONSWAP 各参数（风速、峰频、峰升因子 $\gamma$）的物理含义？为什么深水色散是 $\omega=\sqrt{gk}$（含 $\tanh(kh)$ 的完整形式是什么）？为什么 Hermitian 对称能让 IFFT 输出实数？"统计波谱"与"流体模拟"的本质区别是什么？
2. **实现层**：Stockham 与 Cooley-Tukey 的区别、为什么 GPU 上更合适？共轭打包省了什么（内存/带宽）？频率量化到 $2\pi/T$ 对谱形状的影响有多大？4 层的 Tile 与贡献权重怎么定的？
3. **边界层**：为什么 FFT 海面做不出**卷曲破碎浪**（单值高度场 vs 多值曲面）？200 s 循环对游戏体验意味着什么？平铺重复怎么根除（多层 + 变形 + 视差）？要做波物交互（浮力/船）需要补什么？

## 手写/白板题（自测）

1. 写出 JONSWAP 谱的形式与方向扩散函数的作用。
2. 写出时间演化公式与色散关系，并解释"为什么每个波按自己的相速度走"。
3. 推一下"频域求导"为什么等价于时域微分（$\mathcal F[f']=ik\mathcal F[f]$）。
4. 写出二维 Jacobian 行列式，并说明 $J<0$ 的几何含义。

## 证据

- 工程：`D:\unity\unityProjectLocation\water\Assets\FFTVerify\`（`FFTOcean_Complete.compute:8-13,337-363`、`FFT_SpectrumVerify.cs:44,91-124`、`FFTOcean_Render.cs:37-44`、`FFTOcean_Render.shader:90-226,375-405`）
- 知识库（完整推导链）：`G:\water\.chain` → `方案 · FFT 统计波谱法` 及其子节点（海面能量谱：JONSWAP、方向扩散函数、色散关系与时间演化、共轭打包与 Hermitian 对称、Stockham IFFT、频域解析求导、Jacobian 折叠检测与泡沫、多层频谱叠加与平铺、水面材质渲染、频谱数值验证工具链）
- 侦察报告：`artifacts/water/water-侦察报告-2026-09-21.md`
- 参考：Tessendorf, *Simulating Ocean Water*, SIGGRAPH Course Notes 2001；Hasselmann et al. 1973（JONSWAP）；ChenHanMK1/FFT-Ocean-Code（工程参考实现——**面试时这条边界要说清**）
