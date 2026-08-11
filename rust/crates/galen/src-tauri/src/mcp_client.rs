//! MCP (Model Context Protocol) client for Galen.
//!
//! Implements JSON-RPC 2.0 over stdio transport. Manages server lifecycle:
//! spawn → initialize → discover tools → execute.
//!
//! Connection state machine: Disconnected → Connecting → Connected → Error
//! AuthRequired is reserved for OAuth-based servers (future).

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::sync::Mutex;
use tokio::time::{timeout, Duration};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum McpError {
    Spawn(String),
    Io(String),
    Timeout(String),
    Protocol(i64, String),
    Deserialize(String),
    NotFound(String),
}

impl std::fmt::Display for McpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Spawn(msg) => write!(f, "MCP spawn: {msg}"),
            Self::Io(msg) => write!(f, "MCP I/O: {msg}"),
            Self::Timeout(msg) => write!(f, "MCP timeout: {msg}"),
            Self::Protocol(code, msg) => write!(f, "MCP error [{code}]: {msg}"),
            Self::Deserialize(msg) => write!(f, "MCP deserialize: {msg}"),
            Self::NotFound(msg) => write!(f, "MCP not found: {msg}"),
        }
    }
}

impl std::error::Error for McpError {}

impl From<McpError> for String {
    fn from(e: McpError) -> Self {
        e.to_string()
    }
}

// ---------------------------------------------------------------------------
// Connection status
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum McpConnectionStatus {
    Disconnected,
    Connecting,
    Connected,
    AuthRequired,
    Error,
}

impl std::fmt::Display for McpConnectionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disconnected => write!(f, "disconnected"),
            Self::Connecting => write!(f, "connecting"),
            Self::Connected => write!(f, "connected"),
            Self::AuthRequired => write!(f, "auth_required"),
            Self::Error => write!(f, "error"),
        }
    }
}

// ---------------------------------------------------------------------------
// JSON-RPC types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct JsonRpcResponse {
    id: u64,
    #[serde(default)]
    result: Option<Value>,
    #[serde(default)]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

#[derive(Debug, Clone, Deserialize)]
struct InitializeResult {
    protocol_version: String,
    #[allow(dead_code)]
    server_info: Value,
    capabilities: Value,
}

#[derive(Debug, Clone, Deserialize)]
struct ListToolsResult {
    tools: Vec<McpTool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpTool {
    pub name: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub input_schema: Option<Value>,
}

#[derive(Debug, Clone, Deserialize)]
struct CallToolResult {
    content: Vec<McpContent>,
    #[allow(dead_code)]
    is_error: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
struct McpContent {
    #[serde(rename = "type")]
    content_type: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    resource: Option<Value>,
}

// ---------------------------------------------------------------------------
// McpServer
// ---------------------------------------------------------------------------

/// A connected MCP server instance with lifecycle tracking.
pub struct McpServer {
    pub name: String,
    pub status: McpConnectionStatus,
    stdin: Arc<Mutex<ChildStdin>>,
    stdout: Arc<Mutex<BufReader<ChildStdout>>>,
    _child: Option<Child>,
    next_id: Arc<Mutex<u64>>,
    tools: Vec<McpTool>,
}

impl Drop for McpServer {
    fn drop(&mut self) {
        if let Some(mut child) = self._child.take() {
            let _ = child.start_kill();
            let _ = child.wait();
        }
    }
}

impl McpServer {
    /// Spawn an MCP server process and complete the initialize handshake.
    pub async fn connect(name: &str, command: &str, args: &[String]) -> Result<Self, McpError> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        // Hide console windows on Windows (CREATE_NO_WINDOW)
        #[cfg(windows)]
        {
            #[allow(unused_imports)]
            use std::os::windows::process::CommandExt;
            cmd.creation_flags(0x08000000);
        }
        let mut child = cmd
            .spawn()
            .map_err(|e| McpError::Spawn(format!("{name}: {e}")))?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| McpError::Io(format!("{name}: no stdin")))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| McpError::Io(format!("{name}: no stdout")))?;

