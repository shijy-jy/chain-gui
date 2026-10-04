---
id: node-28
type: note
title: 实战 · 为 Unity 工程制作诗歌剧移动动画（Blender 出片，Unity 使用）
parent: node-1
status: none
created: 2026-09-22T18:45:10+08:00
updated: 2026-09-22T20:26:46+08:00
revision: 4
tags:
- Blender动画
- 原地循环
- 两骨IK
- 脚不打滑
- Unity接入
- 尾巴穿地
- 姿态缓存
rel: contains
rel_desc: 用自建工具链在真实 Unity 工程上做角色移动动画：解析式 IK + 脚不打滑 + 往返校验
---

> 触发：Unity 角色动画不好；脚打滑；原地循环动画；Idle/Walk/Run 重做；把模型复制到 Blender 做动画；步态解算；FBX 多 take

## 背景

Unity 工程 `D:\unity\unityProjectLocation\TA_show` 用 `Player/Visual_Uma` 显示诗歌剧
（`Assets/CharacterLab/Uma/UmaMatikanetannhauser.fbx`，Blender 5.2.2 由 `pmx_to_fbx.py` 从 PMX 转出）。
原来那套移动动画是 `Assets/Editor/UmaAnimationBuilder.cs` **程序化生成的纯 FK 相位表**，
用户反馈「做得不好」。

**根因**：纯 FK 相位表没有接地解算 —— 支撑脚在站立相里不会停在原地，必然滑步；
骨盆高低靠手调正弦，腿伸不到时脚就悬空/穿地。

## 做法（解析式 IK，不依赖 Blender 约束）

脚本：`blender-mcp\uma_locomotion.py`（可在 Blender 里 `exec` 或后台 `--python` 跑）
源工程：`models\uma_unity.blend` → 产出 `models\uma_locomotion.blend`

关键设计（每一条都是踩过坑才定下来的）：

1. **原地跑步机模型**：一个步态周期身体前进 `stride`，所以支撑相里脚相对根要向后走
   **`stride × duty`**，不是整个 stride。搞错这一条会让支撑脚速度直接差 duty 倍。
2. **解析式两骨 IK**：自己解 `(a_len, b_len)` 的三角形，不建 Blender IK 约束、不用 pole target。
3. **关节用父骨局部偏移做精确前向推算**：`set_dir` 只定方向，关节实际位置由 rest 偏移
   旋转得到，和三角解算有亚毫米残差 → 用 `leg_fk` 推真实位置并迭代 4~6 次修正。
4. **骨盆高度由支撑腿可达性反推**：先按艺术意图摆骨盆，再对每根支撑腿算
   `zmax = ankle_z + sqrt(free² − dx² − dy²)`，取 min 往下压。起伏是算出来的，不是手调的。
5. **脚掌滚动 + 鞋底最低点贴地**：踝高 = `SOLE_Z − min(旋转后鞋底局部点的 z)`，
   实现脚跟着地 → 全掌 → 蹬地，而不是整只脚僵平。
6. **必须按真实蒙皮权重算鞋底**：只取 `Ankle` 权重大于 0.5 的顶点会漏掉鞋尖
   （鞋尖有相当权重落在 Knee 上）→ 蹬地时鞋尖穿地。改用全权重 LBS，
   并对着「求值后的真实变形网格」迭代踝高。
7. **姿态缓存必须向下传递**：自己维护的骨骼世界矩阵缓存里，**没被显式摆过的骨头**
   （如 `Tail_Ctrl`）必须按父级姿态递归推算，否则尾巴这类挂在中间骨下的链条
   会按静止姿态摆放，最后几节骨骼姿态全错。这一条同时修掉了「尾巴尖穿地」。
8. **尾巴离地保护**：尾巴静止时就垂到 z=-0.11，跑步时会长到地下 17~24mm；
   在次级运动之后整体往上抬（0→42°，按真蒙皮最低点收敛）。
9. **导出必须 `add_leaf_bones=True`**：设为 False 会丢掉 48 根链尾骨
   （裙摆 `_01`、头发 `_03`、`Ear_03` 等），次级运动最末端就没了。开 True 后
   178 根骨头一根不多一根不少地往返。

