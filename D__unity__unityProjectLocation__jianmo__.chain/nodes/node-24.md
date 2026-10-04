---
id: node-24
type: note
title: 方法 · 游戏提取模型的动画化改造全流程（IK/脚掌锁定/头发动力学/编码）
parent: node-1
status: none
created: 2026-09-19T21:25:58+08:00
updated: 2026-09-19T21:26:06+08:00
revision: 2
tags:
- 动画
- IK
- 动力学
- Blender
- 踩坑
rel: contains
rel_desc: 从人物模型到可动画化的完整改造流程与踩坑
---

> 触发：给游戏提取模型做动画；IK 搭建；脚掌锁定；头发动力学；弹簧骨烘焙；mmd 骨骼动画

**类型：方法（可复用流程）+ 踩坑清单**

把 `诗歌剧.1`（UmaViewer 提取的游戏模型）改造成可用于 Blender 原生动画的完整流程。产物 `models/shikageki_anim.blend`。

## 一、腿部 IK（源文件没有 IK，必须手建）

**约束挂在小腿骨上，不是脚骨。**
```
Knee_L/R  ← IK 约束, chain_count=2, use_tail=True, use_stretch=False
             target = 空物体 IK_Ankle_L/R（放在踝关节位置）
             pole_target = 空物体 IK_Pole_L/R（放在膝盖前方 -Y）
```

**为什么是 `Knee_*` 而不是 `Ankle_*`**：这套游戏骨架的 `Ankle_L` 骨头是**朝上的**（head z=−0.2149 → tail z=−0.1349）。IK 目标的到达点是**受约束骨骼的 tail**，所以必须挂在 tail 正好落在踝关节的那根骨上——也就是小腿。挂在 `Ankle_L` 上会让脚反向求解。

**pole_angle 必须实测扫描，不能猜。** 静止姿势膝盖几乎是直的（预弯仅约 4°），直接扫描判别不出来。做法：先把 IK 目标临时挪到一个**强制弯曲**的位置，再从 −180° 到 180° 每 5° 扫一遍，取膝盖最靠前（y 最小）的角度。
实测两侧都收敛到 **−90°**，且曲线有明确的唯一极小值。

**验证判据**：把胯部下压 0.06，两只脚的踝关节世界坐标应当**一个数字都不变**。实测 `(0.0559, 0.0063, −0.2149)` 完全不变，膝盖前弯到 y=−0.066。

## 二、脚掌水平锁定（必需，不是可选项）

胯部下沉 0.012 时，膝盖会弯 **37°**（腿长 0.189，用余弦定理算得），脚会跟着翘起来 37°。

解法：给 `Ankle_L/R` 加**世界空间**的 `COPY_ROTATION` 约束，target 是一个空物体。
**关键细节**：空物体的朝向必须**等于该骨骼的静止朝向**，否则第一帧脚就会翻 90°。
`Ankle_L` 的静止轴是 X=(1,0,0) Y=(0,0,1) Z=(0,−1,0)，等价于**绕世界 X 转 +90°**，所以空物体 `rotation_euler = (radians(90), 0, 0)`。
之后动画这个空物体即可做脚踝的抬起/压下。

## 三、头发动力学：位置弹簧 + 惯性烘焙

源文件 0 刚体，MMD 物理不存在，只能自己写。头发占模型 58% 高度，不做会极假。

**核心是把「反解骨骼局部旋转」写成解析式，不要用 `pose_bone.matrix` 赋值**（那需要每根骨每帧刷新 depsgraph，47 骨 × 120 帧根本跑不动）。