        let stdin = Arc::new(Mutex::new(stdin));
        let stdout = Arc::new(Mutex::new(BufReader::new(stdout)));
        let next_id = Arc::new(Mutex::new(1u64));

        let mut server = Self {
            name: name.to_string(),
            status: McpConnectionStatus::Connecting,
            stdin,
            stdout,
            _child: Some(child),
            next_id,
            tools: Vec::new(),
        };

        // Initialize handshake
        let _init: InitializeResult = server
            .call("initialize", Some(serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": {},
                "clientInfo": { "name": "galen", "version": "0.1.0" }
            })))
            .await?;

        server.send_notification("notifications/initialized", None).await?;

        // Discover tools
        let tools_result: ListToolsResult =
            server.call("tools/list", None).await.unwrap_or(ListToolsResult { tools: vec![] });

        server.tools = tools_result.tools;
        server.status = McpConnectionStatus::Connected;
        Ok(server)
    }

    /// Call an MCP method and deserialize the result.
    async fn call<R: serde::de::DeserializeOwned>(
        &mut self,
        method: &str,
        params: Option<Value>,
    ) -> Result<R, McpError> {
        let id = {
            let mut guard = self.next_id.lock().await;
            let id = *guard;
            *guard += 1;
            id
        };

        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };

        let request_json =
            serde_json::to_string(&request).map_err(|e| McpError::Io(format!("serialize: {e}")))?;

        // Write
        {
            let mut stdin = self.stdin.lock().await;
            stdin
                .write_all(request_json.as_bytes())
                .await
                .map_err(|e| McpError::Io(format!("write: {e}")))?;
            stdin
                .write_u8(b'\n')
                .await
                .map_err(|e| McpError::Io(format!("write newline: {e}")))?;
        }

        // Read with timeout
        let name = self.name.clone();
        let response_json = timeout(Duration::from_secs(30), async {
            let mut stdout = self.stdout.lock().await;
            let mut line = String::new();
            stdout
                .read_line(&mut line)
                .await
                .map_err(|e| McpError::Io(format!("{name} read: {e}")))?;
            Ok::<String, McpError>(line)
        })
        .await
        .map_err(|_| McpError::Timeout(format!("{name}: request timed out")))??;

        // Parse
        let response: JsonRpcResponse =
            serde_json::from_str(&response_json).map_err(|e| McpError::Deserialize(format!("{name}: {e}")))?;

        if let Some(error) = response.error {
            self.status = McpConnectionStatus::Error;
            return Err(McpError::Protocol(error.code, format!("{}: {}", self.name, error.message)));
        }

        let result = response
            .result
            .ok_or_else(|| McpError::Protocol(-1, format!("{}: empty result", self.name)))?;

        serde_json::from_value(result).map_err(|e| McpError::Deserialize(format!("{name}: {e}")))
    }

    /// Send a JSON-RPC notification (no response expected).
    async fn send_notification(&mut self, method: &str, params: Option<Value>) -> Result<(), McpError> {
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });
        let text = serde_json::to_string(&notification).map_err(|e| McpError::Io(format!("serialize: {e}")))?;
        let mut stdin = self.stdin.lock().await;
        stdin
            .write_all(text.as_bytes())
            .await
            .map_err(|e| McpError::Io(format!("write: {e}")))?;
        stdin
            .write_u8(b'\n')
            .await
            .map_err(|e| McpError::Io(format!("write newline: {e}")))?;
        Ok(())
    }

    /// Call a tool by name and return the text result.
    pub async fn call_tool(&mut self, tool_name: &str, arguments: Value) -> Result<String, McpError> {
        let result: CallToolResult = self
            .call("tools/call", Some(serde_json::json!({
                "name": tool_name,
                "arguments": arguments,
            })))
            .await?;

        let text = result
            .content
            .iter()
            .filter_map(|c| c.text.as_deref())
            .collect::<Vec<_>>()
            .join("\n");

        if text.is_empty() {
            Ok("(tool returned no text content)".to_string())
        } else {
            Ok(text)
        }
    }

    /// Return a copy of the discovered tools.
    pub fn tools(&self) -> Vec<McpTool> {
        self.tools.clone()
    }
}

