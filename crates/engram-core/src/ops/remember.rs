//! remember：记忆的**唯一切入点**（三层重构 P1 核心，设计稿 v1 §4.3 / §10）。
//!
//! 语义（用户原话）：对话是"别人给你讲解的内容"，节点是"你听完后整理好的脉络"。
//! 因此一次 remember 调用 = ① 把一句话/一次工具调用/一个决策**追加进对话账本**（原始输入），
//! ② 可选地把 AI 整理出的脉络**落成节点意图**（过守门），③ 决策留痕——**不保留也必须留一行**。
//!
//! 约束（设计稿 v1 §8）：
//! - 结构性违规（词表外 rel、自环、悬空端点、非法会话名……）→ 阻断报错；
//! - 规矩性违规（正文缺 `> 触发：` 句等）→ 不阻断：节点 frontmatter 打 `conventions` 标记，
//!   响应里提示，audit 留痕——AI 保留完全自主，违规可事后扫出。
//!
//! 溯源：本次追加的账本记录 seq 即溯源锚点，写入所有新建节点的 `origin` 字段
//! （`dialogue/log.jsonl#<seq>`）——「这条记忆从哪来」由此闭合。

use crate::dialogue as df;
use crate::dialogue_log as dl;
use crate::ops::Workspace;
use serde::Deserialize;
use serde_json::{json, Value};

/// 追加到账本的事件（`kind` 区分，与账本记录类型一一对应）
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RememberEvent {
    /// 一条对话消息（role 仅 user / assistant）
    Message { role: String, text: String },
    /// 一次工具调用轨迹（result 建议截断：账本是讲解，不是全量转储）
    Tool {
        name: String,
        #[serde(default)]
        args: String,
        #[serde(default)]
        result: String,
    },
    /// 记忆决策留痕（covers = 被本决策消费的消息 seq 区间，含端点）
    Decision {
        decided: String,
        #[serde(default)]
        covers: Option<(u64, u64)>,
        #[serde(default)]
        nodes: Vec<String>,
        reason: String,
    },
}

/// 落节点的意图（`op` 区分；全部复用现有写路径 → 守门/乐观锁/原子写/审计一个不少）
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum CommitIntent {
    Create {
        title: String,
        #[serde(default)]
        body: Option<String>,
        #[serde(default)]
        tags: Option<Vec<String>>,
        #[serde(default)]
        force: Option<bool>,
    },
    Update {
        id: String,
        mode: String,
        content: String,
        #[serde(default)]
        expected_updated: Option<String>,
    },
    Link {
        from: String,
        to: String,
        rel: String,
        #[serde(default)]
        desc: Option<String>,
    },
    Unlink { from: String, to: String },
    Archive {
        id: String,
        #[serde(default)]
        reason: Option<String>,
    },
}

/// 正文是否含检索线索句（`> 触发：`，指南 L1 编码规范的机器可查形式）
pub fn has_trigger(body: &str) -> bool {
    body.lines().any(|l| l.trim_start().starts_with("> 触发"))
}

