import type { VersionRecord } from "../../types/history";

interface Props {
  versions: VersionRecord[];
  selectedIds: [string | null, string | null]; // [older, newer]
  onToggle: (id: string) => void;
}

function relTime(tsSec: number): string {
  const now = Date.now() / 1000;
  const diff = now - tsSec;
  if (diff < 60) return "just now";
  if (diff < 3600) return `${Math.floor(diff / 60)}m ago`;
  if (diff < 86400) return `${Math.floor(diff / 3600)}h ago`;
  const days = Math.floor(diff / 86400);
  if (days < 30) return `${days}d ago`;
  return new Date(tsSec * 1000).toLocaleDateString();
}

export function VersionListPane({ versions, selectedIds, onToggle }: Props) {
  const isSelected = (id: string) =>
    selectedIds[0] === id || selectedIds[1] === id;

  if (versions.length === 0) {
    return (
      <div className="flex-1 p-4 text-sm text-muted">
        No history recorded for this skill yet.
      </div>
    );
  }

  return (
    <div className="flex-1 overflow-y-auto border-b border-border-subtle">
      <table className="w-full text-sm">
        <thead className="bg-bg-secondary sticky top-0">
          <tr className="text-left text-xs uppercase text-muted">
            <th className="w-10 px-2 py-2"></th>
            <th className="px-2 py-2">Version</th>
            <th className="px-2 py-2">Hash</th>
            <th className="px-2 py-2">Captured</th>
            <th className="px-2 py-2">Trigger</th>
          </tr>
        </thead>
        <tbody>
          {versions.map((v) => (
            <tr
              key={v.id}
              onClick={() => onToggle(v.id)}
              className={`border-t border-border-subtle cursor-pointer hover:bg-surface-hover ${
                isSelected(v.id) ? "bg-surface-active" : ""
              }`}
            >
              <td className="px-2 py-1.5 text-center">
                <input
                  type="checkbox"
                  checked={isSelected(v.id)}
                  onChange={() => onToggle(v.id)}
                  onClick={(e) => e.stopPropagation()}
                  aria-label={`Select version ${v.version_no}`}
                />
              </td>
              <td className="px-2 py-1.5 font-medium text-primary">
                v{v.version_no}
              </td>
              <td className="px-2 py-1.5 font-mono text-xs text-muted">
                {v.content_hash.slice(0, 8)}
              </td>
              <td className="px-2 py-1.5 text-muted">
                {relTime(v.captured_at)}
              </td>
              <td className="px-2 py-1.5">
                <span className="text-xs uppercase px-1.5 py-0.5 bg-surface rounded text-muted">
                  {v.trigger}
                </span>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  );
}
