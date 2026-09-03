//! engram-mcp — Engram 工作区的 MCP server（stdio 传输）。
//! M1 spike：验证 rmcp SDK 在本工程可用——initialize 握手 / tools/list / tools/call 全链路。
//! 本阶段仅提供 ping 探活工具，不触碰图谱读写（M2 抽取共享 lib 后接入）。

use rmcp::{
    handler::server::wrapper::Parameters, schemars, tool, tool_router, transport::stdio,
    ServiceExt,
};

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
struct PingParams {
    /// 可选回显文本
    text: Option<String>,
}

#[derive(Clone)]
struct EngramMcp;

#[tool_router(server_handler)]
impl EngramMcp {
    #[tool(description = "连通性检查：返回 pong（携带 text 时原样回显）")]
    fn ping(&self, Parameters(PingParams { text }): Parameters<PingParams>) -> String {
        match text {
            Some(t) => format!("pong: {t}"),
            None => "pong".to_string(),
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // stdio 传输：MCP 客户端（Claude Desktop / dsh 等）以子进程方式拉起本 server
    let service = EngramMcp.serve(stdio()).await?;
    service.waiting().await?;
    Ok(())
}
