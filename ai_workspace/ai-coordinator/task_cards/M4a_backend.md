# M4a — update_node command（ai-rust 主导）

> **M4 整体目标**：节点点击 → 弹侧栏 → 编辑 status / title / body / tags → 保存 → 写回 .md → 图谱刷新
>
> **本工单（M4a）**：ai-rust 提供后端支撑，**只做 command + 单测，不动前端**。
>
> **下一工单（M4b）**：ai-frontend 拿到本工单完成 + commit 之后启动，**做侧栏 UI + 调 update_node**。
>
> 工单位置：`G:\test1.x\ai_workspace\ai-coordinator\task_cards\M4a_backend.md`

---

## 任务卡元信息

- **code_baseline**：BL-20260812-02
- **主导 AI**：ai-rust
- **协作 AI**：无（M4b 等本卡完成后再启动）
- **预计时长**：0.5-1 人天
- **任务状态**：🆕 协作区已发布，等 ai-rust 拉单

---

## 背景

M3 实现了图谱只读可视化（5 节点 4 边 + 4 NodeType 配色 + 5 NodeStatus 边框 + dagre LR 布局）。下一步要让用户能**编辑节点**——M2 的扫描器只读，M4 需要加写入能力。

ai-frontend 在 M4b 任务卡里要做节点侧栏 UI，UI 调 `update_node` command，command 要返回新 ChainSnapshot 让前端刷新图谱。

---

## 目标

实现 `update_node` Tauri command，支持修改节点的 `title` / `status` / `body` / `tags` 四个字段，写回 `.chain/nodes/{id}.md` 文件，更新 frontmatter 元数据，返回新 ChainSnapshot。

---

## 任务步骤

### Step 0: precheck（5 条）

启动前确认：

- [ ] 当前在 `G:\test1.x\` 根目录
- [ ] 拉取本工单（已经读到 task_cards/M4a_backend.md）
- [ ] git status 干净（如果有 M3 HMR 残留的 .vite 缓存，先 git clean -fdx 排除）
- [ ] `cd src-tauri && cargo test --no-run` 编译过（确认 M2 编译链没坏）
- [ ] 读 `G:\test1.x\ai_workspace\ai-rust\CURRENT.md` 确认自己的 status 是 `idle`（如果是 `running` 就先别动，看是不是别的工单在跑）

### Step 1: 设计 UpdateFields 结构

在 `src-tauri/src/model/mod.rs` 加：

```rust
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct UpdateFields {
    pub title: Option<String>,
    pub status: Option<crate::model::node::NodeStatus>,
    pub body: Option<String>,
    pub tags: Option<Vec<String>>,
}
```

注意：所有字段都是 `Option`，None 表示不改，Some(value) 表示改。`status` 复用 `NodeStatus` 枚举（已存在）。

### Step 2: 实现 update_node command

新建 `src-tauri/src/commands/update_node.rs`：

```rust
use std::path::Path;
use crate::model::{ChainSnapshot, UpdateFields};
use crate::scanner;

/// 修改指定节点的字段并写回磁盘
///
/// # 参数
/// - `dir`: .chain/ 的父目录（如 `G:\test1.x\test-data`，注意不带 `.chain` 后缀）
/// - `node_id`: 节点 ID（如 `g-001`）
/// - `fields`: 要修改的字段（None = 不改，Some = 改）
///
/// # 返回
/// - 成功：新 ChainSnapshot（前端可以直接用这个刷新图谱）
/// - 失败：String 错误信息
#[tauri::command]
pub fn update_node(
    dir: String,
    node_id: String,
    fields: UpdateFields,
) -> Result<ChainSnapshot, String> {
    let dir = Path::new(&dir);
    let node_path = dir.join(".chain").join("nodes").join(format!("{}.md", node_id));

    if !node_path.exists() {
        return Err(format!("节点文件不存在：{}", node_path.display()));
    }

    // 1. 读原文件
    let raw = std::fs::read_to_string(&node_path)
        .map_err(|e| format!("读取失败：{}", e))?;

    // 2. 解析 frontmatter
    let (mut fm, body) = crate::scanner::frontmatter::parse(&raw)
        .map_err(|e| format!("解析 frontmatter 失败：{}", e))?;

    // 3. 应用 fields（这里 impl 在 model::node 里）
    crate::model::node::apply_update(&mut fm, &fields)
        .map_err(|e| format!("应用更新失败：{}", e))?;

    // 4. 写回文件（frontmatter + body + 时间戳/revision 自增）
    let new_content = crate::scanner::frontmatter::serialize(&fm, &body)
        .map_err(|e| format!("序列化失败：{}", e))?;
    std::fs::write(&node_path, new_content)
        .map_err(|e| format!("写回失败：{}", e))?;

    // 5. 重扫整个 chain，返回新 snapshot
    scanner::walker::scan_chain(dir)
        .map_err(|e| format!("重扫失败：{}", e))
}
```

**注意**：
- 这是 1 步同步操作（不用 async），文件 IO 很快
- 出错时给清晰的 String 错误，前端直接显示
- 用 `crate::scanner::frontmatter::parse` 和 `serialize` 复用 M2 的 frontmatter 模块（M2 已实现这两个函数）
- `apply_update` 函数要新建（在 model::node 里），见 Step 3

### Step 3: 在 model::node 实现 apply_update

在 `src-tauri/src/model/node.rs` 加：

```rust
use crate::model::UpdateFields;
use serde_yaml::Value as YamlValue;

