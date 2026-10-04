---
id: node-29
type: note
title: 参考 · 从赛马娘游戏资源里逆向提取角色动画（UnityFS/UnityPy，已打通 80%）
parent: node-28
status: none
created: 2026-09-22T19:59:53+08:00
updated: 2026-09-22T20:26:24+08:00
revision: 3
tags:
- 赛马娘
- UnityFS
- UnityPy
- AnimationClip
- DenseClip
- 动作提取
- CRC32路径哈希
rel: alternative
rel_desc: 做诗歌剧移动动画时，为了拿到游戏原版走跑动作而逆向游戏资源的完整调研
---

> 触发：从游戏里提动作；解包 AssetBundle；Unity 动画曲线怎么解；streamed clip 格式；赛马娘素材

> **状态更新：已 100% 打通**（标题里的"80%"是历史遗留）。streamed 段已解，游戏原版走/跑/转身
> 三条动画已重定向到我们的 Q 版骨架并导出 FBX。

## 结论先说

**完全可行，不需要破解任何加密。** 三段（streamed / dense / constant）全部解通，
四元数模长验证全部为 1.00000。**卡了很久的坑只有一个：`m_StreamedClip.data` 暴露出来的是
uint32 原始位模式数组，不是 float 数组。**

## 环境事实

* 游戏本体：`D:\saimaniang\Umamusume\`（Unity 2022.3）
* 资源库：`umamusume_Data\Persistent\`
  * `meta` —— **SQLite**，表 `a(i,n,d,g,l,c,h,m,k,s,p)`：`n`=资源名，`h`=包哈希，31 万条
  * `dat\<哈希前2位>\<完整哈希>` —— 资源包
* **包没有加密**：文件头就是标准 `UnityFS`
* `pip install UnityPy`（1.25.3）即可直接读

### 资源名都带路径前缀（容易踩）

`n` 字段是 `3d/motion/event/body/type00/anm_eve_type00_run01_loop` 这种完整路径，
**所以 `LIKE 'anm_eve_%'` 匹配不到**，必须写 `LIKE '%anm_eve_%'`。

* 走路：`.../type00/anm_eve_type00_walk01~07_loop`（`walk01_loop` 是标准走，40 帧）
* 跑步：`.../type00/anm_eve_type00_run01~17_loop`（`run01_loop` 28 帧）
* 转身：`.../type00/anm_eve_type00_turn01~08`（一次性，无 `_loop`）
* 角色 1062 模型 prefab：哈希 `7UXEOO7GVBPWKYCZHG2VV3KAPYLMEFC5`（123 个 Transform）

## 骨骼路径哈希 = 标准 CRC32

`zlib.crc32(相对 Animator 物体的完整路径)`（不是 `Animator.StringToHash` 的 mul33）。
路径不带 prefab 根名：`Position/Hip/UpBody_Ctrl/Waist/Spine/Chest/...`。
116 条绑定里 **116/116 全部命中**（详见下面「最后 6 条」）。

## 动画存储：三段拼起来才完整

```
[0, streamCount)                 -> m_StreamedClip  变长关键帧（躯干）
[streamCount, +denseCurveCount)  -> m_DenseClip     定帧采样 30fps（四肢/手指/髋位移）
[剩余]                           -> m_ConstantClip  常量（缩放，全 1.0）
```

每条曲线按 `genericBindings` 的**顺序**分配标量：旋转 4 个（x,y,z,w），位移/缩放 3 个。
跑 76+195+140=411，走 68+89+254=411；411 = 63 旋转×4 + 42 缩放×3 + 11 位移×3 ✓

### streamed 段（**已解通 —— 这就是那个坑**）

格式来自 Unity 引擎自己的头文件
`Runtime/mecanim/animation/streamedclip.h`（镜像：`git.warmcat.org/Unity431f1`）：

```cpp
struct CurveTimeData { float time; UInt32 count; };        // 8 字节块头
struct CurveKey      { int curveIndex; float coeff[4]; };  // 20 字节
// 所有曲线的关键帧按时间排序，同一时刻的 count 个 key 紧随块头
```

**关键陷阱**：`m_StreamedClip.data` 在 UnityPy 里取出来的是 **uint32 原始位模式数组**——
整型字段（`count` / `curveIndex`）就是整数本身，浮点字段（`time` / `coeff`）是 IEEE754 位模式。
之前的代码 `struct.pack('<f', *[float(x) for x in sc.data])` 把它当 float 重打包，数据全毁，
所以怎么对都对不上。正确做法：

```python
def _fbits(u):                      # uint32 位模式 -> float
    return struct.unpack('<f', struct.pack('<I', u & 0xFFFFFFFF))[0]

