export interface SkillHistorySummary {
  id: string;
  name: string;
  description: string | null;
  source_type: string;
  source_ref: string | null;
  source_ref_resolved: string | null;
  content_hash: string | null;
  created_at: number;
  updated_at: number;
  version_count: number;
  latest_captured_at: number | null;
}

export type CaptureTriggerName = "scan" | "import" | "backfill" | "restore";

export interface VersionRecord {
  id: string;
  skill_id: string;
  version_no: number;
  content_hash: string;
  byte_size: number;
  captured_at: number;
  trigger: CaptureTriggerName;
  source_type: string;
  source_ref: string | null;
  source_ref_resolved: string | null;
}

export interface VersionContent {
  record: VersionRecord;
  content: string;
}

// Serde default (no rename_all): exact Rust variant names — capitalized.
export type DiffLineKind = "Context" | "Added" | "Removed";

export interface DiffLine {
  kind: DiffLineKind;
  old_no: number | null;
  new_no: number | null;
  text: string;
}

export interface DiffHunk {
  header: string;
  lines: DiffLine[];
}

export interface RestoreResult {
  skill_id: string;
  new_version_no: number | null;
  no_op: boolean;
  message: string;
}
