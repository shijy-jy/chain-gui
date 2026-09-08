//! audit.jsonl 审计日志（框架 §4/T15：append-only 派生物，ADR 0004 可重建、不进 YAML）。
//! - 记录动作：写入（create/update/link/archive/unlink）、蒸馏（consolidate）、迁移（migrate）、冻结（freeze）
//! - 行格式：JSON per line `{"ts","action","node_id","detail"}`
//! - 追加失败不阻断主写入（审计是派生物）：打 stderr 继续

use crate::scanner::frontmatter::now_iso8601;
use std::path::Path;

pub const AUDIT_FILE: &str = "audit.jsonl";

fn audit_path(root: &Path) -> std::path::PathBuf {
    root.join(".chain").join(AUDIT_FILE)
}

/// 追加一条审计记录。主写入成功后才调用（顺序：先事实源、后审计）。
pub fn append(root: &Path, action: &str, node_id: &str, detail: &str) -> Result<(), String> {
    let line = serde_json::json!({
        "ts": now_iso8601(),
        "action": action,
        "node_id": node_id,
        "detail": detail,
    });
    let mut content = serde_json::to_string(&line).map_err(|e| format!("序列化审计失败：{e}"))?;
    content.push('\n');
    let path = audit_path(root);
    let dir = path.parent().expect(".chain/audit.jsonl 必有父目录");
    if !dir.exists() {
        std::fs::create_dir_all(dir).map_err(|e| format!("创建 .chain 失败：{e}"))?;
    }
    use std::io::Write;
    match std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        Ok(mut f) => {
            f.write_all(content.as_bytes())
                .map_err(|e| format!("写 audit.jsonl 失败：{e}"))?;
            Ok(())
        }
        Err(e) => Err(format!("打开 audit.jsonl 失败：{e}")),
    }
}

/// 只读：读出全部审计行（调试/未来 GUI 审计视图；当前无调用方，测试覆盖）
pub fn read_all(root: &Path) -> Result<Vec<serde_json::Value>, String> {
    let path = audit_path(root);
    if !path.exists() {
        return Ok(Vec::new());
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| format!("读 audit.jsonl 失败：{e}"))?;
    raw.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(|e| format!("audit.jsonl 行解析失败：{e}")))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn ws() -> TempDir {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".chain")).unwrap();
        tmp
    }

    #[test]
    fn append_and_read_roundtrip() {
        let tmp = ws();
        append(tmp.path(), "create", "node-1", "title=测试节点").unwrap();
        append(tmp.path(), "update", "node-1", "append").unwrap();
        let rows = read_all(tmp.path()).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["action"], "create");
        assert_eq!(rows[0]["node_id"], "node-1");
        assert!(!rows[0]["ts"].as_str().unwrap().is_empty());
        assert_eq!(rows[1]["action"], "update");
        // 追加语义：两次调用不覆盖
        let raw = fs::read_to_string(tmp.path().join(".chain/audit.jsonl")).unwrap();
        assert_eq!(raw.lines().count(), 2);
    }

    #[test]
    fn read_missing_is_empty() {
        let tmp = ws();
        assert!(read_all(tmp.path()).unwrap().is_empty());
    }
}
