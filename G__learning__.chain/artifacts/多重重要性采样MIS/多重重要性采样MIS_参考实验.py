# t-001 M0 参考实现：直接光照三策略对比（BSDF 采样 / 光源采样 / MIS balance heuristic）
# 运行：python .chain/artifacts/t-001/t-001_mis_experiment.py
# 场景：着色点 p=(0,0,0)，法线 n=(0,0,1)，朗伯面 rho=0.5；
#       水平矩形面光源（法线朝 -z），自发光 Le=100。
# 建议：完成练习 3 之前不要看本文件。
import numpy as np

RHO = 0.5      # 朗伯 albedo
LE = 100.0     # 光源辐射度

LIGHT_BIG = (2.0, 3.0, 2.0, 3.0, 2.0)     # 1x1 大光源 @ z=2
LIGHT_SMALL = (2.0, 2.1, 2.0, 2.1, 2.0)   # 0.1x0.1 小光源 @ z=2


def ref_integral(light, n=400):
    """参考值：对光源面积做高精度数值积分
    Lo = rho/pi * Le * ∫_A cosθ·cosθ'/r² dA，水平光源 cosθ=cosθ'=z/r
    """
    x0, x1, y0, y1, z = light
    xs = np.linspace(x0, x1, n)
    ys = np.linspace(y0, y1, n)
    X, Y = np.meshgrid(xs, ys)
    r2 = X * X + Y * Y + z * z
    cost = z / np.sqrt(r2)
    dA = ((x1 - x0) / n) * ((y1 - y0) / n)
    return float(RHO / np.pi * LE * np.sum(cost * cost / r2) * dA)


def sample_cosine(n, rng):
    """策略 A：余弦加权半球采样（Malley 法）
    单位圆盘均匀采样 (r,φ) 后正交投影到半球：ω=(r cosφ, r sinφ, sqrt(1-r²))，pdf=cosθ/π
    """
    u1 = rng.random(n)
    u2 = rng.random(n)
    r = np.sqrt(u1)                # 面积均匀 → r=sqrt(u)
    phi = 2.0 * np.pi * u2
    x = r * np.cos(phi)
    y = r * np.sin(phi)
    z = np.sqrt(1.0 - u1)
    return np.stack([x, y, z], axis=1)


def eval_f(omega, light):
    """被积函数 f(ω)=ρ/π·Le·cosθ·V(ω)（含可见性：方向打不到光源则为 0）"""
    x0, x1, y0, y1, zl = light
    z = omega[:, 2]
    f = np.zeros_like(z)
    m = z > 1e-6
    t = zl / z[m]                                   # 沿方向到光源平面的距离
    hit = ((omega[m, 0] * t >= x0) & (omega[m, 0] * t <= x1) &
           (omega[m, 1] * t >= y0) & (omega[m, 1] * t <= y1))
    f[m] = np.where(hit, RHO / np.pi * LE * z[m], 0.0)
    return f


def pdfB_from_dir(omega, light):
    """策略 B 在立体角下的 pdf：pB(ω)=1/A·r²/cosθ'（水平光源 cosθ'=z/r）
    沿方向能打到光源时 r=t，故 pB=t³/(A·z)；打不到则 0。
    """
    x0, x1, y0, y1, zl = light
    A = (x1 - x0) * (y1 - y0)
    z = omega[:, 2]
    pdf = np.zeros_like(z)
    m = z > 1e-6
    t = zl / z[m]
    hit = ((omega[m, 0] * t >= x0) & (omega[m, 0] * t <= x1) &
           (omega[m, 1] * t >= y0) & (omega[m, 1] * t <= y1))
    pdf[m] = np.where(hit, t ** 3 / (A * zl), 0.0)
    return pdf


def strategy_A(light, n, rng):
    """策略 A（BSDF 采样）：f/pA，pA=cosθ/π → 对朗伯面化简为 ρ·Le·V"""
    om = sample_cosine(n, rng)
    f = eval_f(om, light)
    pA = om[:, 2] / np.pi
    return float(np.mean(f / pA))


def strategy_B(light, n, rng):
    """策略 B（光源面积采样）：f/pB"""
    x0, x1, y0, y1, zl = light
    pts = np.stack([rng.uniform(x0, x1, n),
                    rng.uniform(y0, y1, n),
                    np.full(n, zl)], axis=1)
    r = np.sqrt(np.sum(pts * pts, axis=1))
    cost = zl / r
    f = RHO / np.pi * LE * cost              # V=1（样本就在光源上）
    A = (x1 - x0) * (y1 - y0)
    pdfB = r ** 3 / (A * zl)
    return float(np.mean(f / pdfB))


def strategy_MIS(light, n, rng):
    """策略 C（MIS balance，cA=cB=0.5）：贡献 f(ω)/p̂(ω)，p̂=cA·pA+cB·pB
    总估计 = cA·mean_A(contrib) + cB·mean_B(contrib)
    """
    na = n // 2
    nb = n - na
    cA, cB = 0.5, 0.5
    # A 半：余弦半球采样
    om = sample_cosine(na, rng)
    fA = eval_f(om, light)
    pA = om[:, 2] / np.pi
    pB_A = pdfB_from_dir(om, light)
    contribA = fA / (cA * pA + cB * pB_A)
    # B 半：光源面积采样
    x0, x1, y0, y1, zl = light
    pts = np.stack([rng.uniform(x0, x1, nb),
                    rng.uniform(y0, y1, nb),
                    np.full(nb, zl)], axis=1)
    r = np.sqrt(np.sum(pts * pts, axis=1))
    omB = pts / r[:, None]
    fB = RHO / np.pi * LE * omB[:, 2]
    pA_B = omB[:, 2] / np.pi
    A = (x1 - x0) * (y1 - y0)
    pB_B = r ** 3 / (A * zl)
    contribB = fB / (cA * pA_B + cB * pB_B)
    return float(cA * np.mean(contribA) + cB * np.mean(contribB))


def run(light, n=10000, reps=100, seed=0):
    ref = ref_integral(light)
    res = {"A": [], "B": [], "M": []}
    for k in range(reps):
        res["A"].append(strategy_A(light, n, np.random.default_rng(seed * 1000 + k)))
        res["B"].append(strategy_B(light, n, np.random.default_rng(seed * 1000 + k + 1)))
        res["M"].append(strategy_MIS(light, n, np.random.default_rng(seed * 1000 + k + 2)))
    out = {"ref": ref}
    for key, v in res.items():
        v = np.asarray(v)
        out[key] = {
            "mean": float(np.mean(v)),
            "std": float(np.std(v, ddof=1)),
            "bias": float(np.mean(v) - ref),
        }
    return out


if __name__ == "__main__":
    for name, light in [("大光源 1x1 @ z=2", LIGHT_BIG),
                        ("小光源 0.1x0.1 @ z=2", LIGHT_SMALL)]:
        r = run(light)
        print(f"=== {name} ===")
        print(f"参考值 (数值积分): {r['ref']:.6f}")
        print(f"{'策略':<34}{'均值':>12}{'标准差':>12}{'偏差':>12}")
        for key, label in [("A", "A BSDF采样(cos半球)"),
                           ("B", "B 光源面积采样"),
                           ("M", "M MIS balance 0.5/0.5")]:
            d = r[key]
            print(f"{label:<34}{d['mean']:>12.6f}{d['std']:>12.6f}{d['bias']:>12.6f}")
        print(f"方差比 A:M = {(r['A']['std']/r['M']['std'])**2:.2f}  "
              f"方差比 B:M = {(r['B']['std']/r['M']['std'])**2:.2f}")
        print()
