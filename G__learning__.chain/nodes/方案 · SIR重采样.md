---
id: 方案 · SIR重采样
title: 方案 · SIR重采样
parent: 问题：目标分布不可采样时怎么采样
rel: contains
tags:
- 方案
- 采用
- 重采样
- 递进链
revision: 1
updated: 2026-09-08T20:17:33+08:00
---

> 触发：SIR 重采样；采样重要性重采样；重要性重采样

# 方案 · SIR 重采样（Sampling Importance Resampling）

## 解决了什么 / 没解决什么（递进链）

- **解决**：[问题：目标分布不可采样时怎么采样](问题：目标分布不可采样时怎么采样.md)——用两阶段逼近：源分布抽候选 + 按权重重抽，绕过"直接从目标分布采样"。
- **未解决（引出下一环）**：朴素 SIR 有偏（归一化常数未知），且需要两遍扫描存全部候选 → [局限 · 朴素SIR有偏且需两遍](局限 · 朴素SIR有偏且需两遍.md)

## 算法

1. 候选 $Y_1,\dots,Y_M \sim p_{src}$（i.i.d.，源分布可采样）
2. 目标未归一化密度取 $\hat p(y) = f(y)$，权重 $w(y) = \hat p(y)/p_{src}(y)$
3. 以概率 $w(Y_i)/W$ 选中 $Y^*$，其中 $W = \sum_{i=1}^{M} w(Y_i)$

## 无偏 RIS 估计量（必背推导）

用 $W/M$ 估计归一化常数 $Z = \int \hat p$（$\mathbb{E}[W/M] = \mathbb{E}_{p_{src}}[w] = \int p_{src}\cdot(\hat p/p_{src}) = Z$）：

$$F = \frac{W}{M}\cdot\frac{f(Y^*)}{\hat p(Y^*)}$$

无偏性链条：

$$\mathbb{E}[F] = \mathbb{E}_{c}\big[\mathbb{E}_{J|c}[F]\big] = \mathbb{E}_{c}\!\left[\sum_{j=1}^{M}\frac{w(Y_j)}{W}\cdot\frac{W}{M}\cdot\frac{f(Y_j)}{\hat p(Y_j)}\right] = \frac{1}{M}\sum_{j=1}^{M}\mathbb{E}_{c}\!\left[\frac{w(Y_j)\,f(Y_j)}{\hat p(Y_j)}\right]$$

$$= \mathbb{E}_{p_{src}}\!\left[\frac{w\,f}{\hat p}\right] = \int p_{src}\cdot\frac{\hat p}{p_{src}}\cdot\frac{f}{\hat p}\,\mathrm{d}x = \int f = I$$

**关键观察：选中概率里的 $W$ 与补偿因子里的 $W$ 恰好约掉**——权重总和必须参与估计；丢掉 $W/M$（朴素 SIR）就有偏。

## 📌 概念辩证卡⑩：能求值 ↔ 能采样（点查询 ↔ 全局操作）

- **能求值**（管"信息获取"）：给定点 y，O(1) 时间算出 f(y)——渲染里是一条阴影射线。**最便宜的能力**。
- **能采样**（管"样本生成"）：生成密度与目标一致的随机点——**全局操作**，需要目标在整个定义域的形状。分两档：简单 p 采得了（CDF 反解），$\hat p\propto f$ 采不了（它的 CDF 就是算不出的积分本身）。
- **辩证关系**：求值是局部的、采样是全局的——两者分属不同难度。**RIS 的全部存在理由：只借用"求值"，把"全局的采样"降级为"M 次点查询 + 一次加权抽签"**。反例 $e^{x^2}\sin x$：写得出来、求值纳秒、积分无闭式、不能直接按它采样。
- **学习者的判定**：❌ "构造 f̂ 近似 f"——p̂ 就是 f 本身（亮度 ρLeG），从头到尾只求值、不近似；且近似 p̂ 也不破坏无偏性（p̂ 只影响方差）。附带结论：p̂ 去掉 V 是**成本账**（无 V 版 $F=(W/M)\cdot V(Y^*)$ 仍无偏，选中补 1 条阴影射线；含 V 版 $f/\hat p=1$ 但每候选 1 条射线变贵）。
- **来源（对话谱系）**："能求值/能积分/能采样"三能力分离出自 2026-08-28 学习者追问"p̂ 写得出来为什么不能直接积分"；此前"不能积分"源自 M0 阶段（传输项无解析解→蒙特卡洛），"不能采样"源自 M1 动机（p∝f 的 CDF 反不出来）。

