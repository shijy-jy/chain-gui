# render_unified_oss 侦察报告

> 来源：2026-09-21 只读侦察（源码 + 工程图谱 `.chain` 86 节点 + 文档 + 只读 git 命令；未运行构建或二进制、未修改任何文件）
> 对应节点：`render_unified_oss`（项目 · render_unified_oss（四后端统一渲染器））
> 用途：面试前重读细节的原始证据，节点正文是提炼版

## 0. 一句话

Windows 单机、零渲染第三方依赖的四后端统一学习型渲染器：同一 Cornell box 场景由 4 个后端共享，C++17 + CUDA 手工实现，配一套**可执行的**效果判定链 + 86 节点工程记忆图谱。证据 `README.md:3-12`、`ARCHITECTURE.md:9-37`。

## 1. 项目定位：unified = 单一场景多后端

- 主含义＝**单场景多后端统一**：RasterCPU / RasterGPU / PathTraceCPU / PathTraceGPU 共享同一场景 + 同一 `IRenderBackend`，按键 1–4 切换（`README.md:3-12`、`d-001` 核心决策 1、`include/backends/render_backend.h`）。
- 次含义＝**CPU/GPU 着色数学统一**：`bsdf.h` 与 `bsdf_device.h` 镜像同式（`t-010` 原文），跨设备契约集中管控（`ARCHITECTURE.md:48-63`）。
- **不是**多图形 API 统一（只有 OpenGL 显示 + CUDA 计算），**不是**多效果统一管线。
- `ARCHITECTURE.md` 自称"架构宪法"：三层同心圆 + 单向依赖铁律 `core ← assets ← shading/backends ← denoising ← app`。

## 2. 技术栈与规模（全部实测）

- C++17 + CUDA 17；CMake ≥3.24；CUDA 架构 75/80/86/89（`CMakeLists.txt:3,10-13,17,21`）
- `thirdparty/`：glew、glm、imgui、rapidobj、stb（5 个）；OptiX SDK 自动探测、缺失即 stub（`CMakeLists.txt:121-148`、`THIRD_PARTY.md`）
- Windows x64 专属（Win32 + winhttp + opengl32）
- 自研 **65 个源文件 / 15,894 行**（src/ + include/ + eval/）；最大 `path_trace_gpu_backend.cu` 114.7KB、`application.cpp` 56.3KB
- **Shader = 0 个**（无 GLSL/HLSL/SPIR-V）；GPU 计算在 7 个 `.cu` + 2 个 `.cuh`；显示层是**固定管线 GL**（`glMatrixMode`/`glVertex2f`/`glTexCoord2f`）+ CUDA-GL interop + PBO（`src/raster/display.cpp|.cu`）
- git：**28 commits**，单作者 jinyu，**5 个活跃日**（2026-08-16/19、09-02/03/08）；initial commit 534 文件/133,643 行，此后 166 文件 +83,176/−2,892
- **无 CI**（无 `.github/`）

## 3. 渲染能力清单（逐项附路径）

已实现：PBR Cook-Torrance/GGX（`include/shading/microfacet.h`、`bsdf.cpp`、`bsdf_device.h`）；Kulla-Conty 多散射补偿（`ggx_energy_lut.h` 32×32 LUT）；Heitz 2018 VNDF（`microfacet.h sampleGGXVndf`）；玻璃 BTDF（Walter 2007）；MIS + NEE；**ReSTIR DI**（`path_trace_gpu_backend.cu:788/876/952/1077`）+ **ReSTIR GI**（`:1427/1820/2049/2153`）；**SVGF**（`temporal_reproject.cu:21` + `svgf_atrous.cu:17,45`）；OptiX AI 降噪（`optix_denoiser.cpp`，可选）；蓝噪声（`blue_noise_data.cuh` + 自研 FFT 生成器）；色调映射/自动曝光（`halfTonemapKernel:1178`、`logLumReduceKernel:1289`）；PT CPU 软拐点 tone map（`t-029`）。

阴影/可见性：shadow ray/occluded **仅存在于 PT 后端**（`path_trace_cpu_backend.cpp` 8 处、`path_trace_gpu_backend.cu` 30 处），软阴影来自面积顶灯采样，**无独立 shadow map pass**。

**未实现**（区分大小写 grep 0 命中）：SSAO、SSR、IBL/环境贴图、体积光/参与介质、延迟管线/GBuffer、景深/运动模糊、纹理贴图、清漆/薄片/SSS。

