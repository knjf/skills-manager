import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { listSkillsWithHistory, listVersions, restoreVersion } from "../lib/tauri";
import type { SkillHistorySummary, VersionRecord } from "../types/history";
import { SkillListPane } from "./history/SkillListPane";
import { MetadataPanel } from "./history/MetadataPanel";
import { VersionListPane } from "./history/VersionListPane";
import { DiffPane } from "./history/DiffPane";
import { ConfirmDialog } from "../components/ConfirmDialog";

export function HistoryView() {
  const { t } = useTranslation();
  const [skills, setSkills] = useState<SkillHistorySummary[]>([]);
  const [selectedSkillId, setSelectedSkillId] = useState<string | null>(null);
  const [versions, setVersions] = useState<VersionRecord[]>([]);
  const [loadingSkills, setLoadingSkills] = useState(true);
  const [loadingVersions, setLoadingVersions] = useState(false);
  const [selectedVersions, setSelectedVersions] = useState<[string | null, string | null]>([null, null]);
  const [restoreTarget, setRestoreTarget] = useState<VersionRecord | null>(null);
  const [restoreError, setRestoreError] = useState<string | null>(null);

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setLoadingSkills(true);
    listSkillsWithHistory()
      .then(setSkills)
      .catch((e) => console.error("listSkillsWithHistory failed", e))
      .finally(() => setLoadingSkills(false));
  }, []);

  useEffect(() => {
    if (!selectedSkillId) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setVersions([]);
      return;
    }
    setLoadingVersions(true);
    listVersions(selectedSkillId)
      .then(setVersions)
      .catch((e) => console.error("listVersions failed", e))
      .finally(() => setLoadingVersions(false));
  }, [selectedSkillId]);

  useEffect(() => {
    // versions is sorted newest-first; slot 0 = older, slot 1 = newer
    /* eslint-disable react-hooks/set-state-in-effect */
    if (versions.length >= 2) {
      setSelectedVersions([versions[1].id, versions[0].id]);
    } else if (versions.length === 1) {
      setSelectedVersions([null, versions[0].id]);
    } else {
      setSelectedVersions([null, null]);
    }
    /* eslint-enable react-hooks/set-state-in-effect */
  }, [versions]);

  useEffect(() => {
    let refreshTimer: ReturnType<typeof setTimeout> | null = null;
    const unlisten = listen("app-files-changed", () => {
      if (refreshTimer) clearTimeout(refreshTimer);
      refreshTimer = setTimeout(() => {
        listSkillsWithHistory()
          .then(setSkills)
          .catch((e) => console.error("refresh skills failed", e));
        if (selectedSkillId) {
          listVersions(selectedSkillId)
            .then(setVersions)
            .catch((e) => console.error("refresh versions failed", e));
        }
      }, 500);
    });
    return () => {
      if (refreshTimer) clearTimeout(refreshTimer);
      unlisten.then((fn) => fn()).catch(() => {});
    };
  }, [selectedSkillId]);

  const toggleVersion = (id: string) => {
    setSelectedVersions((prev) => {
      let next: [string | null, string | null];
      if (prev[0] === id) next = [null, prev[1]];
      else if (prev[1] === id) next = [prev[0], null];
      else if (prev[0] === null) next = [id, prev[1]];
      else if (prev[1] === null) next = [prev[0], id];
      else next = [prev[1], id]; // evict oldest slot

      // Enforce [older, newer] by version_no
      const [a, b] = next;
      if (a && b) {
        const va = versions.find((v) => v.id === a)?.version_no ?? 0;
        const vb = versions.find((v) => v.id === b)?.version_no ?? 0;
        if (va > vb) return [b, a];
      }
      return next;
    });
  };

  const selectedSkill = skills.find((s) => s.id === selectedSkillId) ?? null;

  // Exactly one version selected and it's not the latest
  const singleSelectedVersion = (() => {
    if (selectedVersions[0] && !selectedVersions[1]) {
      return versions.find((v) => v.id === selectedVersions[0]) ?? null;
    }
    if (!selectedVersions[0] && selectedVersions[1]) {
      return versions.find((v) => v.id === selectedVersions[1]) ?? null;
    }
    return null;
  })();
  const latestVersion = versions[0];
  const canRestore =
    singleSelectedVersion != null &&
    latestVersion != null &&
    singleSelectedVersion.id !== latestVersion.id;

  return (
    <div className="flex h-full">
      <SkillListPane
        skills={skills}
        loading={loadingSkills}
        selectedId={selectedSkillId}
        onSelect={setSelectedSkillId}
      />
      <div className="flex-1 flex flex-col overflow-hidden">
        {!selectedSkill ? (
          <div className="p-4 text-sm text-muted">{t("history.selectSkill")}</div>
        ) : loadingVersions ? (
          <div className="p-4 text-sm text-muted">{t("history.loadingVersions")}</div>
        ) : (
          <>
            <MetadataPanel skill={selectedSkill} />
            <VersionListPane
              versions={versions}
              selectedIds={selectedVersions}
              onToggle={toggleVersion}
            />
            <div className="px-3 py-2 border-b border-border-subtle flex items-center justify-between gap-2">
              {restoreError && (
                <span className="text-xs text-danger">{restoreError}</span>
              )}
              <button
                type="button"
                disabled={!canRestore}
                onClick={() => singleSelectedVersion && setRestoreTarget(singleSelectedVersion)}
                className="ml-auto px-3 py-1 rounded border border-border-subtle text-sm disabled:opacity-40"
              >
                {t("history.restore.button")}
              </button>
            </div>
            {selectedVersions[0] && selectedVersions[1] ? (
              <DiffPane
                oldVersionId={selectedVersions[0]}
                newVersionId={selectedVersions[1]}
              />
            ) : (
              <div className="p-4 text-sm text-muted">
                {versions.length >= 2
                  ? t("history.selectTwo")
                  : versions.length === 1
                    ? t("history.oneVersionOnly")
                    : ""}
              </div>
            )}
          </>
        )}
      </div>
      <ConfirmDialog
        open={restoreTarget !== null}
        title={restoreTarget ? t("history.restore.title", { version: restoreTarget.version_no }) : ""}
        message={t("history.restore.message")}
        tone="warning"
        confirmLabel={t("history.restore.confirm")}
        onClose={() => {
          setRestoreTarget(null);
          setRestoreError(null);
        }}
        onConfirm={async () => {
          if (!restoreTarget) return;
          setRestoreError(null);
          try {
            await restoreVersion(restoreTarget.id);
            if (selectedSkillId) {
              const next = await listVersions(selectedSkillId);
              setVersions(next);
            }
            setRestoreTarget(null);
          } catch (err) {
            console.error("restoreVersion failed", err);
            setRestoreError(String(err));
            throw err;
          }
        }}
      />
    </div>
  );
}