pos = 0
while pos + 2 <= len(data):
    t   = _fbits(data[pos])         # 块头 time
    n   = data[pos + 1]             # 块头 count（普通整数）
    pos += 2
    if n <= 0: break                # (time=+inf, count=0) 是终止符
    for _ in range(n):
        idx  = data[pos]            # 曲线序号（普通整数）
        co   = [_fbits(data[pos+1+j]) for j in range(4)]
        pos += 5
```

铁的验证：**重组出的四元数模长必须 = 1.00000**。跑/走两条 clip 的 63 条旋转曲线全部通过。

首块 `time` 的位模式是 `0xFF7FFFFF` = −FLT_MAX，代表「初始姿势」；
末块 `(0x7F800000=+Inf, count=0)` 是终止符。跑 clip 的块头时间是
帧 0、7、8、9…26（30fps）——即躯干只有 6~8 个关键帧，其余靠插值。

**插值语义**（用实数据验证过，不是猜的）：

$$p(\tau) = ((c_0\tau + c_1)\tau + c_2)\tau + c_3,\qquad \tau = t - t_k$$

注意 **τ 是未归一化的绝对时间偏移**，不是 [0,1] 的段内参数。验证：t=0 那个键的
多项式在 τ=0.233333 处算出 0.011929，与下一键（帧 7）的存储值逐位吻合；
第二段同样精确命中 0.034918。

### dense 段（已解通）

`m_SampleArray` 是 `frameCount × curveCount` 的扁平 float 数组（行主序），
四元数直接读，不需要 `m_ValueArrayDelta` 缩放。验证：42 条旋转曲线模长 0.9998~1.0001。

### constant 段（已解通）

`m_ConstantClip.data` 按同一套标量顺序对应末尾那段，实测缩放值都是 1.0 ✓

### 最后 6 条未解析绑定

不是核心骨骼，是**脚部 IK 的目标/极向量**（在 `pfb_bdy1062_00` 层级之外，所以
预fab 里查不到）。暴力枚举 `Position/<词><L/R><后缀>` 反查命中：

| CRC | 路径 |
|---|---|
| 1713661633 | `Position/Foot_L_Pole` |
| 1610089770 | `Position/Foot_R_Pole` |
| 4024988875 | `Position/Foot_L_Target` |
| 2262390738 | `Position/Foot_R_Target` |

**这些的存在说明游戏骨架带 IK 脚部修正**——原版动画的脚部落地是 IK 后的结果。

## 工具

* `blender-mcp\uma_motion_extract.py` —— 完整解码器（streamed+dense+constant 合成），
  `python uma_motion_extract.py <哈希> <out.json> [--frames N]`，另有 `find <关键字>`
* `blender-mcp\uma_retarget.py` —— 重定向到我们的 Q 版骨架（见节点 node-30）
* `blender-mcp\aplaybox.py` —— 模之屋检索/详情（**下载需登录**，`.pbv` 是加密的）
* `blender-mcp\bvh_gait.py` / `ref_gait.py` / `vmd_analyze.py` —— 步态量化

## 另一条更省事的路（没走）

**UmaViewer 自带录制 VMD 功能**——`Config.json` 里就有 `VmdKeyReductionLevel`。
录一段 VMD，我们这边的 VMD 工具链（`import_vmd.py` + 骨骼字典）是现成的。
