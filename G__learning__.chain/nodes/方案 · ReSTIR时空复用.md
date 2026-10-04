---
id: 方案 · ReSTIR时空复用
title: 方案 · ReSTIR时空复用
parent: 局限 · 相机移动破功且样本仍不足
rel: solves
tags:
- 方案
- 采用
- ReSTIR
- M2
- M3
revision: 1
updated: 2026-09-08T20:17:33+08:00
---

> 触发：ReSTIR 时空复用；蓄水池跨像素跨帧复用；时空重采样

# 方案 · ReSTIR 时空复用（数学物理推导版）

## 解决了什么 / 没解决什么（递进链）

- **解决**：[局限 · 相机移动破功且样本仍不足](局限 · 相机移动破功且样本仍不足.md)——把样本借给邻居像素与上一帧：有效样本数从 O(1) 提到 O(邻居数×帧数)。数学载体 = RIS（[方案 · SIR重采样](方案 · SIR重采样.md)）+ reservoir 合并（[方案 · WRS蓄水池流式](方案 · WRS蓄水池流式.md)）。
- **未解决（引出下一环）**：复用是近似共享 → [局限 · 有偏近似与可见性复用失效](局限 · 有偏近似与可见性复用失效.md)（M 钳制有偏、可见性复用漏光、GI 重连硬性丢弃）。

---

# 一、物理基础：G 从哪来（直接光照贡献的完整推导）

出射辐射率（LTE 直接光部分）：

$$L_o(x,\omega_o) = \int_{H^2} f_r(\omega_i)\, L_i(x,\omega_i)\,\cos\theta_x\,\mathrm{d}\omega_i$$

朗伯面 $f_r=\rho/\pi$；立体角元换面积元 $\mathrm{d}\omega_i = \dfrac{\cos\theta_y\,\mathrm{d}A}{r^2}$；两点间 Radiance 不变（$L_i = L_e V$）：

$$L_o = \frac{\rho}{\pi}\int_A L_e\, V\,\cos\theta_x\,\frac{\cos\theta_y}{r^2}\,\mathrm{d}A = \int_A \underbrace{\frac{\rho}{\pi}\,L_e\,G(y)}_{=:\ \hat p(y)}\,V(y)\,\mathrm{d}A, \qquad G(y) := \frac{\cos\theta_x\cos\theta_y}{r^2}$$

**成果**：被积函数写成 $\hat p\cdot V$ 的乘积——目标分布 $\hat p = \dfrac{\rho}{\pi}L_e G$（不含可见性的贡献），f 与 p̂ 只差一个 V。这是整个 DI 数学的支点。

# 二、RIS 的完整数学（定理与证明）

**设定**：目标（未归一化）$\hat p$，源 $p_{src}$，支持条件 $\hat p>0$ 当 $f\neq0$ 且 $p_{src}>0$ 当 $\hat p>0$。候选 $Y_1,\dots,Y_M\stackrel{iid}{\sim}p_{src}$，$w_i=\hat p(Y_i)/p_{src}(Y_i)$，$W=\sum w_i$，以 $w_i/W$ 选中 $Y^*$。

**定理 1（无偏 RIS）**：$F := \dfrac{W}{M}\cdot\dfrac{f(Y^*)}{\hat p(Y^*)}$ 满足 $\mathbb{E}[F]=\int f$。

**证明**（全期望公式）：

$$\mathbb{E}[F] = \mathbb{E}_c\Big[\mathbb{E}_{J|c}[F]\Big] = \mathbb{E}_c\!\left[\sum_{j=1}^{M}\frac{w_j}{W}\cdot\frac{W}{M}\cdot\frac{f(Y_j)}{\hat p(Y_j)}\right] = \frac{1}{M}\sum_{j=1}^{M}\mathbb{E}_c\!\left[\frac{w_j f(Y_j)}{\hat p(Y_j)}\right]$$

$$\mathbb{E}_{p_{src}}\!\left[\frac{w f}{\hat p}\right] = \int p_{src}\cdot\frac{\hat p}{p_{src}}\cdot\frac{f}{\hat p}\,\mathrm{d}x = \int f \quad\Longrightarrow\quad \mathbb{E}[F]=I\ \blacksquare$$

**观察（全篇最重要的结构）**：选中概率里的 $W$ 与补偿因子里的 $W$ **成对出现、恰好约掉**——无偏性完全来自这个成对结构。

