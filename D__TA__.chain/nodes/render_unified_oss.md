---
id: render_unified_oss
type: note
title: 项目 · render_unified_oss（四后端统一渲染器）
parent: 项目深挖
status: none
tags:
- OpenGL
- CUDA
- ReSTIR
- 路径追踪
- 回归判定链
evidence:
- artifacts/render_unified_oss/render_unified_oss-侦察报告-2026-09-21.md
created: 2026-09-21T20:50:00+08:00
updated: 2026-09-21T20:50:00+08:00
revision: 1
---

> 触发：render_unified_oss；四后端渲染器；ReSTIR 实现；白炉测试；渲染回归判定

# 项目 · render_unified_oss（四后端统一渲染器）

**一句话**：Windows 单机、**零渲染第三方依赖**的四后端统一学习型渲染器——同一个 Cornell box 场景由 4 个后端共享（光栅 CPU/GPU、路径追踪 CPU/GPU），按 1–4 切换，共用同一 `IRenderBackend` 接口；次含义是 **CPU/GPU 着色数学镜像统一**（`bsdf.h` ↔ `bsdf_device.h` 同式）。**不是**多图形 API 统一，也**不是**多效果统一管线。

**技术栈**：C++17 + CUDA 17（arch 75/80/86/89），CMake ≥3.24，Microsoft Visual C++ ；自研 **65 个源文件 / 15,894 行**；**0 个 shader**——GPU 计算在 7 个 `.cu` + 2 个 `.cuh`，显示层是**固定管线 GL + CUDA-GL interop + PBO**。`thirdparty/` 只有 glew/glm/imgui/rapidobj/stb 五个；OptiX 自动探测、缺失即 stub。

**渲染能力**：PBR Cook-Torrance/GGX、Kulla-Conty 多散射补偿（32×32 LUT）、Heitz 2018 VNDF、玻璃 BTDF（Walter 2007）、MIS + NEE、**ReSTIR DI 与 ReSTIR GI**、**SVGF**（时间重投影 + à-trous）、OptiX AI 降噪（可选）、自研 FFT 生成蓝噪声、色调映射与自动曝光。
**未实现**（要主动说，别被问穿）：SSAO、SSR、IBL/环境贴图、体积光与参与介质、延迟管线/GBuffer、景深/运动模糊、纹理贴图、SSS。

## 关键难题 → 实现办法

| 问题 | 办法 | 代价 | 证据 |
|---|---|---|---|
| ReSTIR 采样数 N=32 与 N=1 **看不出差别** | 三层归因（M-cap 硬截断是"均衡器"／空间复用稀释／SVGF 兜底）→ 帧窗口制 `dynCap = N×F`，只压缩历史不稀释新样本 | DI/GI 两处 merge 同步改，储层需补 pdf | `d-010`、`d-011`、`path_trace_gpu_backend.cu:1042-1067,2011-2041` |
| 金属球渲染得"像玻璃球" | 视觉判图 + 数值双轨：球内峰值 RGB **CPU 153 / GPU 5.6** → 根因是 `packColor` **硬钳制白斑**，不是材质错 → 显示链改软拐点 tone map | 改显示链 = 改全模式观感，须重建基线 | `d-013`、`t-028`、`t-029` |
| PT CPU 128 SPP 反而更噪 | F0=0.04 的墙面却 50/50 分样本 → `p_spec = clamp(F_avg(F0), 0.02, 0.98)` + maxDepth 5→8 | 权重与 MIS pdf 需同步换算 | `t-011`、`src/shading/bsdf.cpp` |
| GGX 白炉能量只剩 **0.335** | Kulla-Conty LUT + VNDF 采样 → 白炉 **0.9962–1.0030（±1%）**；VNDF 方差 **0.136 vs 旧 0.747（5.5×）** | LUT 与采样器同源 ⇒ 自洽测试测不出共享偏差，必须独立仲裁 | `t-010` |
| 加载模型后关窗 /GS 崩溃 | int3 + DR0 写监视 + 反汇编，**证伪"模型相关"**：实为 **ODR 违例**（`main.cpp.obj` sizeof 0x180 vs 0x190 写穿栈 Cookie）→ 全量重建 + `OBJECT_DEPENDS` 根治 | 改任一 include 头即全量重编 | `t-003`、`CMakeLists.txt:100-119` |
| 中文 MSVC `/showIncludes` 输出"注意: 包含文件:" | depfile 全失效、头文件改了**静默不重编** → `OBJECT_DEPENDS` 挂全部头 | 编译时间换正确性 | `PROCESS_LOG:29` |
| PT CPU 端到端**不可位级复现**（max_abs_diff≈15–17、~0.4% 像素） | 分后端口径：光栅**位级**、路径追踪**统计级 PSNR≥50 dB** | 放弃位级回归；**根因未闭环** | `d-012`、`PROCESS_LOG:15` |
| `--autotest` 解析 `found+9` **off-by-one** | 修复后发现此前的"四模式冒烟"**实际全在跑 mode 4** | — | `PROCESS_LOG:26` |
| VNDF 跨坐标系移植（y-up） | `T2 = Vh × T1` 会让法线分量变负 → 改 `T2 = T1 × Vh`，三方仲裁收敛 0.05% | — | `t-010`、`PROCESS_LOG:23` |

