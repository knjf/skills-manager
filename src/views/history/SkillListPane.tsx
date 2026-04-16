import { useMemo, useState } from "react";
import type { SkillHistorySummary } from "../../types/history";
import { cn } from "../../utils";

interface Props {
  skills: SkillHistorySummary[];
  loading: boolean;
  selectedId: string | null;
  onSelect: (id: string) => void;
}

export function SkillListPane({ skills, loading, selectedId, onSelect }: Props) {
  const [query, setQuery] = useState("");
  const filtered = useMemo(() => {
    const q = query.trim().toLowerCase();
    if (!q) return skills;
    return skills.filter((s) => s.name.toLowerCase().includes(q));
  }, [skills, query]);

  return (
    <div className="w-72 border-r border-border-subtle flex flex-col h-full bg-bg-secondary">
      <input
        type="text"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder="Search skills…"
        className="m-2 px-2 py-1 border border-border-subtle rounded text-sm bg-surface text-primary placeholder:text-faint focus:outline-none focus:border-accent"
      />
      {loading ? (
        <div className="px-3 py-2 text-sm text-muted">Loading…</div>
      ) : (
        <ul className="flex-1 overflow-y-auto scrollbar-hide">
          {filtered.map((s) => (
            <li
              key={s.id}
              onClick={() => onSelect(s.id)}
              className={cn(
                "px-3 py-2 cursor-pointer hover:bg-surface-hover",
                s.id === selectedId && "bg-surface-active"
              )}
            >
              <div className="font-medium text-sm text-primary">{s.name}</div>
              <div className="text-xs text-muted">
                {s.source_type} · {s.version_count} versions
              </div>
            </li>
          ))}
          {filtered.length === 0 && (
            <li className="px-3 py-2 text-sm text-faint">
              No skills match &ldquo;{query}&rdquo;.
            </li>
          )}
        </ul>
      )}
      <div className="text-xs text-faint p-2 border-t border-border-subtle">
        {skills.length} skills total
      </div>
    </div>
  );
}