/// remember 主入口。
///
/// `commits` 非空时 `event` 必须存在——否则溯源锚点不存在（设计稿 §I1/I2 的机械保证）。
pub fn remember(
    ctx: &Workspace,
    session: &str,
    event: Option<RememberEvent>,
    commits: Option<Vec<CommitIntent>>,
) -> Result<Value, String> {
    ctx.bump_clock()?;
    if !df::is_safe_name(session) {
        return Err(format!(
            "REMEMBER_INVALID_SESSION: 会话 id 非法「{session}」：仅允许字母/数字/-/_/.，不得以 . 开头"
        ));
    }
    if commits.is_some() && event.is_none() {
        return Err(
            "REMEMBER_NO_EVENT: 有节点意图时必须伴随一条对话事件（否则节点没有溯源锚点）".into(),
        );
    }

    let path = df::workspace_dialogue_path(&ctx.root);
    let ledger = dl::read_ledger(&ctx.root)?;
    let seq = ledger.next_seq();

    // ① 追加账本事件（先对话、后落盘——溯源锚点先于被溯源物存在）
    let mut appended: Option<Value> = None;
    if let Some(ev) = event {
        appended = Some(match ev {
            RememberEvent::Message { role, text } => {
                let role = match role.as_str() {
                    "user" => dl::Role::User,
                    "assistant" => dl::Role::Assistant,
                    _ => {
                        return Err(format!(
                            "REMEMBER_BAD_ROLE: role 仅 user/assistant，收到「{role}」"
                        ))
                    }
                };
                if text.trim().is_empty() {
                    return Err("REMEMBER_EMPTY_TEXT: 消息文本不能为空".into());
                }
                let lines = dl::append_message(&path, seq, session, role, &text)?;
                json!({ "kind": "msg", "seq": seq, "role": role.as_str(), "lines": lines })
            }
            RememberEvent::Tool {
                name,
                args,
                result,
            } => {
                if name.trim().is_empty() {
                    return Err("REMEMBER_EMPTY_TOOL: 工具名不能为空".into());
                }
                dl::append_tool(&path, seq, session, &name, &args, &result)?;
                json!({ "kind": "tool", "seq": seq, "name": name })
            }
            RememberEvent::Decision {
                decided,
                covers,
                nodes,
                reason,
            } => {
                let d = match decided.as_str() {
                    "keep" => dl::Decision::Keep,
                    "skip" => dl::Decision::Skip,
                    "revise" => dl::Decision::Revise,
                    _ => {
                        return Err(format!(
                            "REMEMBER_BAD_DECISION: decided 仅 keep/skip/revise，收到「{decided}」"
                        ))
                    }
                };
                dl::append_decision(&path, seq, session, d, covers, &nodes, &reason)?;
                json!({
                    "kind": "decision",
                    "seq": seq,
                    "decided": decided,
                    "covers": covers,
                    "nodes": nodes,
                })
            }
        });
    }

    let origin = dl::provenance(seq);
    let mut out = json!({
        "remembered": true,
        "session": session,
        "seq": seq,
        "appended": appended,
        "origin": origin,
        "created": [],
        "updated": [],
        "linked": [],
        "unlinked": [],
        "archived": [],
        "conventions": [],
        "hint": format!(
            "对话已追加并留痕（AI 指南 v{}）。节点意图已按 op 顺序执行：结构违规阻断、规矩违规仅标记（conventions 字段，不阻断）。origin 为本次对话溯源锚点。",
            ctx.guide_version()
        ),
    });

    // ② 执行节点意图（复用现有写路径；失败即停并报告已执行数量）
    let mut done = 0usize;
    if let Some(cs) = commits {
        for c in cs {
            match c {
                CommitIntent::Create {
                    title,
                    body,
                    tags,
                    force,
                } => {
                    let missing_trigger = body.as_deref().map(|b| !has_trigger(b)).unwrap_or(true);
                    let conventions = if missing_trigger {
                        Some(vec!["missing_trigger".to_string()])
                    } else {
                        None
                    };
                    let v = crate::ops::create_node_impl(
                        ctx,
                        &title,
                        body.as_deref(),
                        tags,
                        force,
                        None,
                        Some(&origin),
                        conventions.clone(),
                    )
                    .map_err(|e| commit_failed(&e, done))?;
                    let item = json!({
                        "id": v["id"],
                        "title": v["title"],
                        "origin": origin,
                        "duplicate_hint": v.get("duplicate_hint").cloned().unwrap_or(Value::Null),
                    });
                    out["created"].as_array_mut().unwrap().push(item);
                    if conventions.is_some() {
                        out["conventions"]
                            .as_array_mut()
                            .unwrap()
                            .push(json!({ "id": v["id"], "missing": ["missing_trigger"] }));
                    }
                }
                CommitIntent::Update {
                    id,
                    mode,
                    content,
                    expected_updated,
                } => {
                    let v = crate::ops::update_node_impl(
                        ctx,
                        &id,
                        &mode,
                        &content,
                        expected_updated.as_deref(),
                    )
                    .map_err(|e| commit_failed(&e, done))?;
                    out["updated"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!({ "id": id, "revision": v["revision"] }));
                }
                CommitIntent::Link {
                    from,
                    to,
                    rel,
                    desc,
                } => {
                    let v = crate::ops::link_nodes_impl(ctx, &from, &to, &rel, desc.as_deref())
                        .map_err(|e| commit_failed(&e, done))?;
                    out["linked"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!({ "from": from, "to": to, "rel": rel }));
                    let _ = v;
                }
                CommitIntent::Unlink { from, to } => {
                    crate::ops::unlink_nodes_impl(ctx, &from, &to)
                        .map_err(|e| commit_failed(&e, done))?;
                    out["unlinked"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!({ "from": from, "to": to }));
                }
                CommitIntent::Archive { id, reason } => {
                    crate::ops::archive_node_impl(ctx, &id, reason.as_deref())
                        .map_err(|e| commit_failed(&e, done))?;
                    out["archived"].as_array_mut().unwrap().push(json!({ "id": id }));
                }
            }
            done += 1;
        }
    }

    ctx.audit(
        "remember",
        "",
        &format!(
            "session={session} seq={seq} commits={done} origin={origin}"
        ),
    );
    Ok(out)
}