## 验证（全部确定性、不看图）

* 穿地：逐帧对 `evaluated_get(depsgraph).to_mesh()` 的 **14880 个顶点**求最低 z
  → 三段动画都是 `-0.25570`（正好等于地面），穿地量 `0.00000`
* 打滑：支撑相脚相对根的水平速度 vs 隐含地面速度 → 误差 `0.00000`
* 循环：首帧 vs 末帧全骨世界矩阵最大差 → `0.000000`
* 往返：重新导入导出后的 FBX，骨头 178 根**零缺失零多余**，网格 bbox 完全一致

## 参数与结果

| Take | 帧数@30fps | 隐含地面速度（模型单位/秒） |
|---|---|---|
| Uma_Idle | 0–120 (4.0s) | 0 |
| Uma_Walk | 0–20 (0.667s) | 0.300 |
| Uma_Run | 0–12 (0.4s) | 0.750 |

模型身高 1.1841 模型单位、髋到踝只有 0.189 → **这是个严重 Q 版的角色，腿极短**。
所以它「天然」走路速度就是 0.39 m/s 上下（Unity 世界，按 Visual_Uma 缩放 1.2969 折算），
跑 0.97 m/s。Unity 里要脚不打滑，`moveSpeed = 隐含速度 × S`，
`S = 角色在 Unity 里的世界身高 ÷ 1.1841`。

## 产物

* `TA_show\Assets\CharacterLab\Uma\UmaMatikanetannhauser_Locomotion.fbx`（3 个 take）
* `TA_show\Assets\CharacterLab\Uma\README_BlenderLocomotion.md`（Unity 接入说明）
* `jianmo\models\uma_unity.blend`（Unity 那份 FBX 原样导入的副本）
* `jianmo\models\uma_locomotion.blend`（带三段动画 + NLA）
* `blender-mcp\verify_fbx.py`（往返校验）、`blender-mcp\make_gait_sheet.py`（步态对比图）
* `blender-mcp\output\sheet_{walk,run}_{side,front}.png`、`sheet_idle_side.png`

## 没做 / 待办

* 没有把 clip 塞进 `UmaAnimatorController`（需要 Unity 侧操作或 Codely 桥）
* 没有做转弯/急停/跳跃；只有 Idle / Walk / Run 三态
* 该角色是 Q 版短腿，若要「人形比例」的步态，只能换模型，不是调参能解决的

---

## 追加：按真人动作捕捉重新标定步态（第二轮）

### 参考数据来源

* **CMU Graphics Lab Motion Capture Database**（`mocap.cs.cmu.edu`，用 BVH 版镜像
  `github.com/una-dinosauria/cmu-mocap`）—— 走路 Subject 08（`008_08_01.bvh`）、
  跑步 Subject 09（`009_09_01.bvh` / `09_06`）。**真人动捕**，不是手 K。
* **Mahlazer Walking Motion**（`learnmmd.com/Mahlazer_Walking_Motion_951Frames.zip`，
  免费配布）—— MMD 原地走路，ReadMe 明确说周期是第 15~54 帧，交叉参考用。
* **模之屋 / aplaybox**：检索接口摸通了（`api.aplaybox.com/api/web/v1/work/search`，
  `{"type":2,"keyword":..,"current_page":..,"per_page":..}`；
  详情 `work/getWorkDetails` 需要 `{work_uuid,work_type_id,user_uid,is_login}`），
  但**下载需要登录**（`downloadWorkNewV2` → `Unauthenticated.`），
  且 `.pbv` 是加密文件（多个样本前 20 字节完全相同、熵 7.37 bits/byte，不是裸 VMD）。
  工具留在 `blender-mcp\aplaybox.py`，登录态可用（Cookie `access_token` → `Authorization: Bearer`）。

### 新工具

* `blender-mcp\bvh_gait.py` —— BVH 解析 + 前向运动学 + 步态量化
  （脚相对根的前后/垂直曲线、骨盆起伏摆动、偏航、前倾、摆臂、占空比）
* `blender-mcp\ref_gait.py` —— VMD 单周期曲线抽取
* `blender-mcp\vmd_analyze.py` / `vmd_gait.py` —— VMD 结构与步态分析

