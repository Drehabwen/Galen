use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

/// Resolve a relative path against the workspace root, with sandbox enforcement.
/// Ensures the resolved path does not escape the workspace via symlinks or `..` traversal.
pub fn resolve_workspace_path(
    workspace_root: &Mutex<Option<PathBuf>>,
    rel: &str,
) -> Result<PathBuf, String> {
    let guard = workspace_root
        .lock()
        .map_err(|e| format!("Workspace lock error: {e}"))?;
    let root = guard
        .as_ref()
        .ok_or_else(|| {
            "未选择工作区。请点击顶部的「选择工作区」，打开一个项目目录。".to_string()
        })?;
    let resolved = root.join(rel);
    // Ensure the resolved path is still within the workspace root
    let canonical =
        fs::canonicalize(&root).map_err(|e| format!("Cannot resolve workspace root: {e}"))?;
    match fs::canonicalize(&resolved) {
        Ok(p) if p.starts_with(&canonical) => Ok(resolved),
        Ok(_) => Err("Access denied: path is outside workspace".to_string()),
        Err(_) => {
            // Path doesn't exist yet (e.g., for writes). Walk components to resolve
            // `..` properly, then check if the resolved path is within the workspace.
            let root_str = canonical.to_string_lossy().to_string();
            let resolved_str = resolved.to_string_lossy().to_string();
            if resolved_str.starts_with(&root_str) {
                return Ok(resolved);
            }
            // Resolve `..` by walking components: skip up on `..`, push on normal parts
            let mut stack: Vec<&str> = Vec::new();
            for part in resolved.components() {
                match part.as_os_str().to_str() {
                    Some("..") => { stack.pop(); }
                    Some(".") | None => {}
                    Some(p) => { stack.push(p); }
                }
            }
            let resolved_normalized: PathBuf = stack.iter().collect();
            let full = root.join(resolved_normalized);
            let full_str = full.to_string_lossy().to_string();
            if full_str.starts_with(&root_str) {
                // Parent dirs must exist for writes to a non-existent path
                if let Some(parent) = full.parent() {
                    if !parent.exists() {
                        return Err("Cannot write outside workspace: parent directory does not exist".to_string());
                    }
                }
                Ok(full)
            } else {
                Err("Access denied: path is outside workspace".to_string())
            }
        }
    }
}
