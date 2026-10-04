---
id: 多重重要性采样MIS
title: 方案 · 多重重要性采样MIS
parent: 局限 · 单一pdf顾此失彼
rel: solves
tags:
- 方案
- 采用
- MIS
- 递进链
evidence:
- .chain/artifacts/多重重要性采样MIS/多重重要性采样MIS_参考实验.py
- .chain/artifacts/多重重要性采样MIS/多重重要性采样MIS_实验结果.txt
revision: 1
updated: 2026-09-08T20:17:33+08:00
---

> 触发：多重重要性采样 MIS；balance heuristic；两个采样策略加权

# 方案 · 多重重要性采样 MIS（Multiple Importance Sampling）

## 解决了什么 / 没解决什么（递进链）

- **解决**：[局限 · 单一pdf顾此失彼](局限 · 单一pdf顾此失彼.md)——两个片面策略（A 怕灯小、B 怕表面糙）按"谁更可能抽到这个样本"加权组合（正—反—合），实验证明 MIS 在两种极端场景都贴着最优策略。
- **未解决（引出下一环）**：直接光的方差压到最优，但每像素仍需要大量样本才收敛——实时 1 SPP 预算下不够 → 跨链接入 [局限 · 每像素样本需求大](局限 · 每像素样本需求大.md)（主线二）

两个片面策略的组合方法（Veach & Guibas 1995）。

## 问题设定

$m$ 个策略 $p_1,\dots,p_m$，样本按比例 $c_i$（$\sum_i c_i = 1$）分给策略 $i$。两种常用形式：

- 多样本版（每策略各采一个 $X_i\sim p_i$）：

$$F = \sum_{i=1}^{m} w_i(X_i)\,\frac{f(X_i)}{p_i(X_i)}$$

- 单样本版（先抽策略 $I\sim \mathrm{Categorical}(c)$，再采 $X\sim p_I$）：

$$F = \frac{w_I(X)}{c_I\,p_I(X)}\,f(X)$$

## 无偏条件（完整推导）

$$\mathbb{E}[F] = \sum_{i=1}^{m} \int_\Omega w_i(x)\,\frac{f(x)}{p_i(x)}\,p_i(x)\,\mathrm{d}x = \int_\Omega f(x)\,\sum_{i=1}^{m} w_i(x)\,\mathrm{d}x$$

$$\mathbb{E}[F] = I \iff \sum_{i=1}^{m} w_i(x) = 1\ \ (\forall x)$$

**关键结论：无偏性与权重具体形式无关，只要权重处处和为 1 就精确无偏。选权重的目的是降方差，不是"保证无偏"。**

## balance heuristic（平衡启发式）

$$w_i^{bal}(x) = \frac{c_i\,p_i(x)}{\sum_{k=1}^{m} c_k\,p_k(x)} = \frac{c_i\,p_i(x)}{\hat{p}(x)}, \qquad \hat{p}(x) := \sum_{k=1}^{m} c_k\,p_k(x)$$

代入单样本版：

$$F_{bal} = \frac{w_I(X)}{c_I\,p_I(X)}\,f(X) = \frac{f(X)}{\hat{p}(X)}$$

**与选中哪个策略无关！balance 估计量 ≡ 用混合分布 $\hat p$ 做重要性采样**（这条代换本身就是无偏性证明）。

## power heuristic（幂启发式）

$$w_i^{pow}(x) = \frac{\big(c_i\,p_i(x)\big)^\beta}{\sum_{k=1}^{m}\big(c_k\,p_k(x)\big)^\beta}, \qquad \beta=1\ \text{即 balance},\ \ \beta=2\ \text{常用}$$

仍然 $\sum_i w_i = 1$ ⟹ 无偏。$\beta=2$ 让"pdf 占优的策略"权重更极端，在单个策略主导样本的场景方差更低。

> 代码对照：`powerHeuristic(pdfJ, pdfK) = pdfJ²/(pdfJ²+pdfK²)` 正是 $\beta=2$、$c_A=c_B$ 时的 $w_i^{pow}$。

## Veach 定理 9.2（balance 的方差最优性）

对任意满足 $\sum_i w_i = 1$ 的权重族 $w$：

$$\operatorname{Var}[F_w] - \operatorname{Var}[F_{bal}] = \int_\Omega f(x)^2 \sum_{i=1}^{m} \frac{\big(w_i(x) - w_i^{bal}(x)\big)^2}{c_i\,p_i(x)}\,\mathrm{d}x \ \ge\ 0$$

**balance heuristic 的方差 ≤ 任何其他组合权重**。直观：谁在这个样本点处 pdf 大，谁的权重就大——"谁的样本更可能落在这，谁说了算"。

> **📌 概念辩证卡⑤：约束 ↔ 自由（MIS 里的无偏与优化）**
> - **约束（无偏性）**：$\sum_i w_i = 1$ —— 一个等式的约束，保证"对不对"。
> - **自由（降方差）**：约束之内权重函数仍有无限多种选择——balance/power 就是在这个自由空间里挑方差最小的。
> - **辩证关系**：无偏性是**底线**（不可突破的约束），方差优化是**底线之上的腾挪**（自由空间内的选择）。先立约束、再求最优——权重选择的全部意义都在"约束内的优化"，而不是"实现无偏"。

## 直接光照中的完整 MIS 公式（与代码逐行对照）

两个策略各采一个样本，$c_A = c_B = 1/2$，混合分布 $\hat p = \tfrac12 p_A + \tfrac12 p_B$：

$$F_{MIS} = \frac{1}{2}\cdot\frac{f(X_A)}{\hat p(X_A)} + \frac{1}{2}\cdot\frac{f(X_B)}{\hat p(X_B)}$$

**策略 A 撞灯时**（$f=(\rho/\pi)L_e\cos\theta$，$p_A=\cos\theta/\pi$）——注意化简：

$$\frac{f}{p_A} = \frac{(\rho/\pi)\,L_e\cos\theta}{\cos\theta/\pi} = \rho\,L_e \qquad\text{（撞灯贡献不含 pdf 的原因）}$$

> 代码对照：`weight = powerHeuristic(lastBsdfPdf, lightPdf); L += weight * beta * emission;`

**策略 B（NEE）**（$f=(\rho/\pi)\,L_e\cos\theta\,V$，$p_B = r^2/(A\cos\theta')$）：

$$F_B = \beta\cdot\frac{\rho}{\pi}\,L_e\,\cos\theta\,V\cdot\frac{A\cos\theta'}{r^2}\cdot w_B$$

> 代码对照：`L += beta * (bc/PT_PI) * Le * ndotl * weight / lightPdf;`（$w_B$ = `powerHeuristic(lightPdf, bsdfPdf)`）

## 实验数据（evidence 实测，N=10000×100 次重复）

| 场景 | 策略A std | 策略B std | MIS std | 方差比 A:MIS | 方差比 B:MIS |
|---|---|---|---|---|---|
| 大光源 1×1 | 0.0343 | 0.000578 | 0.000901 | 1451 | 0.41 |
| 小光源 0.1×0.1 | 0.00423 | 0.000001 | 0.000002 | $6.9\times10^6$ | 0.45 |

三策略偏差均 $\approx 0$（无偏性数值验证）✓；**MIS 在两种极端场景都贴着最优策略**。参考实现见 evidence 列表（Python/numpy，可直接运行复现）。

> 相关概念：[直接光照的两个采样策略](直接光照的两个采样策略.md)。
