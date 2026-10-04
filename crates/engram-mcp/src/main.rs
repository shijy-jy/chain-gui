//! Engram MCP server（规划书 v1.1 阶段一）：
//! stdio 传输的 MCP 服务，把图谱核心操作层（engram_core::ops）映射为 MCP 工具。
//! 用法：engram-mcp --workspace <工作区目录>
//! 注意：stdout 是 JSON-RPC 协议通道，日志只能走 stderr（eprintln!）。

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context as _;
use engram_core::ops::{self as mcp, Workspace};
use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters, ServerHandler},
    model::{
        CallToolResult, ContentBlock, ErrorData, Implementation, ServerCapabilities, ServerInfo,
    },
    schemars, tool, tool_handler, tool_router,
    transport::stdio,
    ServiceExt,
};
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone)]
struct EngramMcp {
    ctx: Arc<Workspace>,
    /// D3：写入串行队列——所有写入工具先拿锁，多客户端并发调用也串行落盘
    write_lock: Arc<tokio::sync::Mutex<()>>,
    /// rmcp #[tool_router] 约定字段：构造时登记工具路由（宏生成代码持有使用）
    #[allow(dead_code)]
    tool_router: ToolRouter<Self>,
}

/// 核心层 Result 映射为 MCP 响应：Ok → JSON 文本（统一注入 guide_version——
/// 设计稿 v1 §10：版本变化当次可见，AI 自主决定重读指南）；Err → 协议级错误（isError）
fn to_result(r: Result<Value, String>, guide_version: u32) -> Result<CallToolResult, ErrorData> {
    match r {
        Ok(mut v) => {
            if let Some(obj) = v.as_object_mut() {
                obj.insert("guide_version".to_string(), serde_json::json!(guide_version));
            }
            Ok(CallToolResult::success(vec![ContentBlock::text(
                serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string()),
            )]))
        }
        Err(e) => Err(ErrorData::internal_error(e, None)),
    }
}