## 验证与评测体系（本项目最强的面试素材）

- **`eval/`**：零依赖可插拔评判核心（C++17 `frame_dump` 输出 PFM/PPM/BMP + Python3 `compare.py` / `selftest.py`），**三步即可接入其它工程**；判据 `max_abs_diff` / `PSNR`（峰值钉死 `max(reference)`）/ `mean_rel_err`；退出码 0=PASS，单行可解析输出。
- **`benchmarks/`**：4 个程序化场景 × 4 模式 = **20 张基线图** + `meta.json`（记录 commit / 分辨率 / passes / **采样语义**）；512-pass 参考图 + `curve.csv` 收敛曲线：**pass 1 = 34.97 dB → pass 64 = 53.39 dB**。
- **`tools/regress.ps1`**：三态判定链——冒烟（exit 0 + 无 crash log）→ `-Compare`（`--passes 8 --dump`，光栅要求 `max_abs_diff = 0`、PT 要求 `PSNR ≥ 50 dB`）→ `-UpdateBaseline`（需 `-Force`/交互确认，防误覆盖）。**把架构文档 §7 从"纸面政策"变成可执行判定**（`d-012` 原文批评旧版"纯黑画面也能 PASS"）。
- **`vision_reports/`**：报告落盘时带时间戳 + 编译期注入 git hash + 参数摘要。实测 8-pass vs 512-pass：**PSNR 30.6578 dB**、mean_rel_err 4.506，8×8 分块矩阵显示误差集中在地板区（块值 158.93）。

## 工具链

- `tools/gen_bench_scenes.py`：纯标准库、**完全确定性**生成 4 个基准场景 OBJ/MTL（粗糙度阶梯+金属排／多发光板／低 SPP 暗场压力／纵深遮挡走廊），重生成为字节一致 → 消除手工建模与场景不可复现。
- `tools/generate_blue_noise.py`：FFT 频谱整形生成 64×64 双通道蓝噪声（seed 42/137、PCG64）→ 消除外部贴图依赖。
- `docx/`：5 份外部 AI 评审文档（架构规划书 v2、UI 材质分析、评判任务方案、"未解决技术问题 P1–P6"）。

## 风险与缺口（面试前必须知道，否则会被问穿）

- **不得主张为本人撰写**：`docx/` 5 份文档带 AIGC 元数据（标注"扣子 Coze"），是**外部 AI 生成的评审输入**。可以说"我用 AI 做过架构评审并据此落地"，不能说"我写了这些方案"。
- **性能证据极弱**：全仓库只有 `README.md:12` 一句"~95 ms/frame @1280×720 RTX 4060 Laptop"，**没有**帧时间 CSV、draw call 计数、带宽工具或 Nsight 报告。→ 要么补测，要么明确说"未做性能量化"。
- **单元测试不在仓库内**：`microfacet_test`（44 断言）/ `model_test` 位于 gitignore 的 `build/verify-models/`，仓库内可跑的只有 `eval/selftest.py`。→ 别说"有完整单测"。
- **`_ref/`（178 文件）是第三方参考实现**（CPUPathTracing / L19 / L21），不计入自研产出。
- **劳动量不可从 git 推断**：仅 28 commits / 5 个活跃日，且 initial commit 一次性 133,643 行。
- 光栅是 CUDA **软件光栅化**（每像素一线程），**不适用 draw call / 带宽话题**——被问到时要说清架构。
- PT CPU 不可位级复现的根因 **未闭环**；工作树不干净（`out/`、`.vs/`、`*.obj` 未纳管）。

## 可能被追问的三层

1. **原理层**：ReSTIR 的 RIS 权重与无偏性条件？M-cap 为什么会"均衡化"样本质量？Kulla-Conty 补偿解决的是能量哪一部分损失？SVGF 与 ReSTIR 的时间复用会不会打架？
2. **实现层**：VNDF 采样为什么方差更低？白炉测试怎么构造（为什么测的是能量守恒而不是画面）？`dynCap` 的 F 怎么选？PFM 为什么选它做中间格式？
3. **边界层**：为什么不给 PT 也做位级回归（浮点非结合性）？这套判定链怎么用到别的工程（三步接入的具体内容）？如果要做真实性能优化，先测什么？

## 证据索引

- 图谱：`G:\openGL\render_unified_oss\.chain\nodes\`（`d-001` 架构决策、`d-010`/`d-011` ReSTIR 诊断、`d-012` 评判设计、`d-013` 显示链、`t-003` ODR、`t-010` 白炉/VNDF、`t-011` pdf、`t-027` SSIM/FLIP 待做、`t-028`/`t-029` tone map、`v-001..v-031` 验收）
- 代码与文档：`ARCHITECTURE.md`（"架构宪法"：单向依赖 `core ← assets ← shading/backends ← denoising ← app`）、`eval/README.md`、`benchmarks/`、`tools/regress.ps1`、`tools/gen_bench_scenes.py`、`tools/generate_blue_noise.py`、`src/raster/display.cpp|.cu`、`include/shading/microfacet.h`、`src/shading/bsdf.cpp`、`path_trace_gpu_backend.cu`、`temporal_reproject.cu`、`svgf_atrous.cu`
