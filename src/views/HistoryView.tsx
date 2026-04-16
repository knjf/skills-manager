import { useEffect, useState } from "react";
import { listSkillsWithHistory, listVersions } from "../lib/tauri";
import type { SkillHistorySummary, VersionRecord } from "../types/history";
import { SkillListPane } from "./history/SkillListPane";

export function HistoryView() {
  const [skills, setSkills] = useState<SkillHistorySummary[]>([]);
  const [selectedSkillId, setSelectedSkillId] = useState<string | null>(null);
  const [versions, setVersions] = useState<VersionRecord[]>([]);
  const [loadingSkills, setLoadingSkills] = useState(true);
  const [loadingVersions, setLoadingVersions] = useState(false);

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

  const selectedSkill = skills.find((s) => s.id === selectedSkillId) ?? null;

  return (
    <div className="flex h-full">
      <SkillListPane
        skills={skills}
        loading={loadingSkills}
        selectedId={selectedSkillId}
        onSelect={setSelectedSkillId}
      />
      <div className="flex-1 flex flex-col p-4 overflow-hidden">
        {!selectedSkill ? (
          <div className="text-muted">Select a skill to view its history.</div>
        ) : loadingVersions ? (
          <div className="text-muted">Loading versions…</div>
        ) : (
          <>
            <div className="text-lg font-semibold text-primary">{selectedSkill.name}</div>
            <div className="text-sm text-muted mb-2">
              {versions.length} versions · source: {selectedSkill.source_type}
            </div>
            {versions.length === 0 && (
              <div className="text-muted">
                No history recorded for this skill yet.
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
