use std::process::Command;
use std::sync::Arc;
use tauri::State;

use crate::core::{central_repo, skill_store::SkillStore};

#[tauri::command]
pub fn get_settings(key: String, store: State<'_, Arc<SkillStore>>) -> Result<Option<String>, String> {
    store.get_setting(&key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_settings(
    key: String,
    value: String,
    store: State<'_, Arc<SkillStore>>,
) -> Result<(), String> {
    store.set_setting(&key, &value).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_central_repo_path() -> String {
    central_repo::base_dir().to_string_lossy().to_string()
}

#[tauri::command]
pub fn open_central_repo_folder() -> Result<(), String> {
    let repo_path = central_repo::base_dir();

    #[cfg(target_os = "macos")]
    let mut cmd = Command::new("open");
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = Command::new("explorer");
        use std::os::windows::process::CommandExt;
        c.creation_flags(0x08000000); // CREATE_NO_WINDOW
        c
    };
    #[cfg(target_os = "linux")]
    let mut cmd = Command::new("xdg-open");

    let status = cmd
        .arg(&repo_path)
        .status()
        .map_err(|e| format!("Failed to open folder: {e}"))?;

    // Windows explorer.exe returns exit code 1 even on success
    #[cfg(not(target_os = "windows"))]
    if !status.success() {
        return Err(format!("File manager exited with status: {status}"));
    }

    let _ = status;
    Ok(())
}