// ── 工具参数 ──────────────────────────────────────────────

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct SearchParams {
    /// 检索关键词（title/tags/body 子串匹配，大小写不敏感）
    query: String,
    /// 返回条数上限（默认 10，最大 100）
    limit: Option<usize>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ReadNodeParams {
    /// 节点 id
    id: String,
    /// true 时附父节点与子节点列表
    include_neighbors: Option<bool>,
    /// v2.20：true 时附代码骨架（code_map_md 全文 + code_map_stale 陈旧标记；
    /// 未挂载骨架时为 null——概念节点的可执行证据，实现级还原走这里）
    include_code_map: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ExpandParams {
    /// 中心节点 id
    id: String,
    /// 扩展层数：仅 1 或 2（默认 1）
    depth: Option<u32>,
    /// 沿链梳理方向：children（从根向下）/ parents（回溯来源）/ both（默认，无向）
    direction: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ReadPathParams {
    /// 起点节点 id
    from: String,
    /// 终点节点 id
    to: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RecallParams {
    /// 语义检索查询（自然语言描述要找的记忆）
    query: String,
    /// 返回条数上限（默认 10，最大 100）
    k: Option<usize>,
    /// true 时纳入已归档节点（默认 false 过滤）
    include_archived: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ConsolidateParams {
    /// 仅蒸馏这些节点参与的簇（可选；缺省 = 全部连通分量）
    targets: Option<Vec<String>>,
    /// false 时真正创建 [蒸馏] 骨架节点（默认 true = 只出计划）
    dry_run: Option<bool>,
    /// 簇数上限（默认 8，最大 100）
    k: Option<usize>,
}

/// 三层重构：remember 的落节点意图（op 区分；全部复用现有写路径，过守门/乐观锁/审计）
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
struct CommitParams {
    /// create / update / link / unlink / archive
    op: String,
    /// create：节点标题
    title: Option<String>,
    /// create：正文（缺「> 触发：」句只标记不阻断）
    body: Option<String>,
    /// create：标签列表
    tags: Option<Vec<String>>,
    /// create：跳过同名/重复检测强制另建
    force: Option<bool>,
    /// create（3.2.1 分析模式必填）：goal / design / task / verification
    node_type: Option<String>,
    /// create（分析模式必填）/ update（可选状态流转）：pending / in_progress / success / failed / blocked（dev 模式另含 none）
    status: Option<String>,
    /// create（分析模式）：挂载父节点（goal 根须为空且全链唯一根）
    parent: Option<String>,
    /// update / archive：节点 id
    id: Option<String>,
    /// update：append / replace_body
    mode: Option<String>,
    /// update：追加或替换的内容
    content: Option<String>,
    /// update：乐观锁（read_node 取得的 updated；不匹配 CONFLICT 不落盘）
    expected_updated: Option<String>,
    /// link / unlink：父节点 id
    from: Option<String>,
    /// link / unlink：子节点 id
    to: Option<String>,
    /// link：contains / solves / alternative
    rel: Option<String>,
    /// link：可选边说明
    desc: Option<String>,
    /// archive：归档原因
    reason: Option<String>,
}

/// 3.2.0 S1/S3/S4：候选方向（多遍关注产生的每一种"可能的读法"）
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
struct CandidateParams {
    /// 方向名（语义描述，如"提炼为方案节点"）
    dir: String,
    /// 权重（LLM 序数分；校准前不进概率）
    score: f64,
    /// 该方向若被抽中要执行的节点意图（仅 sample 模式有效；commit 模式下必须为空）
    commits: Option<Vec<CommitParams>>,
}

/// 3.2.0 S2：伏笔登记条目（潜在痕迹，不进图谱）
#[derive(Debug, Clone, Deserialize, schemars::JsonSchema)]
struct ForeshadowParams {
    /// 覆盖的消息 seq 区间（可空）
    covers: Option<Vec<u64>>,
    /// 一句摘录 + 为什么登记（方向未定）
    note: String,
}

/// 三层重构：remember 事件参数（kind 区分 msg / tool / decision）
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct RememberParams {
    /// 会话 id（工作区持续账本内区分谁在说；仅字母/数字/-/_/.，不得以 . 开头）
    session: String,
    /// 追加的事件类别：msg / tool / decision（None = 本次不追加事件；但有 commits 时必须有事件）
    kind: Option<String>,
    /// msg：角色（user / assistant）
    role: Option<String>,
    /// msg：消息文本
    text: Option<String>,
    /// tool：工具名
    name: Option<String>,
    /// tool：参数摘要
    args: Option<String>,
    /// tool：结果摘要
    result: Option<String>,
    /// decision：keep / skip / revise / foreshadow（foreshadow = 伏笔登记：方向未定的潜在痕迹）
    decided: Option<String>,
    /// decision：覆盖的消息 seq 区间 [from, to]（含端点，两元素数组）
    covers: Option<Vec<u64>>,
    /// decision：本次决策落到的节点 id（可空数组）
    nodes: Option<Vec<String>>,
    /// decision：决策理由（必填；空串拒绝——否则无法区分有意跳过与遗忘）
    reason: Option<String>,
    /// decision：提交模式 commit（默认，argmax 承诺）/ sample（保留式抽取，需 seed + candidates）
    mode: Option<String>,
    /// decision：sample 模式的抽取种子（复盘可重放；同种子同结果）
    seed: Option<String>,
    /// decision：全部候选方向 + 权重（多峰时每峰各建节点；分布全量留痕）
    candidates: Option<Vec<CandidateParams>>,
    /// decision：sample 模式下 AI 声明的抽中方向（与工具按种子的确定性抽取不一致时拒绝）
    selected: Option<String>,
    /// decision：伏笔登记条目（decided=foreshadow 时为主体内容；keep 时也可附记）
    foreshadowing: Option<Vec<ForeshadowParams>>,
    /// 落节点意图列表（可选；sample 模式下放 candidates[].commits）
    commits: Option<Vec<CommitParams>>,
}

/// 三层重构 P2：冻结自愈（治理权转移后的裁决出口）
#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ResolveConflictParams {
    /// 冻结节点 id（read_node 里 frozen=true 的节点）
    id: String,
    /// 裁决后的标题（单行非空；通常去掉 [待裁决] 前缀）
    title: String,
    /// 裁决后的状态：pending / in_progress / success / failed / blocked / none
    status: String,
    /// 裁决后的正文（分析模式必填非空；缺省 = 保持冻结前正文）
    body: Option<String>,
    /// 乐观锁：read_node 取得的 updated（不符返回 CONFLICT，不改变冻结态）
    expected_updated: Option<String>,
}

// ── 工具实现（协议映射层，逻辑全在 engram_core::ops）────────────

/// CommitParams → core CommitIntent（顶层 commits 与 sample 候选 commits 共用）
fn parse_commits(cs: &[CommitParams]) -> Result<Vec<mcp::CommitIntent>, ErrorData> {
    let mut out = Vec::with_capacity(cs.len());
    for c in cs {
        let intent = match c.op.as_str() {
            "create" => mcp::CommitIntent::Create {
                title: c.title.clone().unwrap_or_default(),
                body: c.body.clone(),
                tags: c.tags.clone(),
                force: c.force,
                node_type: c.node_type.clone(),
                status: c.status.clone(),
                parent: c.parent.clone(),
            },
            "update" => mcp::CommitIntent::Update {
                id: c.id.clone().unwrap_or_default(),
                mode: c.mode.clone().unwrap_or_default(),
                content: c.content.clone().unwrap_or_default(),
                expected_updated: c.expected_updated.clone(),
                status: c.status.clone(),
            },
            "link" => mcp::CommitIntent::Link {
                from: c.from.clone().unwrap_or_default(),
                to: c.to.clone().unwrap_or_default(),
                rel: c.rel.clone().unwrap_or_default(),
                desc: c.desc.clone(),
            },
            "unlink" => mcp::CommitIntent::Unlink {
                from: c.from.clone().unwrap_or_default(),
                to: c.to.clone().unwrap_or_default(),
            },
            "archive" => mcp::CommitIntent::Archive {
                id: c.id.clone().unwrap_or_default(),
                reason: c.reason.clone(),
            },
            other => {
                return Err(ErrorData::internal_error(
                    format!(
                        "REMEMBER_BAD_OP: op 仅 create/update/link/unlink/archive，收到「{other}」"
                    ),
                    None,
                ))
            }
        };
        out.push(intent);
    }
    Ok(out)
}

#[tool_router]
impl EngramMcp {
    fn new(ctx: Workspace) -> Self {
        Self {
            ctx: Arc::new(ctx),
            write_lock: Arc::new(tokio::sync::Mutex::new(())),
            tool_router: Self::tool_router(),
        }
    }

    #[tool(
        description = "获取图谱全局概览：节点/边规模、活跃链摘要、健康度计数、工作区模式与指南版本。会话开始或迷失方向时调用。返回 JSON 文本。"
    )]
    async fn get_overview(&self) -> Result<CallToolResult, ErrorData> {
        to_result(mcp::get_overview(&self.ctx), self.ctx.guide_version())
    }

    #[tool(
        description = "按关键词检索节点（title/tags/body 子串匹配，大小写不敏感），按相关度排序。返回 JSON 文本（id/title/snippet），需要全文再 read_node。"
    )]
    async fn search(
        &self,
        Parameters(p): Parameters<SearchParams>,
    ) -> Result<CallToolResult, ErrorData> {
        to_result(mcp::search(&self.ctx, &p.query, p.limit), self.ctx.guide_version())
    }

    #[tool(
        description = "读取单个节点完整内容与元数据（含乐观锁用的 updated 字段）。include_neighbors=true 时附父/子节点列表；include_code_map=true 时附代码骨架（code_map_md/code_map_stale，未挂载为 null）。返回 JSON 文本。"
    )]
    async fn read_node(
        &self,
        Parameters(p): Parameters<ReadNodeParams>,
    ) -> Result<CallToolResult, ErrorData> {
        to_result(mcp::read_node_full(&self.ctx, &p.id, p.include_neighbors, p.include_code_map), self.ctx.guide_version())
    }

    #[tool(
        description = "以某节点为中心无向扩展 1-2 层关系网，返回局部子图（nodes+edges 摘要）。depth 仅支持 1 或 2。返回 JSON 文本。"
    )]
    async fn expand(
        &self,
        Parameters(p): Parameters<ExpandParams>,
    ) -> Result<CallToolResult, ErrorData> {
        to_result(
            mcp::expand(&self.ctx, &p.id, p.depth, p.direction.as_deref()),
            self.ctx.guide_version(),
        )
    }

    #[tool(
        description = "查找两节点间最短关系路径，返回节点序列与关系叙述（narrative 字段，如 A --solves--> B）。不连通时 found=false。返回 JSON 文本。"
    )]
    async fn read_path(
        &self,
        Parameters(p): Parameters<ReadPathParams>,
    ) -> Result<CallToolResult, ErrorData> {
        to_result(mcp::read_path(&self.ctx, &p.from, &p.to), self.ctx.guide_version())
    }

    #[tool(
        description = "获取当前工作区模式的 AI 使用指南全文与版本号。任何写入操作前必须先调用本工具获取最新规范。返回 JSON 文本（content 字段为指南全文）。"
    )]
    async fn get_guide(&self) -> Result<CallToolResult, ErrorData> {
        to_result(mcp::get_guide(&self.ctx), self.ctx.guide_version())
    }

    #[tool(
        description = "语义召回记忆节点（记忆层 L2）：索引未建立或模型不可用时自动降级为关键词检索（degraded:true 显式声明），向量模式叠加使用强度加成并过滤归档。返回 JSON 文本。"
    )]
    async fn recall(
        &self,
        Parameters(p): Parameters<RecallParams>,
    ) -> Result<CallToolResult, ErrorData> {
        // recall 只读（stats 回写为内部派生状态，不受 D3 写锁约束）
        to_result(
            mcp::recall(
                &self.ctx,
                &p.query,
                p.k,
                p.include_archived.unwrap_or(false),
            ),
            self.ctx.guide_version(),
        )
    }

    #[tool(
        description = "蒸馏节点簇为 [蒸馏] 骨架节点（derived:true + 逐条来源引用，检索默认降权；开发模式为主、分析模式共享）。dry_run 默认 true（只出计划）；dry_run=false 真正创建骨架节点，原节点不删。无可蒸馏簇返回 CONSOLIDATE_EMPTY。返回 JSON 文本。"
    )]
    async fn consolidate(
        &self,
        Parameters(p): Parameters<ConsolidateParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let _guard = self.write_lock.lock().await;
        to_result(
            mcp::consolidate(&self.ctx, p.targets, p.dry_run, p.k),
            self.ctx.guide_version(),
        )
    }

    // ── 三层重构 P0/P1 新工具（设计稿 v1 §10）──────────────────────────────

    #[tool(
        description = "【记忆入口·核心】追加对话并可选地把 AI 整理出的脉络落成节点——三层重构后唯一的写入口。kind=msg 追加一条消息；kind=tool 追加工具轨迹；kind=decision 追加记忆决策留痕（decided=keep/skip/revise/foreshadow，covers 为被本决策消费的消息 seq 区间，reason 必填——skip 也必须留痕）。3.2.0 新增：decided=foreshadow 做伏笔登记（foreshadowing 数组：细节方向未定的潜在痕迹，不进图谱）；candidates 记录全部候选方向+权重（多峰分布时每个方向各建节点，origin 带方向序号）；mode=sample 时按 seed 做确定性保留式抽取（AI 声明的 selected 必须与工具抽取一致），未抽中方向留在痕迹。commits 数组可选地执行节点意图（op=create/update/link/unlink/archive），全部过守门；正文缺「> 触发：」句只标记不阻断（conventions 字段）。有 commits 时必须有事件（溯源锚点）。返回 JSON 文本。"
    )]
    async fn remember(
        &self,
        Parameters(p): Parameters<RememberParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let _guard = self.write_lock.lock().await;
        let event = match p.kind.as_deref() {
            None => None,
            Some("msg") => Some(mcp::RememberEvent::Message {
                role: p.role.unwrap_or_default(),
                text: p.text.unwrap_or_default(),
            }),
            Some("tool") => Some(mcp::RememberEvent::Tool {
                name: p.name.unwrap_or_default(),
                args: p.args.unwrap_or_default(),
                result: p.result.unwrap_or_default(),
            }),
            Some("decision") => {
                let covers = match &p.covers {
                    Some(v) if v.len() == 2 => Some((v[0], v[1])),
                    None => None,
                    Some(_) => {
                        return Err(ErrorData::internal_error(
                            "REMEMBER_BAD_COVERS: covers 须为两元素数组 [from, to]（含端点）"
                                .to_string(),
                            None,
                        ))
                    }
                };
                let candidates = match p.candidates {
                    None => None,
                    Some(cs) => {
                        let mut out = Vec::with_capacity(cs.len());
                        for c in cs {
                            let commits = match c.commits {
                                None => Vec::new(),
                                Some(intents) => {
                                    parse_commits(&intents)?
                                }
                            };
                            out.push(mcp::CandidateSpec {
                                dir: c.dir,
                                score: c.score,
                                commits,
                            });
                        }
                        Some(out)
                    }
                };
                let foreshadowing = match p.foreshadowing {
                    None => None,
                    Some(fs) => {
                        let mut out = Vec::with_capacity(fs.len());
                        for f in fs {
                            let covers = match &f.covers {
                                Some(v) if v.len() == 2 => Some((v[0], v[1])),
                                None => None,
                                Some(_) => {
                                    return Err(ErrorData::internal_error(
                                        "REMEMBER_BAD_COVERS: foreshadowing.covers 须为两元素数组 [from, to]"
                                            .to_string(),
                                        None,
                                    ))
                                }
                            };
                            out.push(mcp::ForeshadowSpec {
                                covers,
                                note: f.note,
                            });
                        }
                        Some(out)
                    }
                };
                Some(mcp::RememberEvent::Decision {
                    decided: p.decided.unwrap_or_default(),
                    covers,
                    nodes: p.nodes.unwrap_or_default(),
                    reason: p.reason.unwrap_or_default(),
                    mode: p.mode,
                    seed: p.seed,
                    candidates,
                    selected: p.selected,
                    foreshadowing,
                })
            }
            Some(other) => {
                return Err(ErrorData::internal_error(
                    format!("REMEMBER_BAD_KIND: kind 仅 msg/tool/decision，收到「{other}」"),
                    None,
                ))
            }
        };
        let commits: Option<Vec<mcp::CommitIntent>> = match p.commits {
            None => None,
            Some(cs) => Some(parse_commits(&cs)?),
        };
        to_result(
            mcp::remember(&self.ctx, &p.session, event, commits),
            self.ctx.guide_version(),
        )
    }

    #[tool(
        description = "【只读】对话账本状态：规模（records）、会话与指南版本、消费进度（unconsumed_from 之后的记录是本次接管要读的部分）、决策计数（keep/skip/revise/foreshadow）、伏笔登记聚合视图（foreshadowing）、保留式抽取统计（sample_decisions）、坏行清单（malformed，需修复）。新 AI 接管工作区时先调它。返回 JSON 文本。"
    )]
    async fn dialogue_status(&self) -> Result<CallToolResult, ErrorData> {
        to_result(mcp::dialogue_status(&self.ctx), self.ctx.guide_version())
    }

    #[tool(
        description = "【写入】冻结自愈：并发写冲突后节点进入 [待裁决] 冻结态（frozen=true），用本工具读双方内容后写回最终裁决（title/status/body 一次给定），去除冻结标记。非冻结节点调用报 NOT_FROZEN；expected_updated 不符报 CONFLICT（不改变冻结态）。返回 JSON 文本。"
    )]
    async fn resolve_conflict(
        &self,
        Parameters(p): Parameters<ResolveConflictParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let _guard = self.write_lock.lock().await;
        to_result(
            mcp::resolve_conflict(
                &self.ctx,
                &p.id,
                &p.title,
                &p.status,
                p.body.as_deref(),
                p.expected_updated.as_deref(),
            ),
            self.ctx.guide_version(),
        )
    }
}