```
L_k   = parent_rest_matrix_local⁻¹ @ bone_rest_matrix_local   （常量，预计算）
W_rigid = W_parent @ L_k            ← 零局部旋转时的世界矩阵
base    = W_rigid.translation       （骨头位置）
rigid_tip = W_rigid @ (0, bone_length, 0)

# 弹簧 + 惯性（在世界空间模拟发梢位置）
v   = (v + (rigid_tip − tip) * STIFF + GRAVITY) * DAMP
dir = normalize(tip + v − base)
# 限角：把 dir 朝 rigid_dir 旋转回去，最多 MAXANG
tip = base + dir * bone_length

# 反解
R    = rigid_dir.rotation_difference(dir).to_matrix().to_4x4()
W_k  = R @ W_rigid;  W_k.translation = base
basis = L_k⁻¹ @ W_parent⁻¹ @ W_k
pose_bone.rotation_quaternion = basis.to_quaternion()
```

**两遍法**：第一遍只记录每帧父骨的世界矩阵（每帧一次 depsgraph 更新），第二遍纯数学模拟并插帧。

实测参数：`DAMP=0.72, STIFF=0.30, MAXANG=55°, GRAVITY=(0,0,−0.0012)/帧²`。
24 条链、5640 个关键帧；发梢摆幅约 0.127（1.6 倍骨长），首尾帧精确回到原位可循环。

**链识别**：按名字含 Hair/Ribbon/Tail/Ear 筛选，排除 `_Handle`/`_Ha`/`_Ha2` 空壳骨；沿「只有一个动态子节点」向下延伸；**分叉支路（马耳有两条并行链）要单独成链**，否则会被漏掉。

## 四、可见性测试：相机射线，但别从物体内部打

判断某个部位在镜头里能不能看见，用 `evaluated_object.ray_cast` 从**相机**向目标点打射线，看第一击落点是否落在目标附近。

**踩的坑**：一开始从目标点向相机打射线，结果**第一下就打在目标自身表面**，永远判定为遮挡——144 个姿势全部"不可见"。
**必须加对照组**：拿一个已知可见的点（脸部）测一次，第一击距离应该比到目标的距离小一点点（实测 1.796 vs 1.805）。对照组不过就说明测试本身错了。

## 五、这个模型做动画的硬约束（实测）

- **胳膊伸不出头发轮廓**：手臂全长仅 0.172，而头发半宽 0.37。216 种手臂姿势里只有 **9 种**能让手露出来（`elbow` 和 `wrist` 同时被判可见的只有更少）。且**全部是左臂**（相机侧），右臂在远侧一次都不行。
  → 可行的挥手区：**左臂抬 20°、前摆 50°、肘弯 30°**，手到胸口高度前方。
- 结论：这种体型**头部与头发才是视觉主体**，动画重心应放在头/头发/尾巴的次级运动上，而不是手臂。

## 六、工具链坑

- **Blender 5.2.2（Steam）没有编译 FFMPEG**：`image_settings.file_format` 的枚举里没有任何视频格式，VSE 也编码不了。
  → 用 **Bilibili 客户端自带的 ffmpeg**：`C:\Users\jcm20\AppData\Roaming\bilibili\ffmpeg\ffmpeg.exe`（3.0.1，含 libx264）。
  命令：`ffmpeg -framerate 24 -i f_%04d.png -c:v libx264 -preset slow -crf 18 -pix_fmt yuv420p -movflags +faststart out.mp4`
- **`Material.blend_method` 在 Blender 4.2+ 已被 `surface_render_method` 取代**，赋值 `'OPAQUE'` 不报错但读回来恒为 `'HASHED'`。想"把不透明材质改 OPAQUE 省性能"是无效的。
- **MCP 单次调用有超时**：EEVEE 单帧 3.4 秒 × 120 帧 = 405 秒，必须**分段渲染**（每段 ≤45 帧约 155 秒）。
- **本地 Qwen2.5-VL-3B 对数值和姿势的判断不可靠**：它说"头部占 30%、腿占 40%"（几何实测是 48%/24%），说"没有一条手臂抬起"（几何实测手腕 z 从 0.040 抬到 0.321）。**凡是能用几何量出来的，就不要问视觉模型。**

