use super::{lock_mutex, AppState};
use tauri::State;

#[derive(serde::Serialize)]
pub struct MemoryStatus {
    pub exists: bool,
    pub size: u64,
    pub preview: String,
}

#[tauri::command]
pub fn get_memory_status(state: State<AppState>) -> Result<MemoryStatus, String> {
    let backend = lock_mutex(&state.backend)?;
    let root = match backend.get_workspace_root() {
        Some(root) => root,
        None => {
            return Ok(MemoryStatus {
                exists: false,
                size: 0,
                preview: String::new(),
            })
        }
    };
    let path = root.join("GALEN.md");
    match std::fs::metadata(&path) {
        Ok(meta) => {
            let content = std::fs::read_to_string(&path).unwrap_or_default();
            let preview = content.chars().take(500).collect();
            Ok(MemoryStatus {
                exists: true,
                size: meta.len(),
                preview,
            })
        }
        Err(_) => Ok(MemoryStatus {
            exists: false,
            size: 0,
            preview: String::new(),
        }),
    }
}
