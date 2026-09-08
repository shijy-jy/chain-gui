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

/// 核心层 Result 映射为 MCP 响应：Ok → JSON 文本；Err → 协议级错误（isError）
fn to_result(r: Result<Value, String>) -> Result<CallToolResult, ErrorData> {
    match r {
        Ok(v) => Ok(CallToolResult::success(vec![ContentBlock::text(
            serde_json::to_string_pretty(&v).unwrap_or_else(|_| v.to_string()),
        )])),
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
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ExpandParams {
    /// 中心节点 id
    id: String,
    /// 扩展层数：仅 1 或 2（默认 1）
    depth: Option<u32>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct ReadPathParams {
    /// 起点节点 id
    from: String,
    /// 终点节点 id
    to: String,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct CreateNodeParams {
    /// 节点标题（必填，单行；同名检测命中时需 force=true 另建）
    title: String,
    /// 正文（可选，缺省为 "# 标题" 占位）
    body: Option<String>,
    /// 标签列表（可选）
    tags: Option<Vec<String>>,
    /// true = 跳过同名拦截强制另建
    force: Option<bool>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct UpdateNodeParams {
    /// 节点 id
    id: String,
    /// append = 正文末尾追加；replace_body = 整体替换正文
    mode: String,
    /// 追加或替换的内容
    content: String,
    /// 乐观锁：read_node 取得的 updated 值；不匹配则 CONFLICT 不落盘
    expected_updated: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct LinkNodesParams {
    /// 父节点 id
    from: String,
    /// 子节点 id
    to: String,
    /// 关系类型：contains / solves / alternative
    rel_type: String,
    /// 可选边说明（写入子节点 rel_desc；空串=清除，不传=不动）
    desc: Option<String>,
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
struct ArchiveNodeParams {
    /// 节点 id
    id: String,
    /// 归档原因（可选，写入 frontmatter archived_reason）
    reason: Option<String>,
}

#[derive(Debug, Deserialize, schemars::JsonSchema)]
struct UnlinkNodesParams {
    /// 父节点 id
    from: String,
    /// 子节点 id
    to: String,
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

// ── 工具实现（协议映射层，逻辑全在 engram_core::ops）────────────

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
        to_result(mcp::get_overview(&self.ctx))
    }

    #[tool(
        description = "按关键词检索节点（title/tags/body 子串匹配，大小写不敏感），按相关度排序。返回 JSON 文本（id/title/snippet），需要全文再 read_node。"
    )]
    async fn search(
        &self,
        Parameters(p): Parameters<SearchParams>,
    ) -> Result<CallToolResult, ErrorData> {
        to_result(mcp::search(&self.ctx, &p.query, p.limit))
    }

    #[tool(
        description = "读取单个节点完整内容与元数据（含乐观锁用的 updated 字段）。include_neighbors=true 时附父/子节点列表。返回 JSON 文本。"
    )]
    async fn read_node(
        &self,
        Parameters(p): Parameters<ReadNodeParams>,
    ) -> Result<CallToolResult, ErrorData> {
        to_result(mcp::read_node(&self.ctx, &p.id, p.include_neighbors))
    }

    #[tool(
        description = "以某节点为中心无向扩展 1-2 层关系网，返回局部子图（nodes+edges 摘要）。depth 仅支持 1 或 2。返回 JSON 文本。"
    )]
    async fn expand(
        &self,
        Parameters(p): Parameters<ExpandParams>,
    ) -> Result<CallToolResult, ErrorData> {
        to_result(mcp::expand(&self.ctx, &p.id, p.depth))
    }

    #[tool(
        description = "查找两节点间最短关系路径，返回节点序列与关系叙述（narrative 字段，如 A --solves--> B）。不连通时 found=false。返回 JSON 文本。"
    )]
    async fn read_path(
        &self,
        Parameters(p): Parameters<ReadPathParams>,
    ) -> Result<CallToolResult, ErrorData> {
        to_result(mcp::read_path(&self.ctx, &p.from, &p.to))
    }

    #[tool(
        description = "获取当前工作区模式的 AI 使用指南全文与版本号。任何写入操作前必须先调用本工具获取最新规范。返回 JSON 文本（content 字段为指南全文）。"
    )]
    async fn get_guide(&self) -> Result<CallToolResult, ErrorData> {
        to_result(mcp::get_guide(&self.ctx))
    }

    #[tool(
        description = "【写入】新建知识节点（仅开发模式工作区）。title 必填且单行；检测到同名节点时拒绝，确认另建传 force=true。写入前请先 get_guide。返回 JSON 文本（含新 id 与规范提示）。"
    )]
    async fn create_node(
        &self,
        Parameters(p): Parameters<CreateNodeParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let _guard = self.write_lock.lock().await;
        to_result(mcp::create_node(
            &self.ctx,
            &p.title,
            p.body.as_deref(),
            p.tags,
            p.force,
        ))
    }

    #[tool(
        description = "【写入】更新节点正文：mode=append 追加 / replace_body 整体替换。建议先 read_node 取 updated 并传 expected_updated（乐观锁，不匹配返回 CONFLICT 不落盘）。写入前请先 get_guide。返回 JSON 文本。"
    )]
    async fn update_node(
        &self,
        Parameters(p): Parameters<UpdateNodeParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let _guard = self.write_lock.lock().await;
        to_result(mcp::update_node(
            &self.ctx,
            &p.id,
            &p.mode,
            &p.content,
            p.expected_updated.as_deref(),
        ))
    }

    #[tool(
        description = "【写入】建立 from(父)→to(子) 链接（仅开发模式工作区）。rel_type 仅 contains/solves/alternative；desc 可选边说明。写入前请先 get_guide。返回 JSON 文本。"
    )]
    async fn link_nodes(
        &self,
        Parameters(p): Parameters<LinkNodesParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let _guard = self.write_lock.lock().await;
        to_result(mcp::link_nodes(
            &self.ctx,
            &p.from,
            &p.to,
            &p.rel_type,
            p.desc.as_deref(),
        ))
    }

    #[tool(
        description = "语义召回记忆节点（记忆层 L2）：索引未建立或模型不可用时自动降级为关键词检索（degraded:true 显式声明），向量模式叠加使用强度加成并过滤归档。返回 JSON 文本。"
    )]
    async fn recall(
        &self,
        Parameters(p): Parameters<RecallParams>,
    ) -> Result<CallToolResult, ErrorData> {
        // recall 只读（stats 回写为内部派生状态，不受 D3 写锁约束）
        to_result(mcp::recall(
            &self.ctx,
            &p.query,
            p.k,
            p.include_archived.unwrap_or(false),
        ))
    }

    #[tool(
        description = "【写入】归档节点（仅开发模式工作区）：archived: true + 标题前缀 [归档]，文件移入 .chain/archive/。归档节点默认不进图与检索，recall 传 include_archived=true 可找回，read_node 仍可直读。90 天未触达为建议阈值（仅提示）。写入前请先 get_guide。返回 JSON 文本。"
    )]
    async fn archive_node(
        &self,
        Parameters(p): Parameters<ArchiveNodeParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let _guard = self.write_lock.lock().await;
        to_result(mcp::archive_node(&self.ctx, &p.id, p.reason.as_deref()))
    }

    #[tool(
        description = "【写入】断开 from(父)→to(子) 链接（仅开发模式工作区）：子节点 parent 置 null 并清理 rel/rel_desc。返回 rel_removed 供回溯。写入前请先 get_guide。返回 JSON 文本。"
    )]
    async fn unlink_nodes(
        &self,
        Parameters(p): Parameters<UnlinkNodesParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let _guard = self.write_lock.lock().await;
        to_result(mcp::unlink_nodes(&self.ctx, &p.from, &p.to))
    }

    #[tool(
        description = "蒸馏节点簇为 [蒸馏] 骨架节点（derived:true + 逐条来源引用，检索默认降权；开发模式为主、分析模式共享）。dry_run 默认 true（只出计划）；dry_run=false 真正创建骨架节点，原节点不删。无可蒸馏簇返回 CONSOLIDATE_EMPTY。返回 JSON 文本。"
    )]
    async fn consolidate(
        &self,
        Parameters(p): Parameters<ConsolidateParams>,
    ) -> Result<CallToolResult, ErrorData> {
        let _guard = self.write_lock.lock().await;
        to_result(mcp::consolidate(&self.ctx, p.targets, p.dry_run, p.k))
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
                "Engram MCP server（工作区：{}；模式：{}；AI 指南 v{}）。写入类工具（create_node/update_node/link_nodes/archive_node/unlink_nodes/consolidate）调用前必须先 get_guide 获取最新规范；update_node 建议先 read_node 取 updated 并传 expected_updated 防并发覆盖（CONFLICT 冲突会触发 [待裁决] 冻结，绝不静默覆盖）。",
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

    let server = EngramMcp::new(ctx);
    let service = server.serve(stdio()).await.context("stdio 传输启动失败")?;
    service.waiting().await.context("服务运行异常")?;
    Ok(())
}
