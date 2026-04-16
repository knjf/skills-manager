import { useEffect, useState } from "react";
import { listSkillsWithHistory, listVersions } from "../lib/tauri";
import type { SkillHistorySummary, VersionRecord } from "../types/history";
import { SkillListPane } from "./history/SkillListPane";
import { MetadataPanel } from "./history/MetadataPanel";
import { VersionListPane } from "./history/VersionListPane";

export function HistoryView() {
  const [skills, setSkills] = useState<SkillHistorySummary[]>([]);
  const [selectedSkillId, setSelectedSkillId] = useState<string | null>(null);
  const [versions, setVersions] = useState<VersionRecord[]>([]);
  const [loadingSkills, setLoadingSkills] = useState(true);
  const [loadingVersions, setLoadingVersions] = useState(false);
  const [selectedVersions, setSelectedVersions] = useState<[string | null, string | null]>([null, null]);

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
    const next: [string | null, string | null] =
      versions.length >= 2
        ? [versions[1].id, versions[0].id]
        : versions.length === 1
          ? [null, versions[0].id]
          : [null, null];
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setSelectedVersions(next);
  }, [versions]);

  const toggleVersion = (id: string) => {
    setSelectedVersions((prev) => {
      if (prev[0] === id) return [null, prev[1]];
      if (prev[1] === id) return [prev[0], null];
      if (prev[0] === null) return [id, prev[1]];
      if (prev[1] === null) return [prev[0], id];
      // Both slots full — evict older, shift newer to older, place new
      return [prev[1], id];
    });
  };

  const selectedSkill = skills.find((s) => s.id === selectedSkillId) ?? null;

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
          <div className="p-4 text-sm text-muted">Select a skill to view its history.</div>
        ) : loadingVersions ? (
          <div className="p-4 text-sm text-muted">Loading versions…</div>
        ) : (
          <>
            <MetadataPanel skill={selectedSkill} />
            <VersionListPane
              versions={versions}
              selectedIds={selectedVersions}
              onToggle={toggleVersion}
            />
            <div className="p-4 text-sm text-muted">
              {selectedVersions[0] && selectedVersions[1]
                ? "Diff goes here (Task 14)"
                : versions.length >= 2
                  ? "Select two versions to compare."
                  : "Only one version exists — nothing to compare."}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
