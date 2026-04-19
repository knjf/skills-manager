import { useTranslation } from "react-i18next";
import type { SkillHistorySummary } from "../../types/history";

interface Props {
  skill: SkillHistorySummary;
}

function formatMs(ts: number | null | undefined): string {
  if (!ts) return "—";
  return new Date(ts).toLocaleString();
}

function formatSec(ts: number | null | undefined): string {
  if (!ts) return "—";
  return new Date(ts * 1000).toLocaleString();
}

export function MetadataPanel({ skill }: Props) {
  const { t } = useTranslation();

  return (
    <div className="border-b border-border-subtle p-3">
      <div className="flex items-center gap-2 mb-1">
        <span className="font-semibold text-base text-primary">{skill.name}</span>
        <span className="text-xs uppercase px-2 py-0.5 bg-surface rounded text-muted">
          {skill.source_type}
        </span>
      </div>
      {skill.source_ref && (
        <div className="text-xs text-faint truncate mb-1">{skill.source_ref}</div>
      )}
      {skill.description && (
        <div className="text-sm text-muted mb-1">{skill.description}</div>
      )}
      <div className="flex gap-4 text-xs text-muted">
        <span>{t("history.metadata.versions", { count: skill.version_count })}</span>
        <span>{t("history.metadata.imported")}: {formatMs(skill.created_at)}</span>
        <span>{t("history.metadata.lastUpdate")}: {formatMs(skill.updated_at)}</span>
        {skill.latest_captured_at && (
          <span>{t("history.metadata.lastCapture")}: {formatSec(skill.latest_captured_at)}</span>
        )}
      </div>
      {skill.content_hash && (
        <div className="text-xs text-faint font-mono mt-1">
          {t("history.metadata.hash")}: {skill.content_hash.slice(0, 16)}…
        </div>
      )}
    </div>
  );
}
