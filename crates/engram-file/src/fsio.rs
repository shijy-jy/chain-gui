//! engram-file · 文件层共用原语（三层重构 P4，设计稿 v1 §4.1）。
//! 只做字节与格式底层：原子写、宽松解析、变更检测哈希、frontmatter 取值。
//! **不依赖记忆层**——这是"文件层不认识记忆"的编译期保证之一。

use std::path::Path;

/// 唯一写路径原语：GUI 侧写入也走这里。
pub fn atomic_write(path: &Path, content: &str) -> Result<(), String> {
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("非法文件路径：{}", path.display()))?;
    let tmp = path.with_file_name(format!(".{file_name}.tmp"));
    std::fs::write(&tmp, content).map_err(|e| format!("写临时文件失败：{e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("原子替换失败：{e}")
    })?;
    Ok(())
}

/// 二进制原子写（同 atomic_write 的 tmp/rename 语义；索引向量等非 UTF-8 数据专用，
/// 严禁经 String 转换——from_utf8_lossy 会改写字节破坏数据）
pub fn atomic_write_bytes(path: &Path, content: &[u8]) -> Result<(), String> {
    let file_name = path
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| format!("非法文件路径：{}", path.display()))?;
    let tmp = path.with_file_name(format!(".{file_name}.tmp"));
    std::fs::write(&tmp, content).map_err(|e| format!("写临时文件失败：{e}"))?;
    std::fs::rename(&tmp, path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        format!("原子替换失败：{e}")
    })?;
    Ok(())
}

/// 开发模式宽松解析兜底：无 frontmatter 的 .md 补最小 frontmatter（语义同 GUI update_node）
/// 核心唯一写路径共用原语（GUI set_parent / update_node / MCP 写入工具）
pub fn parse_lenient(raw: &str, node_id: &str) -> Result<(serde_yaml::Mapping, String), String> {
    use serde_yaml::Value as YV;
    match crate::scanner::frontmatter::parse(raw) {
        Ok(result) => Ok(result),
        Err(e) => Err(format!("解析 frontmatter 失败：{e}")),
    }
    .or_else(|_| {
        let now = crate::scanner::frontmatter::now_iso8601();
        let mut m = serde_yaml::Mapping::new();
        m.insert(YV::String("id".into()), YV::String(node_id.into()));
        m.insert(YV::String("type".into()), YV::String("note".into()));
        m.insert(YV::String("status".into()), YV::String("none".into()));
        m.insert(YV::String("title".into()), YV::String(node_id.into()));
        m.insert(YV::String("created".into()), YV::String(now.clone()));
        m.insert(YV::String("updated".into()), YV::String(now));
        m.insert(YV::String("revision".into()), YV::Number(1u64.into()));
        m.insert(YV::String("parent".into()), YV::Null);
        Ok((m, raw.to_string()))
    })
}

/// frontmatter 字符串字段取值
pub fn fm_get_str(fm: &serde_yaml::Mapping, key: &str) -> Option<String> {
    fm.get(serde_yaml::Value::String(key.into()))
        .and_then(|v| v.as_str().map(|s| s.to_string()))
}

/// frontmatter 布尔字段取值（缺省 false）
pub fn fm_get_bool(fm: &serde_yaml::Mapping, key: &str) -> bool {
    fm.get(serde_yaml::Value::String(key.into()))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

/// 标题归一化（T8 阶段一用：大小写/空白不敏感）
pub fn normalize_title_key(t: &str) -> String {
    t.chars()
        .filter(|c| !c.is_whitespace())
        .collect::<String>()
        .to_lowercase()
}

/// 变更检测哈希（非加密用途；同进程版本内一致即可）。
/// 文件层定义——walker（文件层）与 index（记忆层）共用同一口径。
pub fn content_hash(text: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    format!("{:016x}", h.finish())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn atomic_write_replaces_and_cleans_tmp() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("a.md");
        atomic_write(&p, "v1").unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "v1");
        atomic_write(&p, "v2").unwrap();
        assert_eq!(std::fs::read_to_string(&p).unwrap(), "v2");
        // 无 .tmp 残留
        let leftovers: Vec<_> = std::fs::read_dir(tmp.path())
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().contains(".tmp"))
            .collect();
        assert!(leftovers.is_empty(), "tmp 残留：{leftovers:?}");
    }

    #[test]
    fn atomic_write_bytes_preserves_non_utf8() {
        let tmp = TempDir::new().unwrap();
        let p = tmp.path().join("emb.bin");
        let bytes: Vec<u8> = vec![0, 159, 146, 150, 255, 1];
        atomic_write_bytes(&p, &bytes).unwrap();
        assert_eq!(std::fs::read(&p).unwrap(), bytes, "非 UTF-8 字节不得被改写");
    }

    #[test]
    fn parse_lenient_builds_minimal_frontmatter() {
        let (fm, body) = parse_lenient("# 我的笔记\n\n正文", "node-7").unwrap();
        assert_eq!(fm_get_str(&fm, "id").as_deref(), Some("node-7"));
        assert_eq!(fm_get_str(&fm, "type").as_deref(), Some("note"));
        assert_eq!(body, "# 我的笔记\n\n正文", "正文原样保留");
        // 有 frontmatter 的照常解析
        let with_fm = "---\nid: x\ntitle: 标题\n---\n\n内容";
        let (fm2, body2) = parse_lenient(with_fm, "x").unwrap();
        assert_eq!(fm_get_str(&fm2, "title").as_deref(), Some("标题"));
        assert_eq!(body2, "内容");
    }

    #[test]
    fn content_hash_stable_and_sensitive() {
        assert_eq!(content_hash("abc"), content_hash("abc"));
        assert_ne!(content_hash("abc"), content_hash("abd"));
        assert!(content_hash("").len() == 16, "16 位十六进制");
    }
}
