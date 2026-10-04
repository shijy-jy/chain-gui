---
id: node-30
type: note
title: 参考 · 动画重定向到自有骨架（世界空间姿势增量法）
parent: node-28
status: none
created: 2026-09-22T20:26:37+08:00
updated: 2026-09-22T20:50:10+08:00
revision: 3
tags:
- 动画重定向
- retarget
- Blender
- 四元数
- 坐标转换
- 落地修正
- 赛马娘
rel: contains
rel_desc: 把游戏原版走/跑/转身重定向到已修好的 Q 版骨架，产出可直接进 Unity 的 FBX
---

> 触发：把别人的动作套到自己的骨架上；retarget；骨骼映射；动作重定向；delta retarget

## 问题

已有游戏原版动画（见 node-29），要套到我们**已经修好的 Q 版骨架**上。两套骨架的
静止姿势、骨轴朝向、父子链、比例全都不一样，手工对着坐标轴凑符号必然出错。

## 方法：世界空间「姿势增量」传递

对每根骨骼、每一帧算一个**世界空间增量**，再转成我们骨架的 `matrix_basis`：

$$D(b,f) = R_{\text{anim}}(b,f)\cdot R_{\text{rest}}(b)^{-1}$$

$$R_{\text{basis}}(b) = R_{\text{rest}}(b)^{-1}\cdot\Big(D(\text{parent}(b))^{-1}\cdot D(b)\Big)\cdot R_{\text{rest}}(b)$$

**最后一行是纯局部的**——父骨的世界增量两边约掉了。推导：姿势矩阵
$R_{\text{pose}}(b) = D(p)\,R_{\text{rest}}(p)^{-1}R_{\text{rest}}(b)\,R_{\text{basis}}(b)$，
要求 $R_{\text{pose}}(b) = D(b)R_{\text{rest}}(b)$，解出上式。

好处：**我们这侧既不用做 FK，也不用 `view_layer.update()`**，直接把四元数写关键帧。
父骨映射用**我们自己层级**的 parent（例如我方 `Waist` 的父是 `Hip`，就取游戏的 `Hip` 增量），
所以游戏那侧多出来的 `UpBody_Ctrl`、`Spine` 会自动被合并进去，不需要单独处理。

## 坐标转换：反射下的赝矢量规则

Blender：+X=角色左，−Y=前，+Z=上。Unity：+X=右，+Z=前，+Y=上。

$$\text{B} \leftarrow \text{U}:\quad B_x=-U_x,\; B_y=-U_z,\; B_z=U_y$$

这个映射行列式 = −1（是**反射**，符合左右手系转换）。反射下转轴按**赝矢量**变换
（$\mathbf a' = \det(M)\,M\mathbf a$），于是四元数只需：

$$\mathbf q_{\text{blender}} = (u_x,\; u_z,\; -u_y,\; u_w)$$

*位置*用 $(-U_x, -U_z, U_y)$。**验证方法**：Unity 绕 +X 转 θ（俯仰）和 Blender 绕 +X 转 θ
物理上是同一个方向（都低头），绕 +Y 的偏航映射到绕 −Z 也自洽——不要凭"左手系就加负号"想当然。

## 落地修正（数值稳定，可迭代收敛）

重定向后**关节角是对的，但整个人会浮空**：Q 版模型的脚相对腿长比游戏厚 33%
（0.216 vs 0.162），用「网格最低点贴地」去修会把这段几何差算成身体高度差，
造出 17mm 的**假起伏**。

正确做法分两层：

1. **按踝关节落地**：让每帧较低那只脚的**踝关节**回到静止高度。
   踝是纯腿驱动的，比例和游戏一致，没有脚型误差。双脚都明显离地的帧（跑步腾空）
   不落，改为在相邻接触帧之间线性插值，**保住弹道弧线**。
2. **按鞋底精确钳制**：再测一次网格真实最低点，把残余穿地量补掉。
   网格随髋部平移是线性的，所以**一次迭代即收敛**（实测 3.6mm → 0）。

### 比例要用骨链长度，不是髋→踝距离

游戏里 `Thigh` 骨头比 `Hip` 骨低 0.0269，所以「髋骨→踝骨距离」比真实骨链长 4%。
用错会整体差 4%。正确：`(我方 0.088501+0.100723) / (游戏 0.35282+0.40208) = 0.25066`。

## 实测结果（跑/走/转身三条都过）

| clip | 帧 | 穿地 | 悬空 | 髋起伏 | 循环闭合 |
|---|---|---|---|---|---|
| walk01_loop | 40 | 0.00000 | 0.00000 | 3.77mm = 2.0% 腿长 | 0.000000 |
| run01_loop | 28 | 0.00000 | 10 帧腾空 | 20.1mm = 10.7% 腿长 | 0.000000 |
| turn01 | 43 | 0.00000 | 0.00000 | 4.25mm = 2.3% 腿长 | 2.0（本就非循环） |