**定理 2（朴素 SIR 有偏但一致）**：丢掉 $W/M$ 直接以 $Y^*$ 当目标样本：$M$ 有限则 $\mathbb{P}(Y^*\in A)\neq\int_A\hat p/Z$；但 $M\to\infty$ 收敛。**一致性证明**（SLLN 两步）：

$$\frac{W}{M}\xrightarrow{a.s.}Z, \qquad \mathbb{P}(Y^*\in A) = \frac{\frac1M\sum w_j\mathbf{1}_A}{\frac1M\sum w_j} \xrightarrow{a.s.} \frac{\mathbb{E}[w\mathbf{1}_A]}{\mathbb{E}[w]} = \frac{\int_A\hat p}{Z}\ \blacksquare$$

**WRS 合并引理**：reservoir 逐候选更新后 $\mathbb{P}(y=y_j)=w_j/W$（归纳）；**推论**：两 reservoir 合并（$W_1+W_2,\ M_1+M_2$，以 $W_2/(W_1+W_2)$ 取第二者样本）与"两候选流拼接后整体 WRS"同分布——这是时空复用的数学合法性。

# 三、ReSTIR DI 的数学

## 3.1 目标、源与两种可见性处置

$\hat p = \dfrac{\rho}{\pi}L_e G$，$p_{src}=\dfrac{1}{N_L A}$，$w = \dfrac{\rho}{\pi}L_e G\,N_L A$。取 $f=\hat p\,V$，定理 1 给出：

- **无 V 版**：$F = \dfrac{W}{M}V(Y^*)$——候选零阴影射线，选中补 1 条，权重便宜
- **含 V 版**：$\hat p_V=\hat p V$ → $f/\hat p_V=1$，$F=W/M$——每候选 1 条阴影射线，权重贵、方差低

**两种都无偏；选 p̂ 只影响方差与成本**（M1 结论兑现）。

## 3.2 时间复用与 M-cap 偏差的精确来源

上一帧对应像素（重投影 + 深度/法线门）按合并引理并入——**合并本身不破坏无偏**（拼接流仍是同目标同源的候选流，成对结构完好）。

**M-cap**：$W\leftarrow W\cdot\dfrac{M_{cap}}{M},\ M\leftarrow M_{cap}$。**偏差来源**：被丢弃的 $M-M_{cap}$ 份权重的候选**已经参与了选中、却不再出现在 W 里**——成对结构断裂，约不掉了。断裂量 ∝ 丢弃份额 $(M-M_{cap})/M$，$M_{cap}\to\infty$ 时消失 → **有偏但一致**。重缩放保持平均权重 $W/M$ 不变（Bitterli 2021），故近似无偏。**M-cap 与 RR 同构**：RR 在路径长度轴、M-cap 在样本历史轴，做同一笔 Bias²↔Var 交易（辩证卡⑨⑱）。

> **与帧间累积的关系（值层 ↔ 样本层）**：时间复用是帧间累积的**泛化**——累积混合"着色后的值"（历史的数死了），复用合并"样本证据"（可在当前像素重新评估）；同构旋钮：α ↔ M-cap、深度/法线验证 ↔ 有效性门。完整分析与帧间累积四方法谱系（帧平均/EMA/置信度混合/方差引导 + ESS）见 [方案 · 帧间累积](离线与实时的收敛.md)。

### M-cap 设计进阶：动态窗口（2026-09-03 提取自学习者工程设计）

**核心洞察：cap 的单位应该是"帧数"而不是"样本数"**——M-cap 的唯一目的是控制**时间偏置**（旧样本在重投影/光照变化后过期），而过期的时间尺度是**帧**：

$$\mathrm{dynCap} = N\times F,\qquad N=\text{每帧候选数},\ F=\text{历史帧窗口（默认 30）}$$

N=1/F=30 → cap=30（与传统一致，无回归）；N=32/F=30 → cap=960——**质量随 N 线性增长，不再被固定 cap 硬截断**。

**旧式合并的问题（改进的靶子）**：$W_{new}=(W_{cur}+W_{prev})\cdot\dfrac{cap}{M}$ 把**本帧新样本也一起稀释**了。改进——**截断只吃历史，不稀释新样本**：

$$m_{prev}=\min(prev.M,\ dynCap),\qquad w_{prev}=prev.wSum\cdot\frac{m_{prev}}{prev.M},\qquad W_{new}=r.wSum+w_{prev},\qquad M_{new}=r.M+m_{prev}$$

