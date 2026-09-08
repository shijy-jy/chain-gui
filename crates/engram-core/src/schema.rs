//! 数据 schema 版本（宪法第 9 条 / ADR 0013 /《schema v1 定义与迁移接口》§2–§4）：
//! - `.chain/.schema` 记录 `major.minor`；缺失 = 隐式 1.0（现有全部工作区即 v1，零迁移成本）
//! - 写入方仅 GUI 添加工作区与 CLI migrate（adoption 写）；MCP 只读
//! - 读者规则：major 高于当前支持 → 拒绝打开（SCHEMA_TOO_NEW:），绝不堆版本 if 分支；
//!   minor 更高 → 正常打开（未知可选字段由 serde default 忽略）
//! - 版本语义：major = 破坏性事实源变更（A 类迁移，改写文件）；minor = 加性字段/派生物格式（B 类，重建派生物）

use crate::ops::atomic_write;
use serde_json::Value;
use std::path::Path;

/// `.chain/` 下的 schema 版本文件名
pub const SCHEMA_FILE: &str = ".schema";

/// 当前软件支持的 schema 版本（major.minor 字符串，与文档 spec §2 一致）。
/// v1.1（B 类）：派生物格式落地（`.chain/index/` 嵌入索引、`.chain/stats.json` 双时钟统计，
/// 框架 §4/T15 / §9 拍板项 5）——事实源（frontmatter 字段集/目录结构）不变。
pub const CURRENT_SCHEMA_STR: &str = "1.1";

/// 缺失 `.schema` 的隐式版本：**恒为 1.0**（spec §2 字面——未打标工作区即按 v1.0 处理），
/// 不随 CURRENT_SCHEMA_STR 漂移；1.0 工作区经 `engram-cli migrate` 走 B 类步骤升到 1.1。
pub const IMPLICIT_SCHEMA_STR: &str = "1.0";

/// schema 版本号（三位一体：frontmatter 字段集 / 目录结构 / 索引格式）
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SchemaVersion {
    pub major: u32,
    pub minor: u32,
}

impl SchemaVersion {
    /// "1.0" / "2.3" 形式解析；非法 → None
    pub fn parse(s: &str) -> Option<Self> {
        let (maj, min) = s.trim().split_once('.')?;
        Some(Self {
            major: maj.parse().ok()?,
            minor: min.parse().ok()?,
        })
    }

    pub fn current() -> Self {
        Self::parse(CURRENT_SCHEMA_STR).expect("CURRENT_SCHEMA_STR 必须可解析")
    }
}

impl std::fmt::Display for SchemaVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

fn schema_path(root: &Path) -> std::path::PathBuf {
    root.join(".chain").join(SCHEMA_FILE)
}

/// 读 `.chain/.schema`：缺失 → 隐式 1.0（spec §2：未打标工作区按 v1.0 处理，不随当前版本漂移）；
/// 非法 JSON / 非法版本串 → Err（不静默吞坏文件）
pub fn read_schema(root: &Path) -> Result<SchemaVersion, String> {
    let path = schema_path(root);
    if !path.exists() {
        return Ok(SchemaVersion::parse(IMPLICIT_SCHEMA_STR)
            .expect("IMPLICIT_SCHEMA_STR 必须可解析")); // 隐式 1.0（恒定为 v1.0 时代工作区）
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| format!("读 .schema 失败：{e}"))?;
    let v: Value = serde_json::from_str(&raw).map_err(|e| format!(".schema 不是合法 JSON：{e}"))?;
    let s = v
        .get("schema_version")
        .and_then(|x| x.as_str())
        .ok_or_else(|| ".schema 缺少 schema_version 字段".to_string())?;
    SchemaVersion::parse(s)
        .ok_or_else(|| format!(".schema 版本串非法：{s}（应为 major.minor，如 1.1）"))
}

/// 写 `.chain/.schema`（原子写；仅 GUI/CLI 调用，MCP 只读）
pub fn write_schema(root: &Path, version: SchemaVersion) -> Result<(), String> {
    let content = serde_json::to_string_pretty(&serde_json::json!({
        "schema_version": version.to_string(),
    }))
    .map_err(|e| format!("序列化 .schema 失败：{e}"))?;
    atomic_write(&schema_path(root), &content).map_err(|e| format!("写 .schema 失败：{e}"))
}

