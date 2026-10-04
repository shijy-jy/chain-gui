---
id: Stockham IFFT
type: note
title: Stockham IFFT
parent: 方案 · FFT 统计波谱法
status: none
rel: contains
tags:
- 图形渲染
- 海洋
- FFT
- 算法
revision: 1
updated: 2026-09-08T20:22:48+08:00
---

> 触发：Stockham IFFT；1024 点复数；2D IFFT

# Stockham IFFT

FFT 海洋的变换载体：1024 点复数 IFFT，横向 + 纵向各一遍 = 2D IFFT。

## 为什么是 Stockham 变体

经典 Cooley-Tukey FFT 需要 **bit-reversal 重排**（输入按位逆序排列）；Stockham 自排序变体把重排融入蝶形索引公式，**免显式重排**，GPU 上省一次全局置换。

## 工程实现（`CS_HorizontalIFFT` / `CS_VerticalIFFT`）

- `[numthreads(1024,1,1)]`：**一整行数据放进一个线程组**，线程 $i$ 处理行内第 $i$ 个元素；
- 数据载体：`groupshared float4 fftGroupBuffer[2][1024]` **乒乓缓冲**，10 级蝶形（$=\log_2 1024$）交替读写，每级后 `GroupMemoryBarrierWithGroupSync()` 同步；
- 蝶形索引与旋转因子（`ButterFlyValues`）：

$$
b = \frac{N}{2^{step+1}},\quad w = b\left\lfloor\frac{i}{b}\right\rfloor,\quad
\text{旋转因子}=e^{-i\,2\pi w/N}\ (\text{IFFT 取负号})
$$

旋转因子用 `sincos` 一次求值。注意工程 `twiddle.y = -twiddle.y` 是把 FFT 旋转因子改号为 IFFT。

- 横向：`_FourierTarget[id.xy]`；纵向：`_FourierTarget[id.yx]`（转置寻址，无需真实转置矩阵）——两次调用即完成 2D 变换。

## 两个工程细节

1. **复数打包复用**：两个实场打包成 $a+ib$ 做一次复 IFFT（实部/虚部各出一个场）——位移 X+Z、斜率 X+Z 都是这样成对变换的，IFFT 次数减半。
2. **无 $1/N$ 归一化**：IFFT 的 $\frac{1}{N}$ 因子被吸收进 $h_0$ 构造时的缩放与 `_WaveSharp` 参数，GPU 端省一次全局乘。

## 相关节点

- 共轭打包与 Hermitian 对称（输入布局）
- 频域解析求导：位移与斜率（IFFT 的输入从哪来）
