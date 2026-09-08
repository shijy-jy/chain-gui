//! golden 契约测试（终版 §5「MCP 契约」的落地形态）：
//! 用 CARGO_BIN_EXE 拉起真实 engram-mcp 进程，重放 tools/_collect_golden.ps1 的
//! 16 条工具调用（契约 v3：12 工具），把响应与 docs/test-golden/engram-mcp-golden.json
//! 做值级归一化对比：
//!
//! - 时间戳（RFC3339 +08:00）→ "TS"（每次运行必然不同）
//! - 临时工作区路径（*engram_golden_<pid>）→ "WS"（跨机器/CI 路径不同）
//!
//! 契约故意变更时，用 tools/_collect_golden.ps1 重新固化 golden 后本测试再放行。
//! 本测试同时约束：golden 文件必须存在于仓库（include_str! 编译期读取）。

use regex::Regex;
use serde_json::Value;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;
use tempfile::TempDir;

/// 固化契约文件（相对 crates/engram-mcp/tests/ → 仓库根 docs/）
const GOLDEN: &str = include_str!("../../../docs/test-golden/engram-mcp-golden.json");

fn setup_workspace() -> TempDir {
    // 前缀与 golden 采集脚本一致（engram_golden_），归一化正则据此匹配
    let tmp = tempfile::Builder::new()
        .prefix("engram_golden_")
        .tempdir()
        .expect("临时工作区创建失败");
    std::fs::create_dir_all(tmp.path().join(".chain").join("nodes")).unwrap();
    std::fs::write(tmp.path().join(".chain").join(".mode"), "dev").unwrap();
    tmp
}