## 4. 关键难题 → 实现办法

1. ReSTIR N=32 与 N=1 无差别 → 三层归因（**M-cap 硬截断是均衡器**／空间复用稀释／SVGF 兜底）→ 帧窗口制 `dynCap=N×F`，只压缩历史不稀释新样本；代价：DI/GI 两处 merge 同步改、储层需补 pdf（`d-010`、`d-011`、`.cu:1042-1067/2011-2041`）
2. 金属球"像玻璃球" → 视觉判图 + 数值（球内峰值 RGB **153** vs GPU **5.6**）→ 根因是 `packColor` 硬钳制白斑，**非材质错** → 显示链软拐点 tone map；代价：改显示链＝改全模式观感须重建基线（`d-013`、`t-028`、`t-029`）
3. PT CPU 128 SPP 更噪 → F0=0.04 墙面却 50/50 分样本 → `p_spec=clamp(F_avg(F0),0.02,0.98)` + maxDepth 5→8；代价：权重与 MIS pdf 同步换算（`t-011`、`src/shading/bsdf.cpp`）
4. GGX 白炉能量只剩 **0.335** → KC LUT + VNDF → 白炉 **0.9962–1.0030（±1%）**、VNDF 方差 **0.136 vs 旧 0.747（5.5×）**；代价：LUT 与采样器同源自洽测试测不出共享偏差，须独立仲裁（`t-010`）
5. 加载模型后关窗 /GS 崩溃 → int3 + DR0 写监视 + 反汇编 → 证伪"模型相关"，实为 **ODR 违例**（`main.cpp.obj` sizeof=0x180 vs 0x190 写穿栈 Cookie）→ 全量重建 + OBJECT_DEPENDS 根治；代价：改任一 include 头即全量重编（`t-003`、`CMakeLists.txt:100-119`）
6. 中文 MSVC `/showIncludes` 输出"注意: 包含文件:" → depfile 全失效、头改静默不重编 → OBJECT_DEPENDS 挂全部头（`PROCESS_LOG:29`）
7. PT CPU 端到端不可位级复现（max_abs_diff≈15–17、~0.4% 像素集边缘）→ 分后端口径：Raster 位级、PT **统计级 PSNR≥50dB**；代价：放弃位级回归，**根因未闭环**（`d-012`、`PROCESS_LOG:15`）
8. `--passes` 与默认 10s 墙钟并发 → 慢机 pass 数漂移（实测 15 vs 9）→ `--passes` 独占终止条件（`PROCESS_LOG:16`）
9. `--autotest` 解析 `found+9` off-by-one → 此前所有"四模式冒烟"**实际全跑 mode 4**（`PROCESS_LOG:26`）
10. VNDF 跨坐标系移植：y-up 下 `T2=Vh×T1` 法线分量变负 → 改 `T2=T1×Vh`，三方仲裁收敛 0.05%（`t-010`、`PROCESS_LOG:23`）

## 5. 验证与评测体系（最强素材）