/// 把 UpdateFields 里的 Some 字段应用到 frontmatter map
/// 自动更新 updated 时间和 revision+1
pub fn apply_update(
    fm: &mut serde_yaml::Mapping,
    fields: &UpdateFields,
) -> Result<(), String> {
    if let Some(title) = &fields.title {
        fm.insert(
            serde_yaml::Value::String("title".into()),
            serde_yaml::Value::String(title.clone()),
        );
    }
    if let Some(status) = &fields.status {
        let status_str = serde_json::to_string(status)
            .map_err(|e| format!("status 序列化失败：{}", e))?
            .trim_matches('"')
            .to_string();
        fm.insert(
            serde_yaml::Value::String("status".into()),
            serde_yaml::Value::String(status_str),
        );
    }
    if let Some(body) = &fields.body {
        // body 不在 frontmatter，在文件 body 部分。更新 body 在 update_node command 里做。
        // 这里只校验
        if body.is_empty() {
            return Err("body 不能为空".into());
        }
    }
    if let Some(tags) = &fields.tags {
        fm.insert(
            serde_yaml::Value::String("tags".into()),
            serde_yaml::Value::Sequence(
                tags.iter()
                    .map(|t| serde_yaml::Value::String(t.clone()))
                    .collect(),
            ),
        );
    }

    // 自增 revision
    let rev_key = serde_yaml::Value::String("revision".into());
    let new_rev = match fm.get(&rev_key) {
        Some(YamlValue::Number(n)) => n.as_u64().unwrap_or(0) + 1,
        _ => 1,
    };
    fm.insert(
        rev_key,
        serde_yaml::Value::Number(new_rev.into()),
    );

    // 更新 updated 时间
    let now = crate::scanner::frontmatter::now_iso8601();
    fm.insert(
        serde_yaml::Value::String("updated".into()),
        serde_yaml::Value::String(now),
    );

    Ok(())
}
```

注意：
- `body` 不进 frontmatter，但仍然校验非空
- 实际 body 写回在 command 里 `serialize(&fm, &body)` 之前要替换 `body` 变量（这里说明下，command 那边写）
- `now_iso8601` 是 M2 的简化版（无 chrono），直接用

### Step 4: 在 command 里处理 body 替换

修改 Step 2 的 update_node，把 body 字段处理加进去（在 serialize 之前）：

```rust
let mut new_body = body.clone();
if let Some(b) = &fields.body {
    new_body = b.clone();
}

// 然后 serialize(&fm, &new_body) 而不是 &body
```

### Step 5: 注册 command 到 lib.rs

在 `src-tauri/src/lib.rs` 改 `tauri::generate_handler!` 宏：

```rust
.invoke_handler(tauri::generate_handler![
    commands::scan_chain::scan_chain,
    commands::update_node::update_node,
])
```

并加 `pub mod commands;` 已有。`commands/update_node.rs` 已经被 `pub mod commands;` 自动扫描到。

### Step 6: 写 update_node 单元测试

新建 `src-tauri/src/commands/update_node.rs` 末尾 `#[cfg(test)] mod tests`：

测试用例（至少 3 条）：

1. **改 title**：扫描 test-data → 拿到 g-001 → 改 title → 验证写回后能读出新 title + revision+1 + updated 更新
2. **改 status**：改 g-001 status 从 pending → in_progress → 验证文件里 status 字段变了
3. **改 tags**：加一个新 tag → 验证 tags 数组里有新 tag
4. **节点不存在**：传 node_id="nonexistent" → 期待返回 Err
5. **body 校验空字符串**：fields.body = Some("") → 期待返回 Err