// ---------------------------------------------------------------------------
// McpServerRegistry — manages multiple server connections
// ---------------------------------------------------------------------------

/// Tracks multiple MCP server connections with status per server.
pub struct McpServerRegistry {
    servers: Vec<Arc<Mutex<McpServer>>>,
}

impl McpServerRegistry {
    pub fn new() -> Self {
        Self { servers: Vec::new() }
    }

    pub fn from_servers(servers: Vec<McpServer>) -> Self {
        Self {
            servers: servers.into_iter().map(|s| Arc::new(Mutex::new(s))).collect(),
        }
    }

    /// Create a registry from already-connected (cached) server handles.
    pub fn from_cached(servers: Vec<Arc<Mutex<McpServer>>>) -> Self {
        Self { servers }
    }

    pub fn is_empty(&self) -> bool {
        self.servers.is_empty()
    }

    pub fn len(&self) -> usize {
        self.servers.len()
    }

    pub fn servers(&self) -> &[Arc<Mutex<McpServer>>] {
        &self.servers
    }

    /// Get status of all servers.
    pub async fn statuses(&self) -> Vec<McpServerStatus> {
        let mut result = Vec::new();
        for server in &self.servers {
            let s = server.lock().await;
            result.push(McpServerStatus {
                name: s.name.clone(),
                connected: s.status == McpConnectionStatus::Connected,
                status: s.status,
                tool_count: s.tools.len(),
            });
        }
        result
    }

    /// Execute a tool on an MCP server by qualified name.
    pub async fn call_tool(
        &self,
        server_name: &str,
        tool_name: &str,
        arguments: Value,
    ) -> Result<String, McpError> {
        for server in &self.servers {
            let s = server.lock().await;
            if s.name == server_name {
                if s.status != McpConnectionStatus::Connected {
                    return Err(McpError::Protocol(
                        -1,
                        format!("server '{server_name}' is not connected (status: {})", s.status),
                    ));
                }
                if !s.tools.iter().any(|t| t.name == tool_name) {
                    return Err(McpError::NotFound(format!(
                        "tool '{tool_name}' not found on server '{server_name}'"
                    )));
                }
                drop(s);
                let mut s = server.lock().await;
                return s.call_tool(tool_name, arguments).await;
            }
        }
        Err(McpError::NotFound(format!("server '{server_name}' not found")))
    }

    /// Get all tool definitions from all connected MCP servers.
    pub fn mcp_tool_definitions(&self) -> Vec<McpTool> {
        self.servers
            .iter()
            .flat_map(|s| {
                s.try_lock()
                    .map(|guard| guard.tools())
                    .unwrap_or_default()
            })
            .collect()
    }
}

impl Default for McpServerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Server status (for frontend display)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct McpServerStatus {
    pub name: String,
    pub connected: bool,
    pub status: McpConnectionStatus,
    pub tool_count: usize,
}

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    pub command: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct McpConfig {
    #[serde(default)]
    pub mcp_servers: HashMap<String, McpServerConfig>,
}

impl McpConfig {
    pub fn load() -> Option<Self> {
        let path = dirs::config_dir()?.join("galen").join("mcp_servers.json");
        let text = std::fs::read_to_string(&path).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn write_default() -> Option<Self> {
        let dir = dirs::config_dir()?.join("galen");
        if let Err(e) = std::fs::create_dir_all(&dir) {
            eprintln!("[galen] mcp: create_dir_all failed: {e}");
            return None;
        }
        let path = dir.join("mcp_servers.json");
        if path.exists() {
            return Self::load();
        }

        fn deno_cmd() -> String {
            crate::tools::resolve_binary("deno")
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "deno".to_string())
        }
        fn uvx_cmd() -> String {
            crate::tools::resolve_binary("uv")
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|| "uvx".to_string())
        }
        fn deno_run(pkg: &str) -> Vec<String> {
            vec!["run".to_string(), "--allow-all".to_string(), pkg.to_string()]
        }

