use serde::Serialize;
use std::path::PathBuf;

use crate::tools::resolve_binary;

// ---------------------------------------------------------------------------
// Runtime environment detection
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeStatus {
    pub python: RuntimeInfo,
    pub r: RuntimeInfo,
    pub typst: RuntimeInfo,
    pub deno: RuntimeInfo,
    pub uvx: RuntimeInfo,
}

pub use crate::mcp_client::McpServerStatus;

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeInfo {
    pub installed: bool,
    pub version: Option<String>,
    pub path: Option<String>,
    pub install_guide: Option<&'static str>,
}

impl RuntimeInfo {
    fn missing(install_guide: &'static str) -> Self {
        Self {
            installed: false,
            version: None,
            path: None,
            install_guide: Some(install_guide),
        }
    }

    fn found(path: PathBuf, version: Option<String>) -> Self {
        Self {
            installed: true,
            version,
            path: Some(path.to_string_lossy().to_string()),
            install_guide: None,
        }
    }
}

/// Get version string by running `<binary> --version` and parsing the first line.
fn get_version(binary: &str) -> Option<String> {
    std::process::Command::new(binary)
        .arg("--version")
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                let text = String::from_utf8_lossy(&o.stdout);
                text.lines().next().map(|l| l.trim().to_string())
            } else {
                None
            }
        })
}

fn detect_python() -> RuntimeInfo {
    // 1. Check bundled Python (shipped with Galen, no user install needed)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let bundled = dir.join("binaries").join("python").join("python.exe");
            if bundled.exists() {
                let version = get_version(&bundled.to_string_lossy());
                return RuntimeInfo::found(bundled, version);
            }
        }
    }
    // 2. Check system PATH
    for name in &["python", "python3", "py"] {
        if let Some(path) = resolve_binary(name) {
            let version = get_version(name);
            return RuntimeInfo::found(path, version);
        }
    }
    RuntimeInfo::missing(
        "Python 已随 Galen 打包，如缺失请重新安装 Galen。"
    )
}

fn detect_r() -> RuntimeInfo {
    // 1. Check bundled R (optional sidecar)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let bundled = dir.join("binaries").join("R").join("bin").join("R.exe");
            if bundled.exists() {
                let version = get_version(&bundled.to_string_lossy());
                return RuntimeInfo::found(bundled, version);
            }
        }
    }
    // 2. Check system PATH
    if let Some(path) = resolve_binary("R") {
        let version = get_version("R");
        return RuntimeInfo::found(path, version);
    }
    RuntimeInfo::missing(
        "R 可随 Galen 打包 (binaries/R/)，也可从 https://cran.r-project.org 自行安装。"
    )
}

fn detect_typst() -> RuntimeInfo {
    if let Ok(path) = crate::tools::resolve_typst() {
        let version = get_version(
            &path.to_string_lossy()
        );
        return RuntimeInfo::found(path, version);
    }
    RuntimeInfo::missing(
        "请运行 `cargo install typst-cli` 安装 Typst，\n\
         或从 https://github.com/typst/typst/releases 下载后放在 Galen 同目录下。"
    )
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

pub fn detect_all() -> RuntimeStatus {
    RuntimeStatus {
        python: detect_python(),
        r: detect_r(),
        typst: detect_typst(),
        deno: detect_runtime("deno", "Deno", "Deno 随 Galen 打包，如缺失请重新安装"),
        uvx: detect_runtime("uvx", "uv", "uv 已随 Galen 打包，如缺失请重新安装 Galen。"),
    }
}

fn detect_runtime(binary: &str, _label: &str, guide: &'static str) -> RuntimeInfo {
    if let Some(path) = resolve_binary(binary) {
        let version = get_version(binary);
        RuntimeInfo::found(path, version)
    } else {
        RuntimeInfo::missing(guide)
    }
}

/// Prettify the runtime status into a human-readable summary (for the system prompt / status bar).
pub fn status_summary(status: &RuntimeStatus) -> String {
    let mut lines = vec!["## 科研环境状态".to_string()];
    
    lines.push(format!(
        "- Python: {}",
        status_line(&status.python)
    ));
    lines.push(format!(
        "- R:      {}",
        status_line(&status.r)
    ));
    lines.push(format!(
        "- Typst:  {}",
        status_line(&status.typst)
    ));
    
    lines.join("\n")
}

/// Connect to configured MCP servers and report their status (non-blocking, best-effort).
pub async fn detect_mcp_servers() -> Vec<McpServerStatus> {
    let registry = crate::mcp_client::connect_configured_servers().await;
    registry.statuses().await
}

fn status_line(info: &RuntimeInfo) -> String {
    if info.installed {
        match &info.version {
            Some(v) => format!("✅ {}", v),
            None => "✅ 已安装".to_string(),
        }
    } else {
        "❌ 未安装".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_no_crash() {
        // Just ensure detection doesn't panic
        let status = detect_all();
        // At minimum, we get a status struct back
        assert!(!status.python.installed || status.python.version.is_some() || status.python.path.is_some());
    }

    #[test]
    fn test_status_line() {
        let info = RuntimeInfo::found(PathBuf::from("/usr/bin/python3"), Some("Python 3.12.0".into()));
        let line = status_line(&info);
        assert!(line.contains("✅"));
        assert!(line.contains("3.12.0"));
    }

    #[test]
    fn test_status_summary_format() {
        let status = detect_all();
        let summary = status_summary(&status);
        assert!(summary.contains("Python"));
        assert!(summary.contains("R"));
        assert!(summary.contains("Typst"));
    }
}