## 📌 概念辩证卡⑫：概率选中 ↔ 硬过滤（无偏 ↔ 有偏的分水岭）

- **概率选中**：每个候选以 $w_i/W$ 概率被选中，低贡献样本留在池子里只是"概率闭嘴"。选中概率与补偿因子成对出现（W 与 W 约掉）→ **精确无偏**。
- **硬过滤**：删除低贡献候选（或 max-pick 取最大权重）→ 成对结构断裂 → **有偏**（原项目 max-pick 教训："max-pick biases toward the heavy pdf tail；weighted-random is the unbiased paper behavior"）。
- **辩证关系**：无偏性不是一个连续量——是"按概率选中"与"确定性挑选"的**分水岭**。同族的断裂还有 M-cap（被丢弃权重的候选已参与选中、却不在 W 里）——本卡是理解 ReSTIR 偏差哲学的前置（ReSTIR 篇卡⑰ 同构）。
- **学习者的判定**：❌ "筛掉低贡献样本"——应为"按权重概率选中"；❌ "f/f̂ 比值不为 1 导致不正确"——标准形式 p̂=f 比值恒为 1，即便不等无偏性也不受损。口诀：**抽候选 → 算权重 → 概率选中 → W/M 归一化补偿**。

## 📌 概念辩证卡⑭：MIS ↔ RIS（组合器 ↔ 采样器）

- **MIS（组合器）**：输入是**可采样策略集** $\{p_i\}$（每个都能抽样+能算 pdf）；输出是"策略集内的稳健组合"（Σw=1 无偏、balance 方差最优）。**它不能凭空造出新分布**——当目标分布不属于任何可采样策略时，MIS 无米下锅。
- **RIS（采样器）**：输入是**任意目标 p̂ + 求值能力**（卡⑩）；输出是"目标的近似样本"。它专门解决"没有任何策略能采样目标"的场景——这是它**不可替代**的位置。
- **RIS 不可替代的三点**：①目标域超出策略集（p̂∝ρLeG：多光源按贡献加权、环境贴图、BSDF×光源乘积——CDF 反不出来，MIS 组合不出来）；②多光源选择（"每像素按贡献选光源"本身就是 RIS 目标）；③流式可合并（WRS 三字段支撑时空复用，MIS 没有跨像素/跨帧合并机制——ReSTIR 的复用地基）。
- **辩证关系（同一复杂场景里分层嵌套、互不替代）**：**MIS 管"给定策略集内的最优组合"，RIS 管"策略集之外的目标采样"**——两者正交。内层（顶点级）：MIS 组合 NEE/BSDF（power heuristic）保证单条路径贡献稳健；外层（样本级）：RIS 按路径贡献从候选池重采样保证抽到的路径高贡献。甚至可以互嵌：RIS 的目标里嵌 MIS 权重（参考实现 lum(f)·wMis），MIS 的策略之一也可以是 RIS 输出。**ReSTIR DI 全用 RIS；ReSTIR GI 两者都用（MIS 内嵌 + RIS 外选）——这就是两者在真实复杂场景中的关系。**

## 与 M0 的衔接

M0 的 MIS 组合"两个已知策略"；SIR 解决的是更根本的问题——**当目标分布本身不可采样时怎么办**。两者合起来才是重要性采样的完整版图。

## 附录：狭义 MIS ↔ 广义 MIS 的数学对比（M1 压轴）

**共同骨架**（M0 的定理）：$F = \sum_i w_i(X_i)\dfrac{f(X_i)}{p_i(X_i)}$，无偏 ⟺ $\sum_i w_i = 1$。区别只在 **p_i 定义在哪、怎么换算到公共域**。

**狭义 MIS（Veach 1995/97）**：所有策略在同一测度空间 $(\Omega,\mu)$ 上，密度 $p_1,\dots,p_m$。