稳态 $M\to N(F+1)\approx N\times F$。数学上这正是"成对结构断裂"分析的工程应用：**偏差应只来自被丢弃的历史权重，而不是被连带稀释的新权重**——把断裂限制在历史段，本帧样本的成对结构完好。

**自适应 F（样本层的历史拒绝连续化）**：固定 F 只是把"硬"挪了个维度。两个信号：①相机脏标记——移动帧 F 压到 4~8 快速遗忘防鬼影；②重投影失效率 $\rho$（被深度/法线/可见性门拒绝的像素比例）驱动 $F=\mathrm{clamp}\big(F_{base}(1-\rho k),\ F_{min},\ F_{max}\big)$——**这正是值层"置信度混合"（history rejection）在样本层的连续化版本**，与上一节"验证门 ↔ 有效性门"的镜像完全接上。

**最终消解（与广义 MIS 合流）**：完整版里 cap 的语义退化为"MIS 权重分母里的偏置钳制"——配合"历史权重按当前目标 $\hat p_{cur}$ 重评估"（= target-ratio / 广义 MIS 的恒等映射重加权）后，硬截断的偏置基本消失。**这印证了辩证卡⑱⑲ 的预告：偏差的最终出路不是"更巧的截断"，而是"更准的重评估"。**（出处：学习者原项目 render_unified_oss 的工程设计；储层需补存原始 pdf 的细节见其 t-014 节点。）

## 3.3 空间复用：target ratio 重加权与"DI 为什么没有 Jacobian"

邻居样本 $y_j$ 是用 $\hat p_{nbr}$ 选出的，并入当前像素必须换算到当前目标。**DI 的关键**：两个像素的目标定义在**同一个域**（光源表面）——样本不动，只有目标值变：

$$W \mathrel{+}= W_{nbr}\cdot\frac{\hat p_{cur}(y_j)}{\hat p_{nbr}(y_j)},\qquad M\mathrel{+}=M_{nbr},\qquad \text{选中概率}\propto W_{nbr}\cdot\frac{\hat p_{cur}(y_j)}{\hat p_{nbr}(y_j)}$$

**为什么合法**：域映射是恒等映射（样本还是同一个灯点），恒等映射的 Jacobian = 1——**DI 空间复用不需要 Jacobian，原因就在这里**。着色用 $\hat p_{cur}$ 与 $f_{cur}$（$f_{cur}/\hat p_{cur}=V_{cur}$），成对结构在重加权后的流上恢复。

## 3.4 可见性复用的偏差

跳过 $V_{cur}$ 的新阴影射线 → 权重里的 V 与估计量里的 V 失配 → 跨阴影边界**漏光**。跳过 = 有偏省射线；不跳 = 无偏贵成本。门限是平衡旋钮。

## 3.5 无偏性总账（考试必问）

| 配置 | 无偏性 | 偏差来源 |
|---|---|---|
| 仅初始候选 | ✅ 精确无偏 | —— |
| + 时间复用（无 cap） | ✅ 精确无偏 | 合并 = 拼接流 |
| + M 钳制 | ⚠️ 有偏但一致 | 成对结构断裂 ∝ 丢弃份额 |
| + 空间复用（target ratio） | ✅ 期望无偏 | 重加权恢复成对结构 |
| + 可见性复用跳过 | ⚠️ 有偏 | V 失配 → 漏光 |

# 四、ReSTIR GI 的数学

## 4.1 路径空间与重连

DI 样本域 = 光源表面（2 维全域共享）。GI 样本 = 路径 $\bar x=(x_0,\dots,x_k)$，每个像素的路径域**不同**（$x_0$ 不同）——域映射非恒等，DI 的同域重加权失效。

