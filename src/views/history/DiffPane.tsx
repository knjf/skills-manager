import { useEffect, useState } from "react";
import { Diff, Hunk, parseDiff } from "react-diff-view";
import type { FileData, HunkData } from "react-diff-view";
import "react-diff-view/style/index.css";

import { diffVersions, getVersion } from "../../lib/tauri";
import type { DiffHunk, VersionContent } from "../../types/history";

interface Props {
  oldVersionId: string;
  newVersionId: string;
}

function toUnifiedText(
  hunks: DiffHunk[],
  oldName: string,
  newName: string,
): string {
  const header = `diff --git a/${oldName} b/${oldName}\n--- a/${oldName}\n+++ b/${newName}\n`;
  const body = hunks
    .map((h) => {
      const lines = h.lines
        .map((l) => {
          const prefix =
            l.kind === "Added" ? "+" : l.kind === "Removed" ? "-" : " ";
          // Strip trailing \n — the patch line format already represents end-of-line.
          const content = l.text.replace(/\n$/, "");
          return `${prefix}${content}`;
        })
        .join("\n");
      return `${h.header}\n${lines}`;
    })
    .join("\n");
  return `${header}${body}\n`;
}

export function DiffPane({ oldVersionId, newVersionId }: Props) {
  const [oldV, setOldV] = useState<VersionContent | null>(null);
  const [newV, setNewV] = useState<VersionContent | null>(null);
  const [hunks, setHunks] = useState<DiffHunk[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    // eslint-disable-next-line react-hooks/set-state-in-effect
    setLoading(true);
    setError(null);
    Promise.all([
      getVersion(oldVersionId),
      getVersion(newVersionId),
      diffVersions(oldVersionId, newVersionId),
    ])
      .then(([o, n, h]) => {
        if (cancelled) return;
        setOldV(o);
        setNewV(n);
        setHunks(h);
      })
      .catch((err) => {
        if (!cancelled) {
          console.error("DiffPane fetch failed", err);
          setError(String(err));
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [oldVersionId, newVersionId]);

  if (loading)
    return <div className="p-4 text-sm text-muted">Loading diff…</div>;
  if (error)
    return (
      <div className="p-4 text-sm text-danger">
        Failed to load diff: {error}
      </div>
    );
  if (!oldV || !newV) return null;
  if (hunks.length === 0)
    return (
      <div className="p-4 text-sm text-muted">Versions are identical.</div>
    );

  const unified = toUnifiedText(
    hunks,
    `v${oldV.record.version_no}`,
    `v${newV.record.version_no}`,
  );
  const files: FileData[] = parseDiff(unified);

  return (
    <div className="flex-1 overflow-auto p-2 text-xs">
      <div className="mb-2 text-sm text-muted">
        Comparing v{oldV.record.version_no} → v{newV.record.version_no}
      </div>
      {files.map((file, i) => (
        <Diff
          key={i}
          viewType="split"
          diffType={file.type}
          hunks={file.hunks}
        >
          {(renderedHunks: HunkData[]) =>
            renderedHunks.map((h) => <Hunk key={h.content} hunk={h} />)
          }
        </Diff>
      ))}
    </div>
  );
}
