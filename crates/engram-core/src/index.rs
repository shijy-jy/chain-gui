//! 嵌入索引（框架 §5.2）：`.chain/index/` 的 K 存取、哈希校验、全库重嵌。
//! 派生物（ADR 0004）：可重建、不进 YAML；节点文件变更 → 条目 stale → recall 按需重嵌。

use crate::embed::Embedder;
use crate::ops::atomic_write;
use crate::scanner::frontmatter;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

const META_FILE: &str = "meta.json";
const BIN_FILE: &str = "embeddings.bin";
pub const MODEL_NAME: &str = "bge-small-zh-v1.5";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexEntry {
    pub id: String,
    pub hash: String,
    pub archived: bool,
    pub derived: bool,
    pub row: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexMeta {
    pub format: u32,
    pub dim: usize,
    pub model: String,
    pub updated_at: String,
    pub entries: Vec<IndexEntry>,
}

pub struct IndexStore {
    root: PathBuf,
    meta: Option<IndexMeta>,
    rows: Option<Vec<Vec<f32>>>,
    /// 缓存失效锚点：meta.json 的 (mtime, len)（无文件 = None）。
    /// 长驻进程（MCP）中外部 CLI reindex 落盘后，下次访问自动重载（ADR 0004 派生物语义）。
    freshness: Option<(Option<std::time::SystemTime>, u64)>,
}

/// 变更检测哈希（非加密用途；同进程版本内一致即可）
pub fn content_hash(text: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    format!("{:016x}", h.finish())
}

fn index_dir(root: &Path) -> PathBuf {
    root.join(".chain").join("index")
}

/// meta.json 的新鲜度指纹：(mtime, len)；文件不存在 = None
fn meta_freshness(root: &Path) -> Option<(Option<std::time::SystemTime>, u64)> {
    index_dir(root)
        .join(META_FILE)
        .metadata()
        .ok()
        .map(|m| (m.modified().ok(), m.len()))
}

impl IndexStore {
    /// 懒打开：不读盘，首次访问时加载（缺失 = 空索引）
    pub fn open(root: &Path) -> Result<Self, String> {
        Ok(Self {
            root: root.to_path_buf(),
            meta: None,
            rows: None,
            freshness: None,
        })
    }

    fn ensure_loaded(&mut self) -> Result<(), String> {
        if self.meta.is_some() {
            // 缓存失效检测：外部（CLI reindex）改写了 index/meta.json → 重载
            let current = meta_freshness(&self.root);
            if self.freshness == current {
                return Ok(());
            }
        }
        let dir = index_dir(&self.root);
        let meta_path = dir.join(META_FILE);
        if !meta_path.exists() {
            self.meta = Some(IndexMeta {
                format: 1,
                dim: 0,
                model: MODEL_NAME.to_string(),
                updated_at: String::new(),
                entries: Vec::new(),
            });
            self.rows = Some(Vec::new());
            self.freshness = None;
            return Ok(());
        }
        let raw = std::fs::read_to_string(&meta_path)
            .map_err(|e| format!("读 index/meta.json 失败：{e}"))?;
        let meta: IndexMeta =
            serde_json::from_str(&raw).map_err(|e| format!("index/meta.json 解析失败：{e}"))?;
        let rows = if meta.dim > 0 {
            load_rows(&dir.join(BIN_FILE), meta.dim, meta.entries.len())?
        } else {
            Vec::new()
        };
        self.freshness = meta_freshness(&self.root);
        self.meta = Some(meta);
        self.rows = Some(rows);
        Ok(())
    }

    pub fn is_empty(&mut self) -> Result<bool, String> {
        self.ensure_loaded()?;
        Ok(self.rows.as_ref().map(|r| r.is_empty()).unwrap_or(true))
    }

    pub fn len(&mut self) -> Result<usize, String> {
        self.ensure_loaded()?;
        Ok(self.rows.as_ref().map(|r| r.len()).unwrap_or(0))
    }

    pub fn dim(&mut self) -> Result<usize, String> {
        self.ensure_loaded()?;
        Ok(self.meta.as_ref().map(|m| m.dim).unwrap_or(0))
    }

    pub fn is_stale(&mut self, id: &str, hash: &str) -> Result<bool, String> {
        self.ensure_loaded()?;
        Ok(self
            .meta
            .as_ref()
            .and_then(|m| m.entries.iter().find(|e| e.id == id))
            .map(|e| e.hash != hash)
            .unwrap_or(true))
    }

    /// 写入/更新一个条目（含归档与 derived 标记；维度不匹配返回 Err）
    pub fn upsert(
        &mut self,
        id: &str,
        hash: &str,
        vec: Vec<f32>,
        archived: bool,
        derived: bool,
    ) -> Result<(), String> {
        self.ensure_loaded()?;
        let meta = self.meta.as_mut().unwrap();
        let rows = self.rows.as_mut().unwrap();
        if meta.dim != 0 && vec.len() != meta.dim {
            return Err(format!(
                "嵌入维度不匹配：索引 {dim}，输入 {got}",
                dim = meta.dim,
                got = vec.len()
            ));
        }
        if meta.dim == 0 {
            meta.dim = vec.len();
        }
        match meta.entries.iter().position(|e| e.id == id) {
            Some(pos) => {
                rows[pos] = vec;
                let e = &mut meta.entries[pos];
                e.hash = hash.to_string();
                e.archived = archived;
                e.derived = derived;
            }
            None => {
                let row = rows.len();
                rows.push(vec);
                meta.entries.push(IndexEntry {
                    id: id.to_string(),
                    hash: hash.to_string(),
                    archived,
                    derived,
                    row,
                });
            }
        }
        Ok(())
    }

    pub fn remove(&mut self, id: &str) -> Result<(), String> {
        self.ensure_loaded()?;
        let meta = self.meta.as_mut().unwrap();
        let rows = self.rows.as_mut().unwrap();
        if let Some(pos) = meta.entries.iter().position(|e| e.id == id) {
            meta.entries.remove(pos);
            rows.remove(pos);
            for (i, e) in meta.entries.iter_mut().enumerate() {
                e.row = i;
            }
        }
        Ok(())
    }

    /// 迭代 (entry, 向量拷贝) —— 返回持有数据，避免调用方持锁跨过生命周期
    pub fn entries(&mut self) -> Result<Vec<(IndexEntry, Vec<f32>)>, String> {
        self.ensure_loaded()?;
        let meta = self.meta.as_ref().unwrap();
        let rows = self.rows.as_ref().unwrap();
        Ok(meta
            .entries
            .iter()
            .filter_map(|e| rows.get(e.row).map(|r| (e.clone(), r.clone())))
            .collect())
    }

    pub fn flush(&mut self) -> Result<(), String> {
        self.ensure_loaded()?;
        self.meta.as_mut().unwrap().updated_at = frontmatter::now_iso8601();
        let meta = self.meta.as_ref().unwrap();
        let rows = self.rows.as_ref().unwrap();
        let dir = index_dir(&self.root);
        std::fs::create_dir_all(&dir).map_err(|e| format!("创建 index/ 失败：{e}"))?;
        let json =
            serde_json::to_string_pretty(meta).map_err(|e| format!("序列化 meta 失败：{e}"))?;
        atomic_write(&dir.join(META_FILE), &json)?;
        if meta.dim > 0 {
            let mut buf = Vec::with_capacity(rows.len() * meta.dim * 4);
            for r in rows {
                for v in r {
                    buf.extend_from_slice(&v.to_le_bytes());
                }
            }
            crate::ops::atomic_write_bytes(&dir.join(BIN_FILE), &buf)?;
        }
        Ok(())
    }

    /// 全库重嵌（清空重建）；节点文本口径 = title + "\n" + body（与同义词测试集一致）
    pub fn rebuild_all(root: &Path, embedder: &dyn Embedder) -> Result<RebuildReport, String> {
        let start = std::time::Instant::now();
        let mut store = IndexStore::open(root)?;
        store.ensure_loaded()?;
        // 清空旧条目
        store.meta.as_mut().unwrap().entries.clear();
        store.rows.as_mut().unwrap().clear();
        let mut re_embedded = 0usize;
        let mut skipped = 0usize;
        let mut ids = Vec::new();
        let nodes_dir = root.join(".chain").join("nodes");
        if nodes_dir.is_dir() {
            for entry in std::fs::read_dir(&nodes_dir).map_err(|e| format!("读 nodes 失败：{e}"))?
            {
                let path = entry.map_err(|e| format!("读目录项失败：{e}"))?.path();
                if path.extension().and_then(|s| s.to_str()) != Some("md") {
                    continue;
                }
                let raw = std::fs::read_to_string(&path)
                    .map_err(|e| format!("读 {} 失败：{e}", path.display()))?;
                let (fm, body) = match frontmatter::parse(&raw) {
                    Ok(v) => v,
                    Err(e) => {
                        skipped += 1;
                        eprintln!("reindex：跳过 {}（{e}）", path.display());
                        continue; // 宽松/损坏文件：跳过（无 frontmatter 无法取 id）
                    }
                };
                let Some(id) = fm
                    .get(serde_yaml::Value::String("id".into()))
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
                else {
                    skipped += 1;
                    eprintln!("reindex：跳过 {}（frontmatter 缺 id）", path.display());
                    continue;
                };
                let title = fm
                    .get(serde_yaml::Value::String("title".into()))
                    .and_then(|v| v.as_str())
                    .unwrap_or(&id)
                    .to_string();
                ids.push((id, title + "\n" + &body));
            }
        }
        if !ids.is_empty() {
            let texts: Vec<String> = ids.iter().map(|(_, t)| t.clone()).collect();
            let vecs = embedder
                .embed(&texts)
                .map_err(|e| format!("全库重嵌失败：{e}"))?;
            for ((id, _), v) in ids.iter().zip(vecs) {
                let raw =
                    std::fs::read_to_string(nodes_dir.join(format!("{id}.md"))).unwrap_or_default();
                store.upsert(id, &content_hash(&raw), v, false, false)?;
                re_embedded += 1;
            }
        }
        store.flush()?;
        Ok(RebuildReport {
            total: re_embedded,
            re_embedded,
            skipped,
            elapsed_ms: start.elapsed().as_millis() as u64,
        })
    }
}

pub struct RebuildReport {
    pub total: usize,
    pub re_embedded: usize,
    /// 解析失败/缺 id 被跳过的节点数（宽松文件不进索引，召回走关键词仍可见）
    pub skipped: usize,
    pub elapsed_ms: u64,
}

fn load_rows(bin: &Path, dim: usize, count: usize) -> Result<Vec<Vec<f32>>, String> {
    let bytes = std::fs::read(bin).map_err(|e| format!("读 embeddings.bin 失败：{e}"))?;
    if bytes.len() != count * dim * 4 {
        return Err(format!(
            "embeddings.bin 大小不符：期望 {}，实际 {}（索引损坏，请 reindex 重建）",
            count * dim * 4,
            bytes.len()
        ));
    }
    let mut rows = Vec::with_capacity(count);
    for c in 0..count {
        let mut row = Vec::with_capacity(dim);
        for d in 0..dim {
            let off = (c * dim + d) * 4;
            row.push(f32::from_le_bytes([
                bytes[off],
                bytes[off + 1],
                bytes[off + 2],
                bytes[off + 3],
            ]));
        }
        rows.push(row);
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embed::{EmbedError, Embedder};
    use std::fs;
    use tempfile::TempDir;

    /// 固定向量 stub：任何文本返回同一向量（CI 无真实模型，测试不依赖模型文件）
    struct Stub {
        dim: usize,
        value: Vec<f32>,
    }
    impl Embedder for Stub {
        fn embed(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbedError> {
            Ok(texts.iter().map(|_| self.value.clone()).collect())
        }
        fn dim(&self) -> usize {
            self.dim
        }
    }

    fn ws() -> TempDir {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".chain").join("nodes")).unwrap();
        tmp
    }

    fn write_dev_node(tmp: &TempDir, id: &str, title: &str) {
        let content = format!(
            "---\nid: {id}\ntype: note\ntitle: {title}\nparent: null\nstatus: none\ncreated: 2026-09-01T10:00:00+08:00\nupdated: 2026-09-01T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# {title}\n"
        );
        fs::write(
            tmp.path()
                .join(".chain")
                .join("nodes")
                .join(format!("{id}.md")),
            content,
        )
        .unwrap();
    }

    #[test]
    fn upsert_flush_reopen_roundtrip() {
        let tmp = ws();
        let mut store = IndexStore::open(tmp.path()).unwrap();
        assert!(store.is_empty().unwrap(), "新工作区应为空索引");
        let v = vec![1.0f32, 0.0];
        store
            .upsert("a", "hash-a", v.clone(), false, false)
            .unwrap();
        store.flush().unwrap();
        assert!(tmp.path().join(".chain/index/meta.json").exists());
        assert!(tmp.path().join(".chain/index/embeddings.bin").exists());

        let mut reopened = IndexStore::open(tmp.path()).unwrap();
        assert_eq!(reopened.len().unwrap(), 1);
        assert_eq!(reopened.dim().unwrap(), 2);
        assert!(!reopened.is_stale("a", "hash-a").unwrap(), "同哈希应新鲜");
        assert!(reopened.is_stale("a", "other").unwrap(), "异哈希应 stale");
        assert!(reopened.is_stale("missing", "hash-a").unwrap());
        let entries = reopened.entries().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0.id, "a");
        assert_eq!(entries[0].1, v);
        assert!(!entries[0].0.archived);
    }

    #[test]
    fn upsert_replace_and_remove() {
        let tmp = ws();
        let mut store = IndexStore::open(tmp.path()).unwrap();
        store
            .upsert("a", "h1", vec![1.0, 0.0], false, false)
            .unwrap();
        // 同 id 再 upsert = 替换（含 archived 标记刷新）
        store
            .upsert("a", "h2", vec![0.0, 1.0], true, false)
            .unwrap();
        let e = store.entries().unwrap();
        assert_eq!(e.len(), 1);
        assert_eq!(e[0].0.hash, "h2");
        assert!(e[0].0.archived);
        assert_eq!(e[0].1, vec![0.0f32, 1.0]);

        store.remove("a").unwrap();
        assert!(store.is_empty().unwrap());
        store.remove("ghost").unwrap(); // 幂等
    }

    #[test]
    fn upsert_rejects_dim_mismatch() {
        let tmp = ws();
        let mut store = IndexStore::open(tmp.path()).unwrap();
        store
            .upsert("a", "h", vec![1.0, 0.0], false, false)
            .unwrap();
        assert!(
            store
                .upsert("b", "h", vec![1.0, 0.0, 0.0], false, false)
                .is_err(),
            "维度不符应拒绝"
        );
    }

    #[test]
    fn rebuild_all_with_stub_embedder() {
        let tmp = ws();
        write_dev_node(&tmp, "a", "甲");
        write_dev_node(&tmp, "b", "乙");
        // BOM 文件（PS Set-Content -Encoding UTF8 陷阱）应被 frontmatter::parse 剥 BOM 后正常嵌入
        let bom_node = "\u{feff}---\nid: bom\ntype: note\ntitle: BOM 节点\nparent: null\nstatus: none\ncreated: 2026-09-01T10:00:00+08:00\nupdated: 2026-09-01T10:00:00+08:00\nrevision: 1\ntags: []\n---\n\n# BOM\n";
        fs::write(tmp.path().join(".chain/nodes/bom.md"), bom_node).unwrap();
        // 损坏文件（无 frontmatter）应被跳过并计数，不中断全库重建
        fs::write(
            tmp.path().join(".chain/nodes/broken.md"),
            "# 无 frontmatter 的宽松文件\n",
        )
        .unwrap();
        let stub = Stub {
            dim: 2,
            value: vec![1.0, 0.0],
        };
        let report = IndexStore::rebuild_all(tmp.path(), &stub).unwrap();
        assert_eq!(report.re_embedded, 3, "BOM 文件应正常嵌入");
        assert_eq!(report.skipped, 1, "损坏文件应被跳过并计数");
        let mut store = IndexStore::open(tmp.path()).unwrap();
        assert_eq!(store.len().unwrap(), 3);
        // 哈希与节点文件内容一致（变更检测口径）
        for (e, _) in store.entries().unwrap() {
            let raw =
                fs::read_to_string(tmp.path().join(".chain/nodes").join(format!("{}.md", e.id)))
                    .unwrap();
            assert_eq!(e.hash, content_hash(&raw));
        }
        // 空工作区 → 0
        let tmp2 = ws();
        let r2 = IndexStore::rebuild_all(tmp2.path(), &stub).unwrap();
        assert_eq!(r2.re_embedded, 0);
    }

    #[test]
    fn corrupted_bin_reports_reindex_hint() {
        let tmp = ws();
        let mut store = IndexStore::open(tmp.path()).unwrap();
        store
            .upsert("a", "h", vec![1.0, 0.0], false, false)
            .unwrap();
        store.flush().unwrap();
        // 截断 embeddings.bin（2 维 1 行应为 8 字节）
        fs::write(
            tmp.path().join(".chain/index/embeddings.bin"),
            vec![1u8, 2, 3],
        )
        .unwrap();
        let mut reopened = IndexStore::open(tmp.path()).unwrap();
        let err = reopened.entries().unwrap_err();
        assert!(err.contains("reindex"), "损坏应提示 reindex：{err}");
    }

    #[test]
    fn external_reindex_visible_after_cache() {
        let tmp = ws();
        let mut store = IndexStore::open(tmp.path()).unwrap();
        assert!(store.is_empty().unwrap(), "初始应为空索引（并缓存空态）");
        // 外部重嵌（模拟长驻进程外 CLI reindex 落盘）
        write_dev_node(&tmp, "x", "X");
        let stub = Stub {
            dim: 2,
            value: vec![1.0, 0.0],
        };
        IndexStore::rebuild_all(tmp.path(), &stub).unwrap();
        // 同一 store 实例必须看见新索引（缓存失效检测）
        assert!(!store.is_empty().unwrap(), "外部 reindex 后应重载索引");
        assert_eq!(store.len().unwrap(), 1);
        assert_eq!(store.dim().unwrap(), 2);
    }
}