**重连**：借用 $\bar x'=(x_0',x_1',\dots)$，当前像素重建 $\bar x=(x_0,x_1',\dots)$，条件 $V(x_0\to x_1')$ 可见且表面门通过；贡献与目标都在当前像素重评估。**无偏性论证**：可见性失败 = 贡献 0（拒绝，同 RR 终止分支）；通过者按 $\hat p_{cur}$ 合并——条件期望上成对结构恢复。

## 4.2 重连的局限 → 移位映射与 Jacobian（GRIS 预告）

重连是"硬重连"：接不上就丢，红利损失，尾部错配无法解析补偿。**GRIS** 把重连推广为**移位映射** $T:\bar x'\mapsto\bar x$（重连 = 头部换锚 + 尾部恒等的特例）。$T$ 可逆时 Jacobian 补偿：

$$w = \frac{\hat p_{cur}\big(T(\bar x')\big)\,\left|\det\frac{\partial T}{\partial\bar x'}\right|}{p_{src}(\bar x')}$$

**DI 空间复用正是 $T=$ 恒等、Jacobian=1 的特例——DI 与 GRIS 在这条公式下统一**（M4 展开）。

# 五、辩证卡（编号 ⑮–⑳，接 M0 ①–⑨ 与 M1 ⑩–⑭）

> **📌 辩证卡⑮：全局采样 ↔ 点查询+抽签**——RIS 把"全局的采样"降级为"局部的求值"。
> 展开（M1 三轴对齐）：**积分轴**（解析积分做不到 → MC）；**采样轴·简单 p**（CDF 反解是主力且非唯一；Malley 的 `r=√u` 与 `z=√(1-u)` 就是逆变换的几何马甲）；**采样轴·p̂∝f**（它的 CDF 就是算不出的积分本身、反不出来 → RIS 用"求值+概率选中"顶替那个反不出来的 CDF）。一句话：**MC 需要能采样 p；能采样 p 的主力是 CDF 反解；p̂∝f 采不了 = 它的 CDF 反不出来；RIS 用打分顶替了这最后一个反不出来的 CDF。**
> **📌 辩证卡⑯：时间复用 ↔ 空间复用**——时间管"攒"、空间管"铺"，互补覆盖，共同代价 = 错配。
> **📌 辩证卡⑰：复用收益 ↔ 错配代价**——有效性门是平衡点；ReSTIR 工程化的全部艺术。
> **📌 辩证卡⑱：无偏 ↔ 有偏但一致**——M-cap 与 RR 同构，同一笔 Bias²↔Var 交易在两个轴上。
> **📌 辩证卡⑲：DI ↔ GI（同域 ↔ 异域）**——DI 同域重加权（Jacobian=1），GI 重连，GRIS 用移位映射统一两者。
> **📌 辩证卡⑳：质量 ↔ 性能**——$M$/$M_{cap}$/邻居数/门限/可见性重发，每个都是三方权衡旋钮。

# 六、公式速查（验收默写 10 条）

1. $L_o=\int_A\hat p\,V\,\mathrm{d}A$，$\hat p=\dfrac{\rho}{\pi}L_eG$，$G=\dfrac{\cos\theta_x\cos\theta_y}{r^2}$
2. RIS：$F=\dfrac{W}{M}\cdot\dfrac{f(Y^*)}{\hat p(Y^*)}$（W 与 W 约掉）
3. 朴素 SIR 一致性：$W/M\xrightarrow{a.s.}Z$；$\mathbb{P}(Y^*\in A)\to\dfrac{\int_A\hat p}{Z}$
4. $p_{src}=\dfrac{1}{N_L A}$，$w=\dfrac{\rho}{\pi}L_eG\,N_LA$
5. 无 V 版 $F=\dfrac{W}{M}V(Y^*)$；含 V 版 $F=\dfrac{W}{M}$
6. 合并：$W\mathrel{+}=W_2,\ M\mathrel{+}=M_2,\ \mathbb{P}=\dfrac{W_2}{W_1+W_2}$（=拼接流）
7. M-cap：$W\leftarrow W\dfrac{M_{cap}}{M}$（成对结构断裂 → 有偏但一致）
8. 空间复用：$W\mathrel{+}=W_{nbr}\dfrac{\hat p_{cur}(y_j)}{\hat p_{nbr}(y_j)}$（恒等映射，Jacobian=1）
9. GI 重连：$\bar x=(x_0,x_1',\dots)$，条件 $V(x_0\to x_1')$；拒绝=0，接受按 $\hat p_{cur}$ 合并
10. GRIS：$w=\dfrac{\hat p_{cur}(T(\bar x'))\,|\det\partial T/\partial\bar x'|}{p_{src}(\bar x')}$（DI=恒等特例）

## 书目

- Bitterli, Wyman, Pharr, Shirley, Lefohn, Jarosz. *Spatiotemporal reservoir resampling for real-time ray tracing with dynamic direct lighting*. SIGGRAPH 2020.
- Ouyang, Liu, Pharr, et al. *ReSTIR GI: Path Resampling for Real-Time Path Tracing*. HPG 2021.
- Talbot, Cline, Egbert. *Importance Resampling for Global Illumination*. EGSR 2005.
- Lin, Wyman, Yuksel. *Generalized Resampled Importance Sampling*. SIGGRAPH 2022.