/// adoption 写：无 `.schema` 时补写当前版本（幂等；已有则原样读回）
pub fn ensure_schema(root: &Path) -> Result<SchemaVersion, String> {
    if schema_path(root).exists() {
        read_schema(root)
    } else {
        let v = SchemaVersion::current();
        write_schema(root, v)?;
        Ok(v)
    }
}

/// 读者规则（宪法第 9 条）：major 不高于当前支持 → true
pub fn is_supported(found: SchemaVersion) -> bool {
    found.major <= SchemaVersion::current().major
}

/// 打开前检查：schema 太高 → Err(SCHEMA_TOO_NEW: …)；可打开 → Ok(版本)
pub fn check_openable(root: &Path) -> Result<SchemaVersion, String> {
    let found = read_schema(root)?;
    if !is_supported(found) {
        return Err(format!(
            "SCHEMA_TOO_NEW: 工作区数据格式 v{} 高于当前软件支持 v{}，请升级 Engram（拒绝打开，绝不降级写入）",
            found,
            SchemaVersion::current(),
        ));
    }
    Ok(found)
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
    fn test_version_parse_and_roundtrip() {
        assert_eq!(
            SchemaVersion::parse("1.0").unwrap(),
            SchemaVersion { major: 1, minor: 0 }
        );
        assert_eq!(
            SchemaVersion::parse(" 2.3 ").unwrap(),
            SchemaVersion { major: 2, minor: 3 }
        );
        assert!(SchemaVersion::parse("1").is_none());
        assert!(SchemaVersion::parse("1.x").is_none());
        assert!(SchemaVersion::parse("").is_none());
        assert_eq!(SchemaVersion::current().to_string(), "1.1");
    }

    #[test]
    fn test_read_missing_is_implicit_1_0() {
        // 缺失 = 隐式 1.0（spec §2 字面：未打标工作区按 v1.0 处理，不随当前版本漂移）
        let tmp = ws();
        assert_eq!(
            read_schema(tmp.path()).unwrap(),
            SchemaVersion::parse(IMPLICIT_SCHEMA_STR).unwrap()
        );
    }

    #[test]
    fn test_write_and_read_roundtrip() {
        let tmp = ws();
        write_schema(tmp.path(), SchemaVersion { major: 1, minor: 1 }).unwrap();
        assert_eq!(read_schema(tmp.path()).unwrap(), SchemaVersion::current());
        // 未来键可忽略：带额外字段也照读
        fs::write(
            tmp.path().join(".chain").join(SCHEMA_FILE),
            "{\"schema_version\": \"1.1\", \"future\": 1}",
        )
        .unwrap();
        assert_eq!(read_schema(tmp.path()).unwrap(), SchemaVersion::current());
    }

    #[test]
    fn test_read_corrupt_schema_errors() {
        let tmp = ws();
        fs::write(tmp.path().join(".chain").join(SCHEMA_FILE), "not json").unwrap();
        assert!(read_schema(tmp.path()).is_err());
        fs::write(
            tmp.path().join(".chain").join(SCHEMA_FILE),
            "{\"schema_version\": \"x\"}",
        )
        .unwrap();
        assert!(read_schema(tmp.path()).is_err());
    }

    #[test]
    fn test_ensure_schema_adoption_and_idempotent() {
        let tmp = ws();
        // adoption 写
        assert_eq!(ensure_schema(tmp.path()).unwrap(), SchemaVersion::current());
        assert!(tmp.path().join(".chain").join(SCHEMA_FILE).exists());
        // 幂等：已有时原样读回，不重复写
        let mtime_before = fs::metadata(tmp.path().join(".chain").join(SCHEMA_FILE))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(ensure_schema(tmp.path()).unwrap(), SchemaVersion::current());
        let mtime_after = fs::metadata(tmp.path().join(".chain").join(SCHEMA_FILE))
            .unwrap()
            .modified()
            .unwrap();
        assert_eq!(mtime_before, mtime_after, "幂等调用不应重写文件");
    }

    #[test]
    fn test_check_openable_rejects_too_new() {
        let tmp = ws();
        write_schema(tmp.path(), SchemaVersion { major: 2, minor: 0 }).unwrap();
        let err = check_openable(tmp.path()).unwrap_err();
        assert!(err.starts_with("SCHEMA_TOO_NEW"), "应拒绝更高 major：{err}");
        // minor 更高 → 放行（加性变更，旧软件可忽略未知字段）
        write_schema(tmp.path(), SchemaVersion { major: 1, minor: 9 }).unwrap();
        assert!(check_openable(tmp.path()).is_ok());
    }
}