/// 归一化（值级）：时间戳 → TS、临时工作区路径 → WS
/// 路径采用**就地子串替换**（只替换引号内的路径文本，不吞掉整条响应），
/// 保证 get_overview 的 node_count/edge_count/chain_health 等字段仍被逐字段比对。
fn normalize(v: Value) -> Value {
    let ts = Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\+08:00$").unwrap();
    let ts_inside = Regex::new(r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\+08:00").unwrap();
    // [^"] 不能跨越引号：匹配起点即路径自身引号段内（Windows 双反斜杠/Linux 单斜杠均可）
    let ws = Regex::new(r#"[^"]*?engram_golden_[A-Za-z0-9]+"#).unwrap();
    match v {
        Value::Object(map) => Value::Object(
            map.into_iter()
                .map(|(k, val)| (k, normalize(val)))
                .collect(),
        ),
        Value::Array(arr) => Value::Array(arr.into_iter().map(normalize).collect()),
        Value::String(s) => {
            let replaced = ws.replace_all(&s, "WS").into_owned();
            if ts.is_match(&replaced) {
                Value::String("TS".into())
            } else {
                Value::String(ts_inside.replace_all(&replaced, "TS").into_owned())
            }
        }
        other => other,
    }
}

/// 递归按键排序（golden 文件由 PS 5.1 序列化，键序无保证；新侧由 json! 宏构造）
fn sort_json(v: Value) -> Value {
    match v {
        Value::Object(map) => {
            let mut pairs: Vec<(String, Value)> = map
                .into_iter()
                .map(|(k, val)| (k, sort_json(val)))
                .collect();
            pairs.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(pairs.into_iter().collect())
        }
        Value::Array(arr) => Value::Array(arr.into_iter().map(sort_json).collect()),
        other => other,
    }
}

/// 发一条 JSON-RPC 请求并同步读回一行响应
fn send_line(
    stdin: &mut impl Write,
    rx: &mpsc::Receiver<String>,
    req_id: &mut u32,
    method: &str,
    params: &str,
) -> String {
    *req_id += 1;
    let json =
        format!(r#"{{"jsonrpc":"2.0","id":{req_id},"method":"{method}","params":{params}}}"#);
    writeln!(stdin, "{json}").expect("写 stdin 失败");
    stdin.flush().expect("flush stdin 失败");
    rx.recv_timeout(Duration::from_secs(10))
        .expect("等待响应超时（10s）")
}

/// 发一条无响应的 notification
fn send_notification(stdin: &mut impl Write, req_id: &mut u32, method: &str) {
    *req_id += 1;
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":{req_id},"method":"{method}","params":{{}}}}"#
    )
    .expect("写 stdin 失败");
    stdin.flush().expect("flush stdin 失败");
}

/// 拉起 engram-mcp 进程，重放 12 条调用，返回 (tool, request, response) 列表。
/// reader 线程独立收 stdout 行，主线程发请求 + recv_timeout 收响应（防死锁/挂死）。
fn replay_flow(ws: &TempDir) -> Vec<(String, String, String)> {
    let exe = env!("CARGO_BIN_EXE_engram-mcp");
    let mut child: Child = Command::new(exe)
        .arg("--workspace")
        .arg(ws.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .expect("engram-mcp 进程启动失败");

    let stdout = child.stdout.take().expect("stdout 管道");
    let (tx, rx) = mpsc::channel::<String>();
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines() {
            match line {
                Ok(l) => {
                    if tx.send(l).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let mut stdin = child.stdin.take().expect("stdin 管道");
    let mut req_id = 0u32;

    // 握手（响应不参与契约对比，仅保持流对齐）
    let _ = send_line(
        &mut stdin,
        &rx,
        &mut req_id,
        "initialize",
        r#"{"protocolVersion":"2024-11-05","capabilities":{},"clientInfo":{"name":"golden","version":"1.0"}}"#,
    );
    // notifications/initialized：PS 采集流程带 id 发送，rmcp 会回一条 -32601 错误响应
    // （golden 未收录该行，仅需消费以保持响应流对齐）
    send_notification(&mut stdin, &mut req_id, "notifications/initialized");
    let _ = rx
        .recv_timeout(Duration::from_secs(10))
        .expect("notification 错误响应超时（流对齐失败）");

    let mut entries: Vec<(String, String, String)> = Vec::new();
    let mut tool_call = |name: &str, args: &str| {
        let request = format!(r#"{{"name":"{name}","arguments":{args}}}"#);
        let response = send_line(&mut stdin, &rx, &mut req_id, "tools/call", &request);
        entries.push((name.to_string(), request, response));
    };

    // 与 tools/_collect_golden.ps1 完全一致的 16 条（顺序即契约）
    tool_call(
        "create_node",
        r##"{"title":"Golden A","body":"# A\nnode A body"}"##,
    );
    // 防时序抖动：updated 为秒级精度，跨秒创建保证 search 的 updated 倒序结果确定
    // （与 _collect_golden.ps1 的 Start-Sleep 对应，两端必须一致）
    std::thread::sleep(Duration::from_millis(1100));
    tool_call(
        "create_node",
        r##"{"title":"Golden B","body":"# B\nnode B body"}"##,
    );
    tool_call(
        "link_nodes",
        r#"{"from":"node-1","to":"node-2","rel_type":"solves"}"#,
    );
    tool_call("get_overview", "{}");
    tool_call("search", r#"{"query":"Golden"}"#);
    tool_call("read_node", r#"{"id":"node-1","include_neighbors":true}"#);
    tool_call("expand", r#"{"id":"node-1","depth":2}"#);
    tool_call("read_path", r#"{"from":"node-1","to":"node-2"}"#);
    tool_call("get_guide", "{}");
    tool_call(
        "update_node",
        r#"{"id":"node-1","mode":"append","content":"\nappended note"}"#,
    );
    tool_call(
        "link_nodes",
        r#"{"from":"node-1","to":"node-2","rel_type":"bogus"}"#,
    );
    // recall：无索引工作区 → 关键词降级（mode=keyword,degraded=true），确定性无模型依赖
    tool_call("recall", r#"{"query":"Golden"}"#);
    // ── M7' 契约 v3 新增（12 工具）：断边 → 归档 → 归档可见性两档 ──
    tool_call("unlink_nodes", r#"{"from":"node-1","to":"node-2"}"#);
    tool_call(
        "archive_node",
        r#"{"id":"node-2","reason":"内容过时"}"#,
    );
    // 归档后 recall 默认过滤（仅 node-1）→ include_archived=true 找回 node-2（自 L4 起可见）
    tool_call("recall", r#"{"query":"Golden"}"#);
    tool_call("recall", r#"{"query":"Golden","include_archived":true}"#);

    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
    entries
}

#[test]
fn golden_contract_matches_committed_golden() {
    let ws = setup_workspace();
    let entries = replay_flow(&ws);

    // 新侧：与 golden 相同的 [{"tool","request","response"}] 结构
    let new_json: Value = serde_json::to_value(
        entries
            .iter()
            .map(|(tool, request, response)| {
                serde_json::json!({
                    "tool": tool,
                    "request": request,
                    "response": response,
                })
            })
            .collect::<Vec<Value>>(),
    )
    .expect("新侧序列化失败");

    let golden: Value =
        serde_json::from_str(GOLDEN).expect("固化 golden 解析失败（文件缺失或非法 JSON）");

    let new_norm = normalize(new_json);
    let golden_norm = normalize(golden);

    let new_sorted = sort_json(new_norm.clone());
    let golden_sorted = sort_json(golden_norm.clone());

    // 逐条定位差异（整表断言失败时先给出精确条目）
    let new_arr = new_sorted.as_array().expect("新侧应为数组");
    let golden_arr = golden_sorted.as_array().expect("golden 应为数组");
    assert_eq!(
        new_arr.len(),
        golden_arr.len(),
        "条目数不一致：新侧 {}，golden {}",
        new_arr.len(),
        golden_arr.len()
    );
    for (i, (a, b)) in new_arr.iter().zip(golden_arr.iter()).enumerate() {
        assert_eq!(
            a,
            b,
            "MCP 契约漂移（第 {} 条，工具 {}）：\n新侧：{}\ngolden：{}",
            i + 1,
            a.get("tool").and_then(|t| t.as_str()).unwrap_or("?"),
            serde_json::to_string_pretty(a).unwrap(),
            serde_json::to_string_pretty(b).unwrap(),
        );
    }
}
