---
id: node-26
type: note
title: 工具 · 视觉引擎选型 + 模型自动体检 + 渲染验证规范
parent: node-1
status: none
created: 2026-09-21T15:47:19+08:00
updated: 2026-09-21T15:59:23+08:00
revision: 3
tags:
- 视觉引擎
- VLM
- 模型体检
- 工具
- 渲染验证
rel: contains
rel_desc: 视觉引擎部署选型、模型自动体检工具、渲染验证规范
---

> 触发：视觉引擎；VLM；读图；模型体检；渲染验证；本地部署

**类型：工具 + 方法论**

## 一、本机硬件约束（决定了所有选型）

| 项 | 值 | 影响 |
|---|---|---|
| GPU | RTX 4060 Laptop **8 GB** | 常被占 6.78 GB，只剩 ~1.4 GB |
| 内存 | 15.2 GB，**可用常只剩 1.0 GB** | ← **比显存更卡**；模型必须全驻显存 |
| 磁盘 | D: 152 GB / C: 78 GB | 充足 |
| 网络 | huggingface.co **不通**，**hf-mirror.com 通** | 下载走镜像 |
| llama.cpp | b9637（2026/6/14） | 已原生支持 Qwen3-VL，不用换二进制 |

**下载镜像的坑**：hf-mirror 会中途 `curl: (56) Recv failure: Connection was reset`。
必须用 **`-C -` 断点续传 + 循环重试 + `--speed-time 60 --speed-limit 20480` 卡死自动重连**，
否则大文件永远下不完。

## 二、⚠️ 最重要的一条：VLM 不能做定量判断

同一次会话里，Qwen2.5-VL-3B 连续三次给出**与几何测量完全相反**的答案：

| 它说 | 实测 |
|---|---|
| 头占 30%、腿占 40% | 48% / 24% |
| "没有一条手臂抬起" | 手腕 z 从 0.040 → **0.321** |
| "画面没有变化" | **28% 像素变了** |

**换更大的模型也解决不了** —— 这不是参数量问题，是「VLM 不擅长定量/空间判断」的类别局限。

**正确架构是两层：**

1. **确定性测量层（主力，绝不说谎）** —— 射线检测、骨骼夹角、IK 可达性、像素级 A/B 对比、
   相机投影包围盒、镜像最近点距离。**凡是能用几何算出来的，就不要问视觉模型。**
2. **VLM 层（辅助）** —— 只做它擅长的：**读 UI/报错对话框的文字、OCR、粗判"这是不是个人形"、
   "画面里有没有明显穿帮"**。

## 三、视觉引擎部署（可切换型号）

`mcp-stack.ps1` 已改成**按 profile 选模型**，环境变量 `DSH_VISION_PROFILE` 可临时切换：

| profile | 模型 | 权重+mmproj | 峰值显存 | 适用 |
|---|---|---|---|---|
| `qwen3vl8b`（默认） | Qwen3-VL-8B Q4_K_M | 4.68 + 0.70 GB | ~6.3 GB | 效果最好；**需先关 Unity/Edge/Photos** |
| `qwen25vl3b` | Qwen2.5-VL-3B Q4_K_M | 1.80 + 1.25 GB | ~3.5 GB | 同时开 Blender+Unity 时用 |