测试时**用 tempfile crate** 建临时目录 + 把 test-data 的 .chain/nodes 拷过去（不要污染真实 test-data）：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::scanner;
    use std::fs;
    use tempfile::TempDir;

    fn copy_testdata() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let src = std::path::Path::new("G:/test1.x/test-data");
        let dst = tmp.path();
        copy_dir_recursive(src, dst).unwrap();
        tmp
    }

    fn copy_dir_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
        if src.is_dir() {
            fs::create_dir_all(dst)?;
            for entry in fs::read_dir(src)? {
                let entry = entry?;
                let ty = entry.file_type()?;
                let src_path = entry.path();
                let dst_path = dst.join(entry.file_name());
                if ty.is_dir() {
                    copy_dir_recursive(&src_path, &dst_path)?;
                } else {
                    fs::copy(&src_path, &dst_path)?;
                }
            }
        }
        Ok(())
    }

    #[test]
    fn test_update_title() {
        let tmp = copy_testdata();
        let dir = tmp.path().to_str().unwrap().to_string();

        let fields = UpdateFields {
            title: Some("新的目标标题".into()),
            status: None,
            body: None,
            tags: None,
        };
        let new_snap = update_node(dir.clone(), "g-001".into(), fields).unwrap();

        // 验证新 snapshot 里的 g-001 title 已变
        let g = new_snap.nodes.iter().find(|n| n.id == "g-001").unwrap();
        assert_eq!(g.title, "新的目标标题");

        // 验证文件实际写回了
        let raw = fs::read_to_string(tmp.path().join(".chain/nodes/g-001.md")).unwrap();
        assert!(raw.contains("title: 新的目标标题"));
    }

    // 写另外 4 个测试...
}
```

### Step 7: cargo test + cargo build

```bash
cd src-tauri
cargo test            # 期望：之前 M2 的 8 单元 + 1 集成 + 新增 5 单元 = 14 个全过
cargo build           # 期望 0 error
```

### Step 8: 更新 CURRENT.md + CODE_STATE.md

改 `G:\test1.x\ai_workspace\ai-rust\CURRENT.md`：

- `current_task`: M4a update_node command 完成，等 coze 验收
- `status`: `done`
- 当前任务：Step 0/1/2/3/4/5/6/7/8 全勾

改 `G:\test1.x\ai_workspace\CODE_STATE.md`：

- `current_status`: M4a 完成，待 coze 验收
- 进行中区：M4b 待启动（ai-frontend 等本卡验收后启动）
- 已完成区追加 M4a

### Step 9: done commit + self_review

```bash
cd G:\test1.x
git add -A
git commit -m "feat: M4a update_node command + apply_update + 5 unit tests"
```

写 `G:\test1.x\ai_workspace\ai-rust\self_review\M4a.md`，包含：

- 9 步 checklist
- 5 硬指标：
  1. `cargo test` 14 条全过
  2. `cargo build` 0 error
  3. update_node 单元测试覆盖 5 个 case
  4. 写回后能再次 scan_chain 拿到新数据
  5. revision/updated 字段自增正确
- git log commit hash
- 已知问题：暂无
- 主动工程改进 / 已知小瑕疵
- 下一步：等 coze 验收 → 派 M4b 给 ai-frontend

---

## 验收标准（coze 兼任 ai-qa 角色）

coze 在 `reviews/M4a.md` 写验收记录，必须满足：

- [ ] 9 步全勾
- [ ] 5 硬指标全过
- [ ] `update_node` 函数实现：4 个字段（title/status/body/tags）+ Option 设计
- [ ] 单元测试 ≥ 5 条
- [ ] `cargo test` 全过 + `cargo build` 0 error
- [ ] 自动更新 `revision` +1 + `updated` 时间
- [ ] command 已注册到 `tauri::generate_handler!`
- [ ] 错误信息清晰（节点不存在 / body 空 / IO 失败 等场景）

---

## 已知风险 & 应对

| 风险 | 应对 |
|---|---|
| 真实 test-data 写坏了 | 用 tempfile 复制做测试，原数据不动 |
| frontmatter 序列化格式和原文件不一致 | M2 的 `frontmatter::serialize` 已经测过，本卡复用即可 |
| NodeStatus 序列化用 serde_json 取出来带引号 | 已处理：trim_matches('"') |
| ai-frontend 拿到 M4a 之前启动 | 任务卡明确说"等本卡完成后启动 M4b" |
| revision 字段不存在（旧节点没有）| 默认当 0，自增到 1 |

---

## 后续

- 本卡完成后，coze 验收 → 派 M4b 给 ai-frontend（节点侧栏 UI）
- M4b 详细设计见 `task_cards/M4b_frontend.md`（本卡验收后 coze 写）
- M4 整体验收：开发者手动跑 cargo tauri dev → 选目录 → 点节点 → 改 status → 保存 → 看到图谱边框颜色变 in_progress 黄色
