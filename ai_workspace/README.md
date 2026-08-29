# chain-gui AI 协作协议

code_baseline: BL-20260812-01
inherits: render_unified ai_workspace protocol

## 角色
- **主协调 AI (coze)**：任务拆解、状态追踪、验收、冲突协调
- **执行 AI-X**：按任务卡工作，commit + 登记台账

## 基线机制
- 每次主协调派发任务 = 一次基线（BUMP）
- 执行 AI 改代码前：`git commit --allow-empty -m "BL-N baseline: M_X start"`
- 执行 AI 改完代码：`git commit -m "BL-N done: <summary>"`
- 然后更新 CODE_STATE.md

## 文件头规范
- 所有 markdown 文档首行写 `code_baseline: BL-N`
- 所有 commit message 带基线号

## 任务卡位置
- coze 派发的任务卡：`ai_workspace/ai-coordinator/task_cards/`
- 加入任务卡（v3 §6.5）：`ai_workspace/ai-coordinator/task_cards/ai_join_*.md`
- 执行 AI 收到任务卡后，在自己的 `CURRENT.md` 顶部记录"当前任务"

## AI 自我注册协议（v3 §6.5 摘要）
- 新 AI 首次加入时由 coze 在 ai_workspace/ 下建空文件夹 + 写加入任务卡
- 新 AI 自己读 README + 加入任务卡 + CODE_STATE
- 新 AI 自己写 `ai_self_profile.md`（自报家门）+ `CURRENT.md` 自我介绍段 + 追加 CODE_STATE 名单
- baseline + done 两次 commit + 写 `self_review/ai_join.md`
- coze 验收后回填规划书 AI 工具链表格
