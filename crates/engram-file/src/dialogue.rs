//! 文件层 · 对话原始输入接口（三层重构 P0）。
//!
//! 职责边界（严格）：**只做字节与文件名**——原样读、append-only 追加、路径解析、命名校验。
//! **不解析格式、不认识记录类型**；JSONL 的行构造与解析全部归记忆层（`crate::dialogue_log`）。
//! 这样"对话"这个概念在文件层根本不存在，换格式不影响文件层。
//!
//! 存储形态（一工作区一份持续文件，append-only，不裁剪——记忆系统成熟前全部保留）：
//! ```text
//! <workspace>/.chain/dialogue/log.jsonl
//! ```
//! 裁剪语义将来定为「归档整个文件 + 开新文件」，而非删中间段——append-only 因此永远成立。

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// `.chain/` 下的对话目录名
pub const DIALOGUE_DIR: &str = "dialogue";
/// 一工作区一份的持续文件名（不含扩展名）
pub const DIALOGUE_FILE_STEM: &str = "log";
/// 持久文件的会话名（沿用命名校验；同时也是文件 stem）
pub const DIALOGUE_SESSION: &str = DIALOGUE_FILE_STEM;

/// 默认对话目录：`<workspace>/.chain/dialogue/`
pub fn default_dialogue_dir(workspace: &Path) -> PathBuf {
    workspace.join(".chain").join(DIALOGUE_DIR)
}

/// 会话/文件名合法性：仅 ASCII 字母数字与 `-`、`_`、`.`，不得以 `.` 开头，≤128 字符。
/// （防路径穿越；会话 id 惯例用 `s-<日期>-<短名>`，正文内容写在记录里，不要用中文做文件名。）
pub fn is_safe_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 128
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// 对话文件路径：`<workspace>/.chain/dialogue/<name>.jsonl`
pub fn dialogue_path(workspace: &Path, name: &str) -> Result<PathBuf, String> {
    if !is_safe_name(name) {
        return Err(format!(
            "对话文件名非法「{name}」：仅允许字母/数字/-/_/.，且不得以 . 开头（防路径穿越）"
        ));
    }
    Ok(default_dialogue_dir(workspace).join(format!("{name}.jsonl")))
}

/// 工作区持续对话文件的路径（一工作区一份）
pub fn workspace_dialogue_path(workspace: &Path) -> PathBuf {
    default_dialogue_dir(workspace).join(format!("{DIALOGUE_FILE_STEM}.jsonl"))
}

/// 对话目录下已有的对话文件（不含子目录；按文件名升序）。
/// 现阶段通常只有一个 `log.jsonl`；保留列表能力是为将来的"归档后开新文件"。
pub fn list_dialogue_files(workspace: &Path) -> Result<Vec<PathBuf>, String> {
    let dir = default_dialogue_dir(workspace);
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(&dir)
        .map_err(|e| format!("读对话目录失败：{e}"))?
        .flatten()
    {
        let p = entry.path();
        if p.is_file() && p.extension().and_then(|e| e.to_str()) == Some("jsonl") {
            out.push(p);
        }
    }
    out.sort();
    Ok(out)
}

// ── 原件读写（文件层的全部职责）────────────────────────────

/// 原样读字节。**不解析、不校验**——语义判断归记忆层。
/// 外部来源（DSH 会话日志等）用绝对路径直接传。
pub fn read_raw(path: &Path) -> Result<Vec<u8>, String> {
    fs::read(path).map_err(|e| format!("读对话源失败 {}：{e}", path.display()))
}

/// 原样读为文本（不做编码嗅探；非 UTF-8 用 [`read_raw`]）。
pub fn read_raw_text(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|e| format!("读对话源失败 {}：{e}", path.display()))
}

/// 文件是否存在（工作区刚建立时没有对话文件，属正常）
pub fn exists(path: &Path) -> bool {
    path.is_file()
}

/// 文件行数（不解析内容；供规模报告与增量消费判断）。
/// 空文件或缺失 → 0。
pub fn line_count(path: &Path) -> Result<usize, String> {
    if !path.is_file() {
        return Ok(0);
    }
    let raw = read_raw_text(path)?;
    Ok(raw.lines().count())
}