fn commit_failed(e: &str, done: usize) -> String {
    format!(
        "REMEMBER_COMMIT_FAILED: {e}（前 {done} 个意图已执行并留痕，见 audit.jsonl）"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::{create_node, read_node};
    use std::fs;
    use tempfile::TempDir;

    fn ws() -> TempDir {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join(".chain").join("nodes")).unwrap();
        fs::write(tmp.path().join(".chain").join(".mode"), "dev").unwrap();
        tmp
    }

    #[test]
    fn message_appends_with_auto_seq_and_provenance() {
        let tmp = ws();
        let ctx = Workspace::open(tmp.path().to_path_buf()).unwrap();
        let r = remember(
            &ctx,
            "s-1",
            Some(RememberEvent::Message {
                role: "user".into(),
                text: "把 FFT 那段整理成节点".into(),
            }),
            None,
        )
        .unwrap();
        assert_eq!(r["seq"].as_u64(), Some(1));
        assert_eq!(r["origin"], "dialogue/log.jsonl#1");
        let ledger = dl::read_ledger(tmp.path()).unwrap();
        assert_eq!(ledger.records.len(), 1);
        assert_eq!(ledger.next_seq(), 2);
        // 账本文件落盘可读
        let raw = df::read_raw_text(&df::workspace_dialogue_path(tmp.path())).unwrap();
        assert!(raw.contains("整理成节点"));
    }

    #[test]
    fn decision_with_create_commit_writes_origin_and_conventions() {
        let tmp = ws();
        let ctx = Workspace::open(tmp.path().to_path_buf()).unwrap();
        // 先追加一条消息（seq=1），再用 decision 覆盖它并落节点
        remember(
            &ctx,
            "s-1",
            Some(RememberEvent::Message {
                role: "user".into(),
                text: "讲解一下 Gerstner 波".into(),
            }),
            None,
        )
        .unwrap();
        let r = remember(
            &ctx,
            "s-1",
            Some(RememberEvent::Decision {
                decided: "keep".into(),
                covers: Some((1, 1)),
                nodes: vec!["node-1".into()],
                reason: "形成完整方案，值得独立成节点".into(),
            }),
            Some(vec![CommitIntent::Create {
                title: "方案 · Gerstner 波求和".into(),
                body: Some("> 触发：Gerstner；海浪；顶点位移\n\n正文".into()),
                tags: None,
                force: None,
            }]),
        )
        .unwrap();
        assert_eq!(r["seq"].as_u64(), Some(2));
        assert_eq!(r["origin"], "dialogue/log.jsonl#2");
        assert_eq!(r["created"][0]["id"], "node-1");
        assert_eq!(r["created"][0]["origin"], "dialogue/log.jsonl#2");
        assert_eq!(r["conventions"].as_array().unwrap().len(), 0, "有触发句 → 无违规标记");

        let node = read_node(&ctx, "node-1", None).unwrap();
        assert_eq!(node["origin"], "dialogue/log.jsonl#2");
        assert!(node.get("conventions").is_none(), "合规节点不写 conventions");

        // 账本：decision.covers 覆盖了 msg seq 1 → 无未消费
        let ledger = dl::read_ledger(tmp.path()).unwrap();
        assert!(ledger.unconsumed_seqs().is_empty());
        assert_eq!(ledger.decision_counts(), (1, 0, 0));
    }

    #[test]
    fn missing_trigger_is_annotated_not_blocked() {
        let tmp = ws();
        let ctx = Workspace::open(tmp.path().to_path_buf()).unwrap();
        let r = remember(
            &ctx,
            "s-1",
            Some(RememberEvent::Message {
                role: "assistant".into(),
                text: "整理了结论".into(),
            }),
            Some(vec![CommitIntent::Create {
                title: "没有触发句的节点".into(),
                body: Some("正文没有触发句".into()),
                tags: None,
                force: None,
            }]),
        )
        .unwrap();
        // 不阻断：节点创建成功
        assert_eq!(r["created"][0]["id"], "node-1");
        // 违规被标记
        let convs = r["conventions"].as_array().unwrap();
        assert_eq!(convs.len(), 1);
        assert_eq!(convs[0]["missing"][0], "missing_trigger");
        // 标记落进 frontmatter
        let node = read_node(&ctx, "node-1", None).unwrap();
        assert_eq!(node["conventions"][0], "missing_trigger");
    }

    #[test]
    fn skip_decision_without_commits_leaves_trace() {
        let tmp = ws();
        let ctx = Workspace::open(tmp.path().to_path_buf()).unwrap();
        remember(
            &ctx,
            "s-1",
            Some(RememberEvent::Message {
                role: "user".into(),
                text: "确认一下".into(),
            }),
            None,
        )
        .unwrap();
        let r = remember(
            &ctx,
            "s-1",
            Some(RememberEvent::Decision {
                decided: "skip".into(),
                covers: Some((1, 1)),
                nodes: vec![],
                reason: "纯确认，无新信息".into(),
            }),
            None,
        )
        .unwrap();
        assert_eq!(r["seq"].as_u64(), Some(2));
        let ledger = dl::read_ledger(tmp.path()).unwrap();
        assert_eq!(ledger.decision_counts(), (0, 1, 0));
        assert!(ledger.unconsumed_seqs().is_empty(), "skip 也算消费");
    }

    #[test]
    fn commits_without_event_are_rejected() {
        let tmp = ws();
        let ctx = Workspace::open(tmp.path().to_path_buf()).unwrap();
        let e = remember(
            &ctx,
            "s-1",
            None,
            Some(vec![CommitIntent::Create {
                title: "孤儿节点".into(),
                body: None,
                tags: None,
                force: None,
            }]),
        )
        .unwrap_err();
        assert!(e.contains("REMEMBER_NO_EVENT"), "{e}");
    }

    #[test]
    fn invalid_session_and_bad_values_are_blocked() {
        let tmp = ws();
        let ctx = Workspace::open(tmp.path().to_path_buf()).unwrap();
        assert!(remember(
            &ctx,
            "../evil",
            Some(RememberEvent::Message {
                role: "user".into(),
                text: "x".into(),
            }),
            None,
        )
        .unwrap_err()
        .contains("REMEMBER_INVALID_SESSION"));
        assert!(remember(
            &ctx,
            "s-1",
            Some(RememberEvent::Message {
                role: "robot".into(),
                text: "x".into(),
            }),
            None,
        )
        .unwrap_err()
        .contains("REMEMBER_BAD_ROLE"));
        assert!(remember(
            &ctx,
            "s-1",
            Some(RememberEvent::Decision {
                decided: "maybe".into(),
                covers: None,
                nodes: vec![],
                reason: "x".into(),
            }),
            None,
        )
        .unwrap_err()
        .contains("REMEMBER_BAD_DECISION"));
        // 空 reason 由账本层拒绝
        assert!(remember(
            &ctx,
            "s-1",
            Some(RememberEvent::Decision {
                decided: "skip".into(),
                covers: None,
                nodes: vec![],
                reason: "  ".into(),
            }),
            None,
        )
        .is_err());
    }

    #[test]
    fn structural_violation_blocks_and_reports_done_count() {
        let tmp = ws();
        let ctx = Workspace::open(tmp.path().to_path_buf()).unwrap();
        // 第二个意图是非法 rel（结构违规）→ 阻断；第一个已执行
        let e = remember(
            &ctx,
            "s-1",
            Some(RememberEvent::Message {
                role: "user".into(),
                text: "x".into(),
            }),
            Some(vec![
                CommitIntent::Create {
                    title: "第一节点".into(),
                    body: None,
                    tags: None,
                    force: None,
                },
                CommitIntent::Link {
                    from: "node-1".into(),
                    to: "node-1".into(),
                    rel: "contains".into(),
                    desc: None,
                },
            ]),
        )
        .unwrap_err();
        assert!(e.contains("REMEMBER_COMMIT_FAILED"), "{e}");
        assert!(e.contains("前 1 个意图已执行"), "{e}");
        // 第一个意图确实落地
        let node = read_node(&ctx, "node-1", None).unwrap();
        assert_eq!(node["id"], "node-1");
    }

    #[test]
    fn update_and_archive_intents_route_through_guards() {
        let tmp = ws();
        let ctx = Workspace::open(tmp.path().to_path_buf()).unwrap();
        // 用 legacy 通道建节点，再用 remember 的 update/archive 意图操作它
        create_node(&ctx, "被修订的节点", Some("> 触发：修订\n\n旧正文"), None, None).unwrap();
        let r = remember(
            &ctx,
            "s-2",
            Some(RememberEvent::Decision {
                decided: "revise".into(),
                covers: None,
                nodes: vec!["node-1".into()],
                reason: "结论变了".into(),
            }),
            Some(vec![
                CommitIntent::Update {
                    id: "node-1".into(),
                    mode: "append".into(),
                    content: "补充的新结论".into(),
                    expected_updated: None,
                },
                CommitIntent::Archive {
                    id: "node-1".into(),
                    reason: Some("被新方案取代".into()),
                },
            ]),
        )
        .unwrap();
        assert_eq!(r["updated"][0]["id"], "node-1");
        assert_eq!(r["archived"][0]["id"], "node-1");
        // 归档语义生效：活跃图不再含 node-1，archive/ 有文件
        let snap = ctx.scan().unwrap();
        assert!(snap.nodes.iter().all(|n| n.id != "node-1"));
        assert!(snap.archived.iter().any(|n| n.id == "node-1"));
        assert!(tmp.path().join(".chain/archive/node-1.md").exists());
    }
}