启动前会检查可用内存并在不足时提示。模型文件放在 `C:\Users\jcm20\ollama\models\`。

## 四、模型自动体检工具 `model_checkup.py`

**一键出一份模型报告**，全部结论来自网格/骨骼/投影数据，不含主观判断。

六个部分：建模（顶点/面/三角比/松散点/非流形/边界边/连通块/UV/法线/变换）、
材质（面分布/贴图是否打包或丢失/是否走 MMD 着色器）、骨骼（数量/层级/约束/蒙皮完整度/
每顶点影响数/**伪顶点组**）、比例（由骨骼推 头/躯干/腿 占身高比）、
对称性（**绕模型自身中线**镜像的最近点距离）、多视角渲染（前/侧/后/顶/等轴 + 轮廓宽高比 + 是否被裁切）。

```python
import model_checkup
model_checkup.run()                      # 自动找模型
model_checkup.run(names=["City"])        # 指定对象
```

无骨架的普通网格也能查（自动降级）。

**两个已修的缺陷**（值得记住）：
- 对称性原本绕 **x=0** 镜像 —— 对不居中的模型毫无意义（一个 x∈[0,40] 的城市算出"中位距离 20"）。
  改为**绕模型自身 x 中线**，城市立刻显示"中位 0，左右对称良好" ✓
- 渲染后原本把所有网格的 `hide_render` 重置为 False，**会破坏用户原来的隐藏状态**。
  改为渲染前快照、渲染后精确还原。

## 五、验证渲染的两个硬要求

1. **必须带贴图**：`scene.display.shading.color_type = 'TEXTURE'`。
   MMD 材质的 `diffuse_color` 全是默认白，`MATERIAL` / `OBJECT` 模式出来一片灰白，等于没材质。
2. **速度**：`engine='BLENDER_WORKBENCH'` + `bpy.ops.render.opengl(animation=True, view_context=False)`
   → **0.02 秒/帧**（739 帧 40 秒）。EEVEE 是 3.4 秒/帧（739 帧 42 分钟），只在出成片时用。

## 六、工具坑：编辑带中文的 .ps1 会丢 BOM

用编辑器改写含中文的 PowerShell 脚本后，**UTF-8 BOM 会丢失**。
Windows PowerShell 5.1 没有 BOM 就按 ANSI 解码 → 中文注释字节被解坏 →
**报出一堆莫名其妙的语法错误，而那些行其实完全正常**（本次报了 7 处，全是假的）。

修复：
```powershell
$t = [System.IO.File]::ReadAllText($f, (New-Object System.Text.UTF8Encoding($false)))
[System.IO.File]::WriteAllText($f, $t, (New-Object System.Text.UTF8Encoding($true)))
```
校验语法的正确方式（不会被编码骗）：
```powershell
$e=$null; $tk=$null
[System.Management.Automation.Language.Parser]::ParseFile($f,[ref]$tk,[ref]$e)
```

## 补充：Qwen3-VL-8B 部署完成 + **`-fa on` 是决定性的**

### 部署结果

```
模型   Qwen3VL-8B-Instruct-Q4_K_M.gguf         4.68 GB
投影   mmproj-Qwen3VL-8B-Instruct-Q8_0.gguf    0.70 GB
路径   C:\Users\jcm20\ollama\models\
启动   mcp-stack.ps1，profile qwen3vl8b
```

### ⚠️ 不加 Flash Attention 会慢 14 倍

第一次部署后测速：生成只有 **2.6~12.2 tok/s**，大图要 47 秒。
诊断：显存 7816/8188 MB **打满**，KV 缓存挤不下 → 部分层回退 CPU。

给启动参数加 **`-fa on`** 之后：

| 指标 | 加 `-fa on` 前 | 后 | 提升 |
|---|---|---|---|
| 720p 图 总耗时 | 47.1 s | **8.7 s** | 5.4× |
| **生成速度** | 2.6 tok/s | **36.4 tok/s** | **14×** |
| Prompt 处理 | 39.8 tok/s | **126.8 tok/s** | 3.2× |

**36.4 tok/s 才是 8B Q4 在 RTX 4060 上全 GPU 应有的水平。**
`-fa on` 压缩了 KV 缓存（8B/36 层/GQA-8 → 每 token 约 144 KB），
8192 上下文从 1.18 GB 降到能装下，全部层回到 GPU。

> **教训：本地部署 VLM 时，显存"差不多够"和"够"是 14 倍的速度差。**
> 第一步就该开 Flash Attention，而不是等测速发现慢了再查。

### 质量提升（对比 3B）

同一个 720p 双模型渲染图，8B 的回答：
> "画面中有 2 个人形角色。左侧角色：**手臂被头发遮挡**，姿态不清晰……
> 右侧角色：右臂自然下垂，左臂抬起弯曲，姿态正常，无明显穿帮。"

**它自己发现了「左侧角色手臂被头发遮挡」** —— 这正是我早先靠射线遮挡测试量出来的结论
（诗歌剧手臂全长 0.172，头发半宽 0.37，胳膊伸不出头发轮廓）。3B 从来没做到过。

**但定量仍然不可靠**：同一张图问"头部占身高百分之多少"，它给出的百分比依然与几何实测不符。
**两层架构的结论不变。**

### 请求编码的坑（会伪装成"模型变傻"）

用 PowerShell 发中文 body 时，`Invoke-RestMethod` 默认**不按 UTF-8 编码**，
模型收到乱码会回复"您的消息可能不完整/包含乱码"，**看起来像模型能力问题，其实是发送端**。

正确姿势（`vision_bench.py` 里已固化）：
```powershell
$bytes = [System.Text.Encoding]::UTF8.GetBytes($json)
Invoke-RestMethod -Body $bytes -ContentType 'application/json; charset=utf-8'
```
Python 侧：`json.dumps(..., ensure_ascii=False).encode("utf-8")`