/// 追加一行（append-only）。调用方（记忆层）负责保证 `line` 是合法的单行文本且不含换行。
/// 目录不存在则创建；以 `create(true).append(true)` 打开，**永不截断**。
pub fn append_line(path: &Path, line: &str) -> Result<(), String> {
    if line.contains('\n') || line.contains('\r') {
        return Err("append_line：单行内容不得包含换行（一行一条记录）".into());
    }
    if line.trim().is_empty() {
        return Err("append_line：空行不入档".into());
    }
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("创建对话目录失败：{e}"))?;
    }
    let mut f = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|e| format!("打开对话文件失败 {}：{e}", path.display()))?;
    let mut buf = String::with_capacity(line.len() + 1);
    buf.push_str(line);
    buf.push('\n');
    f.write_all(buf.as_bytes())
        .map_err(|e| format!("追加对话失败：{e}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn ws() -> TempDir {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".chain").join("nodes")).unwrap();
        tmp
    }

    #[test]
    fn name_guard_blocks_traversal() {
        assert!(is_safe_name("log"));
        assert!(is_safe_name("s-2026-10-03-tauri-ui"));
        assert!(!is_safe_name("../evil"));
        assert!(!is_safe_name("a/b"));
        assert!(!is_safe_name(".hidden"));
        assert!(!is_safe_name(""));
        assert!(!is_safe_name("中文名")); // 中文写内容，不做文件名
        assert!(dialogue_path(Path::new("G:/ws"), "../evil").is_err());
    }

    #[test]
    fn append_only_never_truncates() {
        let tmp = ws();
        let p = workspace_dialogue_path(tmp.path());
        assert!(!exists(&p), "工作区刚建立时无对话文件");
        assert_eq!(line_count(&p).unwrap(), 0, "缺失文件按 0 行处理");

        append_line(&p, r#"{"k":"session"}"#).unwrap();
        append_line(&p, r#"{"k":"msg"}"#).unwrap();
        assert_eq!(line_count(&p).unwrap(), 2);

        let before = read_raw_text(&p).unwrap();
        append_line(&p, r#"{"k":"msg"}"#).unwrap();
        let after = read_raw_text(&p).unwrap();
        assert!(after.starts_with(&before), "append-only：既有内容逐字不变");
        assert_eq!(line_count(&p).unwrap(), 3);

        // 中文与标点经字节往返不失真（文件层不做编码转换）
        append_line(&p, r#"{"text":"方案 · FFT 统计波谱法"}"#).unwrap();
        assert!(read_raw_text(&p).unwrap().contains("方案 · FFT 统计波谱法"));
    }

    #[test]
    fn append_line_rejects_bad_input() {
        let tmp = ws();
        let p = workspace_dialogue_path(tmp.path());
        assert!(append_line(&p, "a\nb").is_err(), "一行一条：禁止多行");
        assert!(append_line(&p, "  \t ").is_err(), "空行不入档");
    }

    #[test]
    fn missing_read_reports_clearly() {
        let tmp = ws();
        let e = read_raw_text(&workspace_dialogue_path(tmp.path())).unwrap_err();
        assert!(e.contains("读对话源失败"), "{e}");
    }

    #[test]
    fn list_files_filters_and_sorts() {
        let tmp = ws();
        let root = tmp.path();
        append_line(&dialogue_path(root, "log").unwrap(), "{}").unwrap();
        append_line(&dialogue_path(root, "log-2025").unwrap(), "{}").unwrap();
        fs::write(default_dialogue_dir(root).join("notes.md"), "x").unwrap();
        fs::create_dir_all(default_dialogue_dir(root).join("archive")).unwrap();

        let files = list_dialogue_files(root).unwrap();
        let names: Vec<String> = files
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().to_string())
            .collect();
        assert_eq!(names, vec!["log-2025.jsonl".to_string(), "log.jsonl".to_string()]);
        assert!(list_dialogue_files(&ws().path().to_path_buf()).unwrap().is_empty());
    }
}
