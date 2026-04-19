use serde::Serialize;
use std::sync::Arc;
use tauri::State;

use crate::core::{
    diff::{compute_diff, DiffHunk},
    error::AppError,
    skill_store::SkillStore,
    version_store::{VersionContent, VersionRecord},
};

#[derive(Debug, Clone, Serialize)]
pub struct SkillHistorySummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub source_type: String,
    pub source_ref: Option<String>,
    pub source_ref_resolved: Option<String>,
    pub content_hash: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub version_count: i64,
    pub latest_captured_at: Option<i64>,
}

#[tauri::command]
pub async fn list_skills_with_history(
    state: State<'_, Arc<SkillStore>>,
) -> Result<Vec<SkillHistorySummary>, AppError> {
    let store = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<Vec<SkillHistorySummary>, AppError> {
        let skills = store.get_all_skills().map_err(AppError::db)?;
        let summary_map = store.version_summary_map().map_err(AppError::db)?;
        let mut out: Vec<SkillHistorySummary> = skills
            .into_iter()
            .filter(|s| s.source_type != "native")
            .map(|s| {
                let (version_count, latest_captured_at) = match summary_map.get(&s.id) {
                    Some((c, t)) => (*c, Some(*t)),
                    None => (0, None),
                };
                SkillHistorySummary {
                    id: s.id,
                    name: s.name,
                    description: s.description,
                    source_type: s.source_type,
                    source_ref: s.source_ref,
                    source_ref_resolved: s.source_ref_resolved,
                    content_hash: s.content_hash,
                    created_at: s.created_at,
                    updated_at: s.updated_at,
                    version_count,
                    latest_captured_at,
                }
            })
            .collect();
        // Most recently captured first, ties by name
        out.sort_by(|a, b| {
            b.latest_captured_at
                .cmp(&a.latest_captured_at)
                .then_with(|| a.name.cmp(&b.name))
        });
        Ok(out)
    })
    .await?
}

#[tauri::command]
pub async fn list_versions(
    store: State<'_, Arc<SkillStore>>,
    skill_id: String,
) -> Result<Vec<VersionRecord>, AppError> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        store.list_versions(&skill_id).map_err(AppError::db)
    })
    .await?
}

#[tauri::command]
pub async fn get_version(
    store: State<'_, Arc<SkillStore>>,
    version_id: String,
) -> Result<VersionContent, AppError> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        store.get_version(&version_id).map_err(AppError::db)
    })
    .await?
}

#[tauri::command]
pub async fn diff_versions(
    store: State<'_, Arc<SkillStore>>,
    old_id: String,
    new_id: String,
) -> Result<Vec<DiffHunk>, AppError> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let old = store.get_version(&old_id).map_err(AppError::db)?;
        let new = store.get_version(&new_id).map_err(AppError::db)?;
        if old.record.skill_id != new.record.skill_id {
            return Err(AppError::invalid_input(
                "version IDs must belong to the same skill",
            ));
        }
        Ok(compute_diff(&old.content, &new.content, 3))
    })
    .await?
}

#[derive(Debug, Clone, Serialize)]
pub struct RestoreResult {
    pub skill_id: String,
    pub new_version_no: Option<i64>,
    pub no_op: bool,
    pub message: String,
}

#[tauri::command]
pub async fn restore_version(
    state: State<'_, Arc<SkillStore>>,
    version_id: String,
) -> Result<RestoreResult, AppError> {
    let store = state.inner().clone();
    tauri::async_runtime::spawn_blocking(move || -> Result<RestoreResult, AppError> {
        let target = store.get_version(&version_id).map_err(AppError::db)?;

        // Short-circuit if target is already the latest version
        if let Some(latest) = store
            .latest_version(&target.record.skill_id)
            .map_err(AppError::db)?
        {
            if latest.id == version_id {
                return Ok(RestoreResult {
                    skill_id: target.record.skill_id,
                    new_version_no: None,
                    no_op: true,
                    message: "target is already the latest version".to_string(),
                });
            }
        }

        // Resolve central_path and write the SKILL.md
        let skill = store
            .get_skill_by_id(&target.record.skill_id)
            .map_err(AppError::db)?
            .ok_or_else(|| AppError::not_found("skill not found"))?;
        let skill_md = std::path::Path::new(&skill.central_path).join("SKILL.md");

        std::fs::write(&skill_md, &target.content).map_err(|e| {
            AppError::io(format!("failed to write {}: {e}", skill_md.display()))
        })?;

        // Capture new version with trigger=restore
        store
            .restore_version(&version_id)
            .map_err(AppError::db)?;

        // Fetch the newly-captured version's number
        let new_latest = store
            .latest_version(&target.record.skill_id)
            .map_err(AppError::db)?;
        let new_version_no = new_latest.as_ref().map(|v| v.version_no);

        // Trigger re-sync of active scenario so all agents receive restored content
        if let Err(err) = crate::commands::scenarios::sync_current_scenario_internal(&store) {
            log::warn!("scenario sync after restore failed: {err}");
        }

        Ok(RestoreResult {
            skill_id: target.record.skill_id,
            new_version_no,
            no_op: false,
            message: "restored".to_string(),
        })
    })
    .await?
}