- **`eval/`** ＝ 零依赖可插拔评判核心（C++17 `frame_dump` PFM/PPM/BMP + Python3 `compare.py`/`selftest.py`），**3 步接入其它工程**；判据 `max_abs_diff`/`PSNR`（峰值钉死 `max(reference)`）/`mean_rel_err`；退出码 0=PASS、单行可解析（`eval/README.md:16-32,50-65`、`d-012`）
- **`benchmarks/`**：4 程序化场景 × 4 模式 = **20 张基线** + `meta.json`（记 commit/分辨率/passes/**采样语义**）；512-pass 参考图 + `curve.csv` 收敛曲线：**pass 1 = 34.97dB → pass 64 = 53.39dB**
- **`tools/regress.ps1`**：冒烟（exit 0 + 无 crash log）→ `-Compare`（`--passes 8 --dump` 判指标：Raster `max_abs_diff=0`、PT `PSNR≥50dB`）→ `-UpdateBaseline`（需 `-Force`/交互确认防误覆盖）。**把 ARCHITECTURE §7 从"文档政策"变成可执行判定链**（`d-012` 原文批评旧版"纯黑也能 PASS"）
- **`vision_reports/`**：带时间戳 + 编译期注入 git hash + 参数摘要的报告落盘。实测 `comparecli_latest.md`：8-pass vs 512-pass **PSNR 30.6578dB**、mean_rel_err 4.506，8×8 分块矩阵显示误差集中在地板区（块值 **158.93**）
- 结论：截图比对 / 指标阈值 / 确定性测量三者齐全且分后端定口径。弱项：无 CI 门禁（位级一致仅承诺同机同驱动）、无 SSIM/FLIP（t-027 待做）

## 6. 工具链

- `tools/regress.ps1` 回归+判定链接线
- `tools/gen_bench_scenes.py`：纯标准库、**完全确定性**生成 4 基准场景 OBJ/MTL（粗糙度阶梯+金属排／多发光板／低 SPP 暗场压力／纵深遮挡走廊），重生成为字节一致 → 消除手工建模与场景不可复现
- `tools/generate_blue_noise.py`：FFT 频谱整形生成 64×64 双通道蓝噪声（seed 42/137、PCG64）→ 消除贴图依赖，`THIRD_PARTY.md:28-33` 明确自研归因
- `docx/`：5 份**带 AIGC 元数据的外部 AI 评审文档**（架构规划书 v2、UI 材质分析、评判任务方案、评判评审、"未解决技术问题 P1–P6"），派生 `d-008`/`d-012`/`t-010`
- 隐性：`build/verify-models/` 下离线工具（`precompute_kc.cpp`、`ui_glyph_audit.cpp`、`minidebugger.c`），gitignore 但节点留档

## 7. 面试素材 6 条（证据强度）

1. **【强】** 可执行渲染回归判定链（零依赖 eval/ + 20 基线 + 三态脚本 + 分后端口径）
2. **【强】** ReSTIR M-cap 饱和的定量诊断与设计（三层归因 + 稳态推导 ~960 vs ~30 有效样本）
3. **【强】** PBR 数学正确性的独立交叉仲裁（白炉 ±1%、VNDF 方差 5.5×、**三次自我证伪**）
4. **【强】** ODR/栈 Cookie 崩溃逆向定位（把"加载模型崩溃"证伪成"关窗 ODR 违例"）
5. **【中】** 观感问题"数值+视觉"双轨诊断闭环（153 vs 5.6 → packColor 钳制 → tone map → 用户肉眼验收）
6. **【中】** 架构治理与零语义迁移（架构宪法 + M1–M4，1480 行拆分，迁移前后回归 4/4 PASS）

## 8. 不确定项

1. **单元测试不在仓库内**：`microfacet_test`(44 断言)/`model_test` 位于 gitignore 的 `build/verify-models/`，**无法核实**；仓库内可跑测试仅 `eval/selftest.py`
2. **性能数据极弱**：仅 `README.md:12` "~95 ms/frame @1280×720 RTX 4060 Laptop" 一处声明；**未找到**帧时间 CSV/draw call 计数/带宽工具/Nsight 报告；`ptcpu_stats.log` 只记 avg_lum
3. **draw call / 带宽主题不适用**：光栅是 CUDA 软件光栅化（每像素一线程），非 GL draw call 管线
4. PT CPU 不可位级复现的**根因未闭环**（`PROCESS_LOG:15` 自述"另立调查项"）
5. **docx/ 5 份带 AIGC 元数据**（`ContentProducer: 001191110102...`、标注"扣子(Coze)"），是外部 AI 生成的评审输入，不宜作为作者独立撰写物主张
6. `v-001..v-031`（31 个验收节点）内容高度模板化，结论未逐条核实；86 节点与 28 commit 不成比例
7. **图谱是后期批量回溯产物**：`t-032`/`t-035`/`t-036` 明写"回溯修改""代码内化"，且 `.chain/archive/pre_v11_code_nodes/` 存 12 个 pre-v11 归档节点
8. **`_ref/`（178 文件）是第三方参考实现**（CPUPathTracing / L19 / L21），非自研，不可计入作者产出
9. **工作树不干净**：含未纳入版本控制的 `*.obj`、`_mcp_*.jsonl`、`_out_*.jsonl`、`out/`、`.vs/`
10. **提交日期为 2026-08/09**+5 个活跃日+initial commit 一次性 133,643 行 → **真实开发时长/劳动量无法从 git 历史推断**

## 未决观察

最强三块：判定链、数学仲裁、崩溃根因定位（证据链完整、有实测数字）。最弱两块：性能证据、单元测试可核实性。面试前建议补帧时间测量，或明确承认未做性能量化。
