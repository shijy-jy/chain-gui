---
id: ReSTIR时空复用
type: note
title: 方案 · ReSTIR 时空复用（含无偏性的"成对结构"）
parent: 渲染与着色器
status: none
tags:
- ReSTIR
- RIS
- 时空复用
- M-cap
- 无偏性
created: 2026-09-22T03:00:00+08:00
updated: 2026-09-22T03:00:00+08:00
revision: 1
---

> 触发：ReSTIR 原理；蓄水池跨帧复用；M-cap 为什么有偏；DI 与 GI 的区别；时空重采样

# 方案 · ReSTIR 时空复用（含无偏性的"成对结构"）

**来源**：我在 `G:\learning` 里的完整推导（数学物理版），以及我在 `render_unified_oss` 里的实现与实测（`d-010`/`d-011`/`t-014`）。**这是整个技术栈里最能打的一个话题**：既懂推导、又实现过、还诊断过它的偏差来源。

## 一、要解决什么

实时路径追踪每像素只有 1~少数几条光线，直接光的方差大到不可用。ReSTIR 的思路：**把样本借给邻居像素与上一帧**，有效样本数从 $O(1)$ 提到 $O(\text{邻居数}\times\text{帧数})$。

数学载体两个：**RIS**（重采样重要性采样）+ **蓄水池（reservoir）合并**。

## 二、RIS：无偏性的"成对结构"（全篇最重要的一步）

目标（未归一化）$\hat p$，源分布 $p_{src}$，候选 $Y_1..Y_M \sim p_{src}$（独立同分布），权重 $w_i=\hat p(Y_i)/p_{src}(Y_i)$，$W=\sum w_i$，以 $w_i/W$ 的概率选中 $Y^*$。估计量：

$$F := \frac{W}{M}\cdot\frac{f(Y^*)}{\hat p(Y^*)},\qquad \mathbb{E}[F]=\int f$$

**证明的关键结构**：选中概率里的 $W$ 与补偿因子里的 $W$ **成对出现、恰好约掉**——展开全期望后

$$\mathbb{E}[F]=\frac1M\sum_j \mathbb{E}\left[\frac{w_j f(Y_j)}{\hat p(Y_j)}\right]=\frac1M\sum_j \int p_{src}\cdot\frac{\hat p}{p_{src}}\cdot\frac{f}{\hat p}\,\mathrm dx=\int f$$

> **一句话记住**：**无偏性来自"成对结构"（$W$ 在选中概率与补偿因子里配对），而不是来自某个聪明的公式。** 后面所有的偏差分析都变成同一句话——**哪里让成对结构断裂，哪里就有偏差。**

**朴素 SIR**（丢掉 $W/M$）是**有偏但一致**的：$M$ 有限时不等于目标分布，但 $M\to\infty$ 由强大数定律收敛（$\frac{W}{M}\xrightarrow{a.s.}Z$）。

**蓄水池合并引理**：两 reservoir 合并（$W_1+W_2$、$M_1+M_2$，以 $W_2/(W_1+W_2)$ 取第二个的样本）与"两候选流拼接后整体 WRS"**同分布**——这就是时空复用的数学合法性。

## 三、ReSTIR DI

**目标构造**（物理基础）把被积函数写成 $\hat p\cdot V$ 的形式：

$$L_o=\int_A \underbrace{\frac{\rho}{\pi}L_e\,G}_{=:\hat p(y)}\,V(y)\,\mathrm dA,\qquad G=\frac{\cos\theta_x\cos\theta_y}{r^2},\qquad p_{src}=\frac{1}{N_LA},\qquad w=\frac{\rho}{\pi}L_eG\,N_LA$$

- **无 V 版**：$F=\frac{W}{M}V(Y^*)$——候选不射阴影线，选中后补 1 条，**权重便宜**；
- **含 V 版**：$F=W/M$——每个候选都射阴影线，**权重贵但方差低**。
两者都无偏：**选哪个 $\hat p$ 只影响方差与成本，不影响正确性。**

**时间复用**：上一帧对应像素（重投影 + 深度/法线门）按合并引理并入。**合并不破坏无偏**（拼接流仍是同目标同源）。

**M-cap 的偏差从哪来（精确来源）**：$W\leftarrow W\cdot M_{cap}/M,\ M\leftarrow M_{cap}$。被丢弃的 $M-M_{cap}$ 份权重**已经参与了选中、却不再出现在 $W$ 里**——成对结构断裂，约不掉了。断裂量 ∝ 丢弃份额 $(M-M_{cap})/M$，$M_{cap}\to\infty$ 时消失 → **有偏但一致**。
**与俄罗斯轮盘同构**：RR 在**路径长度轴**上做同样的 Bias²↔Var 交易，M-cap 在**样本历史轴**上做——同一笔账。

## 四、我在实现里对 M-cap 的改造（面试加分项）

**洞察：cap 的单位应该是"帧数"而不是"样本数"**——cap 的唯一目的是控制**时间偏置**（旧样本过期），而过期的时间尺度是**帧**：

$$\mathrm{dynCap}=N\times F\quad(N=\text{每帧候选数},\ F=\text{历史帧窗口，默认 30})$$