        let config = Self {
            mcp_servers: HashMap::from([
                ("fetch".to_string(), McpServerConfig {
                    command: uvx_cmd(),
                    args: vec!["mcp-server-fetch".to_string()],
                    enabled: false,
                }),
                ("memory".to_string(), McpServerConfig {
                    command: deno_cmd(),
                    args: deno_run("npm:@modelcontextprotocol/server-memory"),
                    enabled: false,
                }),
                ("sequential-thinking".to_string(), McpServerConfig {
                    command: deno_cmd(),
                    args: deno_run("npm:@modelcontextprotocol/server-sequential-thinking"),
                    enabled: false,
                }),
                ("filesystem".to_string(), McpServerConfig {
                    command: deno_cmd(),
                    args: deno_run("npm:@modelcontextprotocol/server-filesystem"),
                    enabled: false,
                }),
                ("git".to_string(), McpServerConfig {
                    command: uvx_cmd(),
                    args: vec!["mcp-server-git".to_string(), "--repository".to_string(), ".".to_string()],
                    enabled: false,
                }),
            ]),
        };
        let text = serde_json::to_string_pretty(&config).ok()?;
        if let Err(e) = std::fs::write(&path, text) {
            eprintln!("[galen] mcp: write_default failed: {e}");
        }
        Some(config)
    }
}

/// Discover and connect to all configured, enabled MCP servers.
pub async fn connect_configured_servers() -> McpServerRegistry {
    let config = McpConfig::load().unwrap_or_default();
    let mut servers = Vec::new();

    for (name, server_config) in &config.mcp_servers {
        if !server_config.enabled {
            continue;
        }
        match McpServer::connect(name, &server_config.command, &server_config.args).await {
            Ok(server) => servers.push(server),
            Err(e) => eprintln!("MCP {name}: {e}"),
        }
    }

    McpServerRegistry::from_servers(servers)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_error_display() {
        let e = McpError::Spawn("test: spawn failed".into());
        assert!(e.to_string().contains("MCP spawn: test: spawn failed"));

        let e = McpError::Protocol(42, "bad request".into());
        assert!(e.to_string().contains("MCP error [42]: bad request"));

        let e = McpError::NotFound("server 'x' not found".into());
        assert!(e.to_string().contains("MCP not found: server 'x' not found"));
    }

    #[test]
    fn mcp_error_into_string() {
        let e = McpError::Timeout("test timeout".into());
        let s: String = e.into();
        assert_eq!(s, "MCP timeout: test timeout");
    }

    #[test]
    fn connection_status_display_all_variants() {
        let cases = [
            (McpConnectionStatus::Disconnected, "disconnected"),
            (McpConnectionStatus::Connecting, "connecting"),
            (McpConnectionStatus::Connected, "connected"),
            (McpConnectionStatus::AuthRequired, "auth_required"),
            (McpConnectionStatus::Error, "error"),
        ];
        for (status, expected) in cases {
            assert_eq!(status.to_string(), expected);
        }
    }

    #[test]
    fn connection_status_serde_roundtrip() {
        let status = McpConnectionStatus::Connected;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, r#""connected""#);
        let back: McpConnectionStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(back, McpConnectionStatus::Connected);
    }

    #[test]
    fn mcp_config_default_is_empty() {
        let config = McpConfig::default();
        assert!(config.mcp_servers.is_empty());
    }

    #[test]
    fn registry_starts_empty() {
        let reg = McpServerRegistry::new();
        assert!(reg.is_empty());
        assert_eq!(reg.len(), 0);
        assert!(reg.servers().is_empty());
    }

    #[test]
    fn unknown_binary_returns_none() {
        assert!(crate::tools::resolve_binary("nonexistent_binary_xyz_123").is_none());
    }
}
