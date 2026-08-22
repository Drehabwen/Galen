use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::Mutex;

/// Resolve a relative path against the workspace root with one boundary used
/// by tools, Tauri commands and Artifact preview.
pub fn resolve_workspace_path(
    workspace_root: &Mutex<Option<PathBuf>>,
    rel: &str,
) -> Result<PathBuf, String> {
    let guard = workspace_root
        .lock()
        .map_err(|e| format!("Workspace lock error: {e}"))?;
    let root = guard.as_ref().ok_or_else(|| {
        "未选择工作区。请点击顶部的「选择工作区」，打开一个项目目录。".to_string()
    })?;
    resolve_workspace_path_from_root(root, rel)
}

pub fn resolve_workspace_path_from_root(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let canonical_root = fs::canonicalize(root)
        .map_err(|e| format!("Cannot resolve workspace root {}: {e}", root.display()))?;
    if !canonical_root.is_dir() {
        return Err("Workspace root is not a directory".to_string());
    }

    let relative = normalize_relative(rel)?;
    let candidate = canonical_root.join(relative);

    if candidate.exists() {
        let canonical_candidate = fs::canonicalize(&candidate)
            .map_err(|e| format!("Cannot resolve workspace path: {e}"))?;
        if canonical_candidate.starts_with(&canonical_root) {
            return Ok(canonical_candidate);
        }
        return Err("Access denied: path is outside workspace".to_string());
    }

    // For a write target that does not exist yet, canonicalize its nearest
    // existing ancestor. This catches a parent symlink that points outside.
    let mut ancestor = candidate.as_path();
    while !ancestor.exists() {
        ancestor = ancestor
            .parent()
            .ok_or("Access denied: path has no workspace ancestor")?;
    }
    let canonical_ancestor = fs::canonicalize(ancestor)
        .map_err(|e| format!("Cannot resolve workspace path ancestor: {e}"))?;
    if !canonical_ancestor.starts_with(&canonical_root) {
        return Err("Access denied: path is outside workspace".to_string());
    }
    Ok(candidate)
}

fn normalize_relative(rel: &str) -> Result<PathBuf, String> {
    let mut normalized = PathBuf::new();
    for component in Path::new(rel).components() {
        match component {
            Component::Normal(part) => normalized.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err("Access denied: path is outside workspace".to_string());
                }
            }
            Component::Prefix(_) | Component::RootDir => {
                return Err("Access denied: absolute paths are not allowed".to_string());
            }
        }
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "galen-workspace-path-{}-{}",
            std::process::id(),
            tag
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("data")).unwrap();
        std::fs::write(root.join("data/sample.csv"), "id,value\n1,2\n").unwrap();
        root
    }

    #[test]
    fn resolves_existing_and_new_paths_inside_workspace() {
        let root = workspace("inside");
        let existing = resolve_workspace_path_from_root(&root, "data/sample.csv").unwrap();
        assert!(existing.ends_with("data/sample.csv"));
        let new_file = resolve_workspace_path_from_root(&root, "data/output.csv").unwrap();
        assert!(new_file.ends_with("data/output.csv"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_parent_traversal_and_absolute_paths() {
        let root = workspace("escape");
        assert!(resolve_workspace_path_from_root(&root, "../outside.txt").is_err());
        assert!(resolve_workspace_path_from_root(&root, "data/../../outside.txt").is_err());
        let absolute = std::env::temp_dir().join("outside.txt");
        assert!(
            resolve_workspace_path_from_root(&root, absolute.to_string_lossy().as_ref()).is_err()
        );
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn does_not_confuse_workspace_with_same_prefix_sibling() {
        let root = workspace("prefix");
        let sibling = root.with_file_name(format!(
            "{}-sibling",
            root.file_name().unwrap().to_string_lossy()
        ));
        std::fs::create_dir_all(&sibling).unwrap();
        std::fs::write(sibling.join("secret.txt"), "secret").unwrap();
        let relative = format!(
            "../{}/secret.txt",
            sibling.file_name().unwrap().to_string_lossy()
        );
        assert!(resolve_workspace_path_from_root(&root, &relative).is_err());
        let _ = std::fs::remove_dir_all(root);
        let _ = std::fs::remove_dir_all(sibling);
    }
}
