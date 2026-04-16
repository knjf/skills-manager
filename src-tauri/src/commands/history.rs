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
    store: State<'_, Arc<SkillStore>>,
) -> Result<Vec<SkillHistorySummary>, AppError> {
    let store = store.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let skills = store.get_all_skills().map_err(AppError::db)?;
        let mut out = Vec::with_capacity(skills.len());
        for s in skills {
            let versions = store.list_versions(&s.id).map_err(AppError::db)?;
            let latest_captured_at = versions.first().map(|v| v.captured_at);
            out.push(SkillHistorySummary {
                id: s.id,
                name: s.name,
                description: s.description,
                source_type: s.source_type,
                source_ref: s.source_ref,
                source_ref_resolved: s.source_ref_resolved,
                content_hash: s.content_hash,
                created_at: s.created_at,
                updated_at: s.updated_at,
                version_count: versions.len() as i64,
                latest_captured_at,
            });
        }
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