// D4：握手下发指南版本与写入规范（initialize 响应 instructions 字段）
#[tool_handler]
impl ServerHandler for EngramMcp {
    fn get_info(&self) -> ServerInfo {
        let mut si = Implementation::from_build_env();
        si.name = "engram-mcp".into();
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(si)
            .with_instructions(format!(
                "Engram MCP server（工作区：{}；模式：{}；AI 指南 v{}）。三层重构后**记忆的唯一写入口是 remember**（对话账本 + 节点意图 + 决策留痕；取代已移除的 create_node/update_node/link_nodes/archive_node/unlink_nodes）。任何写入前必须先 get_guide 获取最新规范；update 意图建议先 read_node 取 updated 并传 expected_updated 防并发覆盖（CONFLICT 冲突会触发 [待裁决] 冻结）。每个工具响应携带 guide_version，版本变化当次可见。",
                self.ctx.root.display(),
                self.ctx.mode_str(),
                self.ctx.guide_version(),
            ))
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut workspace: Option<PathBuf> = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        // 四版本矩阵 + git 短哈希（ADR 0011）：安装版/开发版漂移验证锚点
        if a == "--version" || a == "-V" {
            println!(
                "{}",
                engram_core::version::VersionInfo::new(env!("CARGO_PKG_VERSION")).display_line()
            );
            return Ok(());
        }
        if a == "--workspace" || a == "-w" {
            workspace = args.next().map(PathBuf::from);
        }
    }
    let Some(root) = workspace else {
        eprintln!("engram-mcp：缺少 --workspace <工作区目录>");
        eprintln!("用法：engram-mcp --workspace G:\\path\\to\\workspace");
        std::process::exit(2);
    };
    let ctx = Workspace::open(root).unwrap_or_else(|e| {
        eprintln!("engram-mcp 启动失败：{e}");
        std::process::exit(1);
    });
    eprintln!(
        "engram-mcp 已启动：workspace={} mode={} guide=v{}",
        ctx.root.display(),
        ctx.mode_str(),
        ctx.guide_version(),
    );

    // v2.20 后台预热嵌入模型：recall 首调不再同步加载模型（外部实测 120s 挂死修复）
    engram_core::embed::warm_up_embedder();

    let server = EngramMcp::new(ctx);
    let service = server.serve(stdio()).await.context("stdio 传输启动失败")?;
    service.waiting().await.context("服务运行异常")?;
    Ok(())
}