$$F = \sum_{i=1}^{m} w_i(X_i)\,\frac{f(X_i)}{p_i(X_i)},\qquad w_i^{bal} = \frac{c_i p_i}{\sum_j c_j p_j}$$

核心定理（Veach Thm 9.2）：$\operatorname{Var}[F_w]-\operatorname{Var}[F_{bal}] = \displaystyle\int_\Omega f^2\sum_i\frac{(w_i-w_i^{bal})^2}{c_i p_i}\,\mathrm{d}\mu \ge 0$。结构特征：权重是**同一点上**的密度份额。

**广义 MIS（Sbert 2018 形式化；GRIS 2022 应用于重采样）**：策略的样本来自**不同测度空间** $(\Omega_i,\mu_i)$——面积 vs 立体角、BDPT 的路径空间 vs 主样本空间（Veach 论文第 10 章已用 Jacobian 处理过）、ReSTIR 邻居蓄水池的**隐式输出分布**。设定：策略 i 在 $\Omega_i$ 以密度 $q_i$ 采样，映射 $T_i:\Omega_i\to\Omega$ 送进公共积分域。若 $T_i$ 可逆，$q_i$ 在公共域诱导的密度：

$$p_i(x) = q_i\big(T_i^{-1}(x)\big)\cdot\Big|\det\tfrac{\partial T_i^{-1}}{\partial x}\Big| = q_i\big(T_i^{-1}(x)\big)\cdot\big|J_i\big|^{-1}$$

然后狭义 MIS 的全部机械照常运转（$p_i$ 换成 Jacobian 修正后的）。**新增的唯一数学实体：Jacobian。** 狭义 MIS = $T_i=id$ 的特例。

**ReSTIR 的位置**：
- DI 空间复用 = 广义 MIS 的 $T=id$、$J=1$ 特例：策略 j = 邻居蓄水池输出分布 $q_j(y)\propto\hat p_j(y)$（隐式，无解析式）；跨域重加权 = $\dfrac{f_{cur}(y)}{q_j(y)}\propto\dfrac{\hat p_{cur}(y)\,V(y)}{\hat p_j(y)}$——**target-ratio 就是这么来的**；归一化常数的比值被 W/M 机制吸收，故**只需精确到常数倍**（隐式分布能用的关键）。
- GI/GRIS = 完整形态：$T$ = 移位映射（$J\neq1$ 进权重）：$w = \dfrac{\hat p_{cur}(T(\bar x'))\,\lvert\det\partial T/\partial\bar x'\rvert}{p_{src}(\bar x')}$

**一张表**：

| | 狭义 MIS | 广义 MIS |
|---|---|---|
| 采样来源 | 同一 $(\Omega,\mu)$，m 个密度 | 各自 $(\Omega_i,\mu_i)$，$q_i$ + 映射 $T_i$ |
| 域换算 | 无 | $p_i = q_i\circ T_i^{-1}\cdot\lvert J_i\rvert^{-1}$ |
| balance | $w_i = c_ip_i/\sum c_jp_j$ | 同形式（$p_i$ 为 Jacobian 修正后） |
| 新数学 | —— | **Jacobian** |
| ReSTIR 落点 | 顶点级 NEE/BSDF（GI 路径内） | DI 空间复用（$T=id,J=1$，隐式策略）；GI 重连/GRIS（$T\neq id,J\neq1$） |
| 结构洞察 | 同点密度份额 | 跨域密度份额 × 域变换密度比 |

**一句话**：狭义 MIS 问"同一个点上哪个策略更可能采到"；广义 MIS 问"异域采到的样本，换算到我的域后价值几何"——多付的代价是一枚 Jacobian。DI 的 target-ratio 是 $J=1$ 的免费情形；GRIS 把免费变收费，换来更广的复用。

## 书目

- Talbot, Cline, Egbert. *Importance Resampling for Global Illumination*. EGSR 2005.
- Bitterli et al. SIGGRAPH 2020（§4，ReSTIR DI 的 RIS 形式化出处）。
- Sbert & Havran 等（广义 MIS 系列，2018+）；Lin et al. *Generalized Resampled Importance Sampling*. SIGGRAPH 2022。