### 标定结果（真人 vs 本动画）

| 参数 | CMU 真人 | 本动画 |
|---|---|---|
| 走 · 单脚前后行程 | 0.877 腿长 | 0.869 |
| 走 · 抬脚高度 | 0.180 腿长 | 0.186 |
| 走 · 周期 | 0.917 s | 0.933 s |
| 跑 · 单脚前后行程 | 0.854 腿长 | 0.852 |
| 跑 · 抬脚高度 | 0.456/0.379 腿长 | 0.423 |
| 跑 · 前倾 | +10.3° | +10.0° |
| 跑 · 腾空相 | 有 | 19% 帧数 |

### 对照真人数据时揪出的三个真 bug（都是「看着像在动、其实没生效或反了」）

1. **骨盆左右摆方向反了**：原来左脚支撑时骨盆往右倒（`sway = -A·sin`），
   和真人数据正好相反 → 改成 `+A·sin`。
2. **骨盆偏航用错轴**：写成绕世界 Y（那是**侧倾**），角色 up 是 +Z，
   偏航必须绕 Z。同时胸腔反向扭转量从 0.85 倍提到 1.15 倍（真人实测 1.1~1.3 倍）。
3. **摆臂几乎没生效**（两处叠加）：
   ① T-pose 的手臂沿 ±X 轴，`swing_rot(x=sw, y=down)` 先绕 X 转等于没转；
   必须 `rx(sw) @ ry(down)` —— **先放下手臂，再前后摆**。
   ② `clip.arm_swing` 已经是弧度，却又乘了一次 `DEG`，实际摆幅只有 0.47°。
   修好后：手腕前后摆幅 **0.0003 → 0.156 模型单位**。

### 第二轮参数

| Take | 帧数@30fps | 时长 | 隐含地面速度（模型单位/秒） |
|---|---|---|---|
| Uma_Idle | 0–120 | 4.000 s | 0 |
| Uma_Walk | 0–28 | 0.933 s | **0.2865** |
| Uma_Run | 0–20 | 0.667 s | **0.6372** |

Unity 世界速度（按 `Visual_Uma` 缩放 1.2969 折算）：走 **0.37 m/s**、跑 **0.83 m/s**。

### 教训

* 「动画看起来在动」不等于参数生效了 —— 一定要**量出来**（本轮的摆臂 0.47°、
  骨盆摆动反向，都是量了才发现的）。
* 自建姿态系统里，**旋转的施加顺序**（先放下再摆动）和**单位**（度/弧度）
  是最容易静默出错的两处。
* 对比真人数据的最大价值不是「抄曲线」，而是**发现自己的符号/轴/顺序错误**。

---

## 续：游戏原版动画已提取并重定向完成（2026-09-22）

之前 node-29 里卡住的 streamed 段已解通（坑是 `m_StreamedClip.data` 是 uint32 位模式数组
而不是 float 数组），游戏原版三条动画已完整提出并重定向到我们修好的 Q 版骨架上。

**产物**：`TA_show\Assets\CharacterLab\Uma\UmaMatikanetannhauser_GameLocomotion.fbx`（5.50 MB）

| take | 帧数 | 来源 |
|---|---|---|
| `Uma_Walk_Game` | 40 | `anm_eve_type00_walk01_loop` |
| `Uma_Run_Game` | 28 | `anm_eve_type00_run01_loop` |
| `Uma_Turn_Game` | 43 | `anm_eve_type00_turn01` |

**验证全绿**：穿地 0.00000、走路/转身悬空 0.00000、跑步 10 帧腾空（弹道弧线保住了）、
loop 闭合 0.000000、FBX 往返 178 骨骼无缺失、网格 bbox 完全一致、
手腕侧向从 T-pose ±0.36 落到 0.10~0.15（手臂确实垂下来了）、膝盖恒朝前弯、
跑步头部相对髋恒前倾 15~37mm（走路 ±6mm）——**之前"跑没有前倾"的问题这次是从游戏原版动画来的**。

方法记在 node-30。步态对比图：`blender-mcp\output\sheet_game_{walk,run,turn}_{side,front}.png`。