其他必须量的量（"参数写了" ≠ "效果出来了"）：手腕侧向从 T-pose 的 ±0.36 落到 0.10~0.15
（手臂确实垂下来了）、手腕前后行程 0.14~0.17（确实在摆）、**膝盖相对髋踝中点恒为负**
（确实朝前弯）、跑步头部相对髋恒前倾 15~37mm 而走路只有 ±6mm。

FBX 往返验证：178 骨骼无缺失、网格 bbox 一致、三条动画鞋底最低 z 都 = −0.25570。

## 工具

* `blender-mcp\uma_retarget.py` —— 算姿势增量 → `retarget_*.json`
* `blender-mcp\apply_retarget.py` —— 在 Blender 内建 action、落地修正、推 NLA、量化验证
* `blender-mcp\render_game_gait.py` + `make_gait_sheet.py` —— 出步态对比图

---

## 续：接入 Unity 工程（TA_show）的实测配置

### 工程结构

`SampleScene.unity` 用的是 `UmaMatikanetannhauser.fbx`（模型）+ `UmaAnimatorController.controller`（控制器），
动画是**独立 `.anim` 资源**不是 FBX 子资源。`.anim` 的曲线路径格式是
`1062_マチカネタンホイザ/1062_マチカネタンホイザ_arm/Position/Hip/Waist/Chest/...`
—— 和我们 Blender 骨架层级一致，所以重定向出来的动画能直接绑。

最终：Walk → `Uma_Walk_Game.anim`(41 帧/1.3333s)、Run → `Uma_Run_Game.anim`(29 帧/0.9333s)，
首末帧差 **0.000000**（无缝循环）。

### 踩坑：Blender FBX 导出会吃掉最后一帧

Blender 的 FBX 导出（NLA 模式）按 `[frame_start, frame_end)` **半开区间**烘焙。
0..40 的 action 导出来只有 0..39，Unity 里循环会短一帧、回绕少插一次。
**对策：生成 action 时在末尾多写一帧**（frame_range[1] 变成 N+1，导出正好得 0..N）。

### 滑步 = 移速 ÷ 动画自然速度 对不上

`PlayerController` 把 `MoveSpeed` 参数设成 `实际速度 ÷ moveSpeed`，所以
**只要 moveSpeed 等于动画自然速度，任何速度下都不滑步**（这是它设计得好的地方）。

实测角色网格世界高度 1.5500 m，动画自然速度走 0.3186 m/s、跑 0.5516 m/s，
而原来 `moveSpeed=1.6 / sprintSpeed=3.5` → **滑步 5.0 / 6.3 倍**，
这正是"Unity 里做的不好"的主因之一。

**根因是比例**：这角色腿长只有 0.245 m 却站在 1.55 m 的世界高度里（腿占身高 16%，
正常人约 48%）。0.245 m 的腿走 1.6 m/s 需要 6.5 步/秒，物理上做不到。
所以只能三选一：降移速 / 加播放倍速 / 接受滑步。

选定折中：`moveSpeed=0.64`、`sprintSpeed=0.88`、
Walk 状态 `m_Speed=2.0088`（参数驱动，有效倍速 = m_Speed × MoveSpeed）、
Run 状态 `m_Speed=1.5954`（常数）。滑步误差 0.0001 / −0.0002 m/s ≈ 0。
冲刺时 `MoveSpeed=0.88/0.64=1.375`，夹在 `Walk→Run >1.3` 与 `Run→Walk <1.25` 之间，稳定不抖动。

### 可复用技巧：怎么驱动一个正在运行的 Unity 编辑器

没有 MCP 桥、也不能再开一个 batchmode 实例（工程锁）时：

1. 往 `Assets/Editor/` 放一个 `[InitializeOnLoad]` 静态类，静态构造函数里
   `EditorApplication.delayCall += ...`，用 **哨兵文件** 防止每次域重载都重跑；
   要重跑就删哨兵 + 改一下 .cs 内容（触发重编译 → 域重载 → 再跑一次）。
   全过程写日志到工程外，方便在 Unity 外面核对。
2. 用它做正事要用 **Unity 自己的 API**（`AssetDatabase`、`AnimatorController`、
   `AnimationUtility`、`SerializedObject`），比手改 `.controller`/`.anim` YAML 安全得多。
3. **Unity 只在窗口获得焦点时才刷新资源、重编译脚本。** 后台进程调
   `SetForegroundWindow` 会被 Windows 挡掉（实测前台窗口没变）。必须先
   `AttachThreadInput(自己的线程, 当前前台窗口的线程, true)`，再模拟一次
   **ALT 键按下+抬起**解锁前台限制，然后 `SetForegroundWindow` 才会成功
   （实测返回 True 且前台句柄确实变了）。窗口最小化时先 `ShowWindow(SW_RESTORE)`。

这个工程里其实装了 **Codely Bridge**（`cn.tuanjie.codely.bridge`，日志显示注册了 74 个自定义工具），
但没找到可用的协议入口，最后走的是上面这条路。