- $N{=}1,F{=}30$ → cap=30（与传统一致，无回归）；$N{=}32,F{=}30$ → **cap=960**，质量随 $N$ 线性增长、不再被固定 cap 硬截断。
- **旧式合并的问题**：$W_{new}=(W_{cur}+W_{prev})\cdot\frac{cap}{M}$ 把**本帧新样本也一起稀释**了。改进为**截断只吃历史**：$m_{prev}=\min(prev.M,\mathrm{dynCap})$，$w_{prev}=prev.wSum\cdot\frac{m_{prev}}{prev.M}$，$W_{new}=r.wSum+w_{prev}$ → 稳态 $M\to N(F+1)$。
- **这正是"成对结构断裂"分析的工程应用**：偏差只应来自被丢弃的历史权重，**不应连带稀释新权重**。
- 实测与诊断：`render_unified_oss` 的 `dynCap` 实现（`path_trace_gpu_backend.cu:1042-1067,2011-2041`）；以及反直觉现象——**N=32 与 N=1 看不出差别**，三层归因（M-cap 硬截断把样本质量均衡化 / 空间复用稀释 / SVGF 兜底），稳态推导有效样本 ~960 vs ~30。

## 五、空间复用：为什么 DI 不需要 Jacobian

邻居样本是用 $\hat p_{nbr}$ 选出的，并入当前像素必须换算到当前目标：

$$W \mathrel{+}= W_{nbr}\cdot\frac{\hat p_{cur}(y_j)}{\hat p_{nbr}(y_j)}$$

**DI 的样本定义在同一个域（光源表面）**——样本不动、只有目标值变，域映射是**恒等映射、Jacobian = 1**。这就是"DI 空间复用不需要 Jacobian"的完整原因（DI 是 GRIS 的恒等特例）。

**可见性复用**：跳过当前帧的新阴影射线 → 权重里的 $V$ 与估计量里的 $V$ 失配 → **跨阴影边界漏光**。跳过 = 有偏省射线；不跳 = 无偏贵成本。

## 六、ReSTIR GI 与 GRIS

GI 的样本是**路径** $\bar x=(x_0,\dots,x_k)$，每个像素的路径域不同（$x_0$ 不同）→ DI 的同域重加权失效。

**重连（reconnection）**：借用 $\bar x'$，重建 $\bar x=(x_0,x_1',\dots)$，条件 $V(x_0\to x_1')$ 可见且表面门通过；失败即贡献 0（同 RR 终止分支），接受者按当前 $\hat p$ 合并。
**重连是"硬"的**：接不上就丢、红利损失、尾部错配无法解析补偿。

**GRIS** 把重连推广为**移位映射** $T:\bar x'\mapsto \bar x$（重连 = 头部换锚 + 尾部恒等），可逆时用 Jacobian 补偿：

$$w=\frac{\hat p_{cur}(T(\bar x'))\left|\det\frac{\partial T}{\partial \bar x'}\right|}{p_{src}(\bar x')}$$

**DI 与 GRIS 在这个公式下统一**（DI：$T=$ 恒等、Jacobian=1）。

## 七、无偏性总账（面试必答表）

| 配置 | 无偏性 | 偏差来源 |
|---|---|---|
| 仅初始候选 | ✅ 精确无偏 | —— |
| + 时间复用（无 cap） | ✅ 精确无偏 | 合并 = 拼接流 |
| + M 钳制 | ⚠️ 有偏但一致 | 成对结构断裂 ∝ 丢弃份额 |
| + 空间复用（target ratio） | ✅ 期望无偏 | 重加权恢复成对结构 |
| + 可见性复用跳过 | ⚠️ 有偏 | $V$ 失配 → 漏光 |

## 八、可能被追问的三层

1. **原理层**：RIS 无偏性为什么依赖"$W$ 成对出现"？朴素 SIR 的一致性怎么证明（两步 SLLN）？M-cap 与 RR 的同构关系？为什么 DI 空间复用 Jacobian=1 而 GI 不是？
2. **实现层**：reservoir 里为什么要存原始 pdf（不存就没法在别的像素重评估）？`dynCap` 的 $F$ 怎么选（相机脏标记压到 4~8、重投影失效率驱动自适应）？可见性复用与阴影射线预算怎么权衡？
3. **边界层**：ReSTIR 在动态场景/大量光源/透明材质下哪里会崩？偏差的最终出路是什么（我在推导里的结论是：**不是"更巧的截断"，而是"更准的重评估"**——历史权重按当前目标重评估 = 广义 MIS 的恒等映射重加权）？GRIS 之后还有什么（神经/学习型采样）？

## 九、手写/白板题（自测）

1. 写出 RIS 估计量与无偏性证明的关键一步，并指出"成对结构"在哪两处出现。
2. 写出 $G$ 的定义与 $\hat p$ 的构造，说明为什么要把被积函数写成 $\hat p\cdot V$。
3. 写出无偏性总账表（五种配置的无偏性与偏差来源）。
4. 说明 DI 空间复用为什么 Jacobian = 1，GI 为什么不行、要怎么补。

## 十、证据

- `G:\learning\.chain`：`方案 · ReSTIR时空复用`（本节点主源：完整推导、RIS 定理与证明、M-cap 偏差来源、dynCap 设计、GI 重连与 GRIS、辩证卡⑮–⑳、10 条公式速查与书目）、`无偏性与收敛性`（MSE = Var + Bias²）、`方案 · SIR重采样`、`方案 · WRS蓄水池流式`、`局限 · 有偏近似与可见性复用失效`
- `G:\openGL\render_unified_oss`：`d-010`/`d-011`（我的实现在 N=32 时的反直觉现象与三层归因）、`t-014`、`path_trace_gpu_backend.cu:788/876/952/1077`（DI）、`:1427/1820/2049/2153`（GI）
- 书目：Bitterli et al. SIGGRAPH 2020；Ouyang et al. HPG 2021（ReSTIR GI）；Talbot et al. EGSR 2005（RIS）；Lin, Wyman, Yuksel SIGGRAPH 2022（GRIS）
