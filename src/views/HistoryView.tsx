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
    // versions is sorted newest-first; slot 0 = older, slot 1 = newer
    if (versions.length >= 2) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setSelectedVersions([versions[1].id, versions[0].id]);
    } else if (versions.length === 1) {
      setSelectedVersions([null, versions[0].id]);
    } else {
      setSelectedVersions([null, null]);
    }
  }, [versions]);

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
