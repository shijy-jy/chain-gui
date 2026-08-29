//! 过程日志模块（v1.2）：`.chain/PROCESS_LOG.md` 带时间戳的试错流水账。
//! 与图谱节点互补：节点记结论，PROCESS_LOG 记过程（环境坑/失败尝试/关键转折）。
//! 该文件不在 nodes/ 下，不会被扫描器当节点解析。
use std::fs;
use std::path::PathBuf;
use tauri::command;
use crate::scanner::frontmatter::now_iso8601;

const LOG_NAME: &str = "PROCESS_LOG.md";

fn log_path(dir: &str) -> PathBuf {
    PathBuf::from(dir).join(".chain").join(LOG_NAME)
}

/// 追加一条带时间戳的过程日志。
/// 幂等追加，不覆盖已有内容；日志文件不存在时自动创建（带表头）。
#[command]
pub fn append_log(dir: String, text: String) -> Result<String, String> {
    let text = text.trim();
    if text.is_empty() {
        return Err("日志内容不能为空".into());
    }

    let path = log_path(&dir);
    let ts = now_iso8601();

    if !path.exists() {
        let header = format!("# 过程日志\n\n> 试错流水账：环境坑 / 失败尝试 / 关键转折。按时间倒序追加，一行一条。\n> 结论仍以图谱节点为准，这里的记录是给后来者的铺路石。\n\n");
        fs::write(&path, header).map_err(|e| format!("创建过程日志失败：{e}"))?;
    }

    let mut content = fs::read_to_string(&path).map_err(|e| format!("读过程日志失败：{e}"))?;
    // 单行日志（内部换行转为 "；"）
    let one_line = text.replace('\n', "；");
    if !content.ends_with('\n') {
        content.push('\n');
    }
    content.push_str(&format!("- {} {}\n", ts, one_line));
    fs::write(&path, content).map_err(|e| format!("写过程日志失败：{e}"))?;

    Ok(ts)
}

/// 读取过程日志全文（不存在返回空字符串）
#[command]
pub fn get_process_log(dir: String) -> Result<String, String> {
    let path = log_path(&dir);
    if !path.exists() {
        return Ok(String::new());
    }
    fs::read_to_string(&path).map_err(|e| format!("读过程日志失败：{e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_append_creates_log_with_header() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().to_str().unwrap().to_string();
        fs::create_dir_all(tmp.path().join(".chain")).unwrap();

        append_log(dir.clone(), "环境坑：测试一条".into()).unwrap();

        let content = fs::read_to_string(log_path(&dir)).unwrap();
        assert!(content.contains("# 过程日志"));
        assert!(content.contains("环境坑：测试一条"));
        assert!(content.contains("+08:00"), "应有时间戳: {}", content);
    }

    #[test]
    fn test_append_multiple_lines() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().to_str().unwrap().to_string();
        fs::create_dir_all(tmp.path().join(".chain")).unwrap();

        append_log(dir.clone(), "第一条".into()).unwrap();
        append_log(dir.clone(), "第二条".into()).unwrap();
        append_log(dir.clone(), "多行\n内容\n合并".into()).unwrap();

        let content = fs::read_to_string(log_path(&dir)).unwrap();
        assert!(content.contains("第一条"));
        assert!(content.contains("第二条"));
        assert!(content.contains("多行；内容；合并"), "多行应合并为单行: {}", content);
        // 三条日志行
        let log_lines = content.lines().filter(|l| l.starts_with("- ")).count();
        assert_eq!(log_lines, 3);
    }

    #[test]
    fn test_append_empty_rejected() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().to_str().unwrap().to_string();
        let result = append_log(dir, "   ".into());
        assert!(result.is_err());
    }

    #[test]
    fn test_get_log_missing_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().to_str().unwrap().to_string();
        let content = get_process_log(dir).unwrap();
        assert_eq!(content, "");
    }
}
