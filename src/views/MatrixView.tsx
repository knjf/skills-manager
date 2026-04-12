import { useCallback, useEffect, useMemo, useState } from "react";
import {
  ChevronRight,
  ChevronDown,
  Grid3X3,
  Loader2,
  Package,
  Check,
  X,
  ToggleLeft,
  ToggleRight,
} from "lucide-react";
import { toast } from "sonner";
import { cn } from "../utils";
import { useApp } from "../context/AppContext";
import { getErrorMessage } from "../lib/error";
import * as api from "../lib/tauri";
import type {
  PackRecord,
  PackSkillRecord,
  ToolInfo,
  SkillToolToggle,
} from "../lib/tauri";

// ── Types ──

interface PackWithSkills {
  pack: PackRecord;
  skills: PackSkillRecord[];
}

// ── Component ──

export function MatrixView() {
  const { activeScenario, tools, refreshManagedSkills } = useApp();

  const [packs, setPacks] = useState<PackWithSkills[]>([]);
  const [loading, setLoading] = useState(true);
  const [expandedPacks, setExpandedPacks] = useState<Set<string>>(new Set());
  const [togglingCell, setTogglingCell] = useState<string | null>(null);

  // Toggles loaded per skill (skill_id -> SkillToolToggle[])
  const [skillToggles, setSkillToggles] = useState<
    Record<string, SkillToolToggle[]>
  >({});

  // Only show installed + enabled tools as columns
  const columns = useMemo(
    () => tools.filter((t) => t.installed && t.enabled),
    [tools],
  );

  // ── Data loading ──

  const loadPacks = useCallback(async () => {
    setLoading(true);
    try {
      const allPacks = await api.getAllPacks();
      const packsWithSkills: PackWithSkills[] = await Promise.all(
        allPacks.map(async (pack) => {
          const skills = await api.getSkillsForPack(pack.id);
          return { pack, skills };
        }),
      );
      setPacks(packsWithSkills);
    } catch (error) {
      toast.error(getErrorMessage(error, "Failed to load packs"));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    loadPacks();
  }, [loadPacks]);

  // Load toggles for skills in expanded packs
  const loadSkillToggles = useCallback(
    async (skillIds: string[]) => {
      if (!activeScenario) return;
      const results: Record<string, SkillToolToggle[]> = {};
      await Promise.all(
        skillIds.map(async (skillId) => {
          try {
            const toggles = await api.getSkillToolToggles(
              skillId,
              activeScenario.id,
            );
            results[skillId] = toggles;
          } catch {
            // skill may not be in this scenario
          }
        }),
      );
      setSkillToggles((prev) => ({ ...prev, ...results }));
    },
    [activeScenario],
  );

  // When a pack is expanded, load its skill toggles
  useEffect(() => {
    const skillIds: string[] = [];
    for (const { pack, skills } of packs) {
      if (expandedPacks.has(pack.id)) {
        for (const skill of skills) {
          if (!skillToggles[skill.id]) {
            skillIds.push(skill.id);
          }
        }
      }
    }
    if (skillIds.length > 0) {
      loadSkillToggles(skillIds);
    }
  }, [expandedPacks, packs, loadSkillToggles, skillToggles]);

  // ── Helpers ──

  const togglePackExpand = (packId: string) => {
    setExpandedPacks((prev) => {
      const next = new Set(prev);
      if (next.has(packId)) next.delete(packId);
      else next.add(packId);
      return next;
    });
  };

  /** Calculate pack-level status for a tool column: all/some/none enabled */
  const getPackToolStatus = (
    packSkills: PackSkillRecord[],
    toolKey: string,
  ): "all" | "some" | "none" => {
    let enabled = 0;
    let total = 0;
    for (const skill of packSkills) {
      const toggles = skillToggles[skill.id];
      if (!toggles) continue;
      const toggle = toggles.find((t) => t.tool === toolKey);
      if (toggle) {
        total++;
        if (toggle.enabled) enabled++;
      }
    }
    if (total === 0) return "none";
    if (enabled === total) return "all";
    if (enabled > 0) return "some";
    return "none";
  };

  const handleToggleSkillTool = async (
    skillId: string,
    toolKey: string,
    enabled: boolean,
  ) => {
    if (!activeScenario) return;
    const cellKey = `${skillId}-${toolKey}`;
    setTogglingCell(cellKey);
    try {
      await api.setSkillToolToggle(
        skillId,
        activeScenario.id,
        toolKey,
        enabled,
      );
      // Reload toggles for this skill
      const toggles = await api.getSkillToolToggles(
        skillId,
        activeScenario.id,
      );
      setSkillToggles((prev) => ({ ...prev, [skillId]: toggles }));
      await refreshManagedSkills();
    } catch (error) {
      toast.error(getErrorMessage(error, "Failed to toggle"));
    } finally {
      setTogglingCell(null);
    }
  };

  const handleTogglePackTool = async (
    pack: PackWithSkills,
    toolKey: string,
  ) => {
    if (!activeScenario) return;
    const status = getPackToolStatus(pack.skills, toolKey);
    const newEnabled = status !== "all";

    setTogglingCell(`pack-${pack.pack.id}-${toolKey}`);
    try {
      for (const skill of pack.skills) {
        const toggles = skillToggles[skill.id];
        if (!toggles) continue;
        const toggle = toggles.find((t) => t.tool === toolKey);
        if (toggle && toggle.enabled !== newEnabled) {
          await api.setSkillToolToggle(
            skill.id,
            activeScenario.id,
            toolKey,
            newEnabled,
          );
        }
      }
      // Reload all toggles for this pack's skills
      await loadSkillToggles(pack.skills.map((s) => s.id));
      await refreshManagedSkills();
      toast.success(
        newEnabled
          ? `${pack.pack.name} enabled for ${toolKey}`
          : `${pack.pack.name} disabled for ${toolKey}`,
      );
    } catch (error) {
      toast.error(getErrorMessage(error, "Failed to toggle pack"));
    } finally {
      setTogglingCell(null);
    }
  };

  const handleToggleColumnAll = async (toolKey: string) => {
    if (!activeScenario) return;
    // Determine if we should enable or disable all
    let allEnabled = true;
    for (const { skills } of packs) {
      for (const skill of skills) {
        const toggles = skillToggles[skill.id];
        if (!toggles) continue;
        const toggle = toggles.find((t) => t.tool === toolKey);
        if (toggle && !toggle.enabled) {
          allEnabled = false;
          break;
        }
      }
      if (!allEnabled) break;
    }
    const newEnabled = !allEnabled;
    const toolName =
      columns.find((c) => c.key === toolKey)?.display_name || toolKey;

    setTogglingCell(`col-${toolKey}`);
    try {
      for (const { skills } of packs) {
        for (const skill of skills) {
          const toggles = skillToggles[skill.id];
          if (!toggles) continue;
          const toggle = toggles.find((t) => t.tool === toolKey);
          if (toggle && toggle.enabled !== newEnabled) {
            await api.setSkillToolToggle(
              skill.id,
              activeScenario.id,
              toolKey,
              newEnabled,
            );
          }
        }
      }
      // Reload all skill toggles
      const allSkillIds = packs.flatMap(({ skills }) =>
        skills.map((s) => s.id),
      );
      await loadSkillToggles(allSkillIds);
      await refreshManagedSkills();
      toast.success(
        newEnabled
          ? `All skills enabled for ${toolName}`
          : `All skills disabled for ${toolName}`,
      );
    } catch (error) {
      toast.error(getErrorMessage(error, "Failed to toggle column"));
    } finally {
      setTogglingCell(null);
    }
  };

  // ── Render helpers ──

  const renderPackToolCell = (pack: PackWithSkills, tool: ToolInfo) => {
    const status = getPackToolStatus(pack.skills, tool.key);
    const isToggling =
      togglingCell === `pack-${pack.pack.id}-${tool.key}`;

    return (
      <td key={tool.key} className="px-2 py-2 text-center">
        <button
          onClick={() => handleTogglePackTool(pack, tool.key)}
          disabled={isToggling || !activeScenario}
          className={cn(
            "inline-flex h-7 w-7 items-center justify-center rounded-md transition-colors",
            status === "all" &&
              "bg-emerald-500/15 text-emerald-600 dark:text-emerald-400 hover:bg-emerald-500/25",
            status === "some" &&
              "bg-amber-500/15 text-amber-600 dark:text-amber-400 hover:bg-amber-500/25",
            status === "none" &&
              "bg-surface-hover text-faint hover:bg-surface-active hover:text-muted",
            isToggling && "opacity-50",
          )}
          title={
            status === "all"
              ? `All skills in ${pack.pack.name} enabled for ${tool.display_name}`
              : status === "some"
                ? `Some skills in ${pack.pack.name} enabled for ${tool.display_name}`
                : `No skills in ${pack.pack.name} enabled for ${tool.display_name}`
          }
        >
          {isToggling ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
          ) : status === "all" ? (
            <Check className="h-3.5 w-3.5" />
          ) : status === "some" ? (
            <ToggleLeft className="h-3.5 w-3.5" />
          ) : (
            <X className="h-3.5 w-3.5" />
          )}
        </button>
      </td>
    );
  };

  const renderSkillToolCell = (skill: PackSkillRecord, tool: ToolInfo) => {
    const toggles = skillToggles[skill.id];
    const toggle = toggles?.find((t) => t.tool === tool.key);
    const enabled = toggle?.enabled ?? false;
    const cellKey = `${skill.id}-${tool.key}`;
    const isToggling = togglingCell === cellKey;

    if (!toggle) {
      return (
        <td key={tool.key} className="px-2 py-1.5 text-center">
          <span className="inline-flex h-6 w-6 items-center justify-center text-faint">
            <span className="h-1.5 w-1.5 rounded-full bg-border-subtle" />
          </span>
        </td>
      );
    }

    return (
      <td key={tool.key} className="px-2 py-1.5 text-center">
        <button
          onClick={() => handleToggleSkillTool(skill.id, tool.key, !enabled)}
          disabled={isToggling || !activeScenario}
          className={cn(
            "inline-flex h-6 w-6 items-center justify-center rounded transition-colors",
            enabled
              ? "text-emerald-500 hover:bg-emerald-500/10"
              : "text-faint hover:bg-surface-hover hover:text-muted",
            isToggling && "opacity-50",
          )}
          title={
            enabled
              ? `${skill.name} enabled for ${tool.display_name}`
              : `${skill.name} disabled for ${tool.display_name}`
          }
        >
          {isToggling ? (
            <Loader2 className="h-3 w-3 animate-spin" />
          ) : enabled ? (
            <ToggleRight className="h-3.5 w-3.5" />
          ) : (
            <ToggleLeft className="h-3.5 w-3.5" />
          )}
        </button>
      </td>
    );
  };

  // ── Main render ──

  if (loading) {
    return (
      <div className="app-page">
        <div className="app-page-header">
          <h1 className="app-page-title flex items-center gap-2">
            <Grid3X3 className="h-5 w-5 text-accent" />
            Agent Matrix
          </h1>
        </div>
        <div className="flex flex-1 items-center justify-center">
          <Loader2 className="h-8 w-8 animate-spin text-muted" />
        </div>
      </div>
    );
  }

  if (packs.length === 0) {
    return (
      <div className="app-page">
        <div className="app-page-header">
          <h1 className="app-page-title flex items-center gap-2">
            <Grid3X3 className="h-5 w-5 text-accent" />
            Agent Matrix
          </h1>
        </div>
        <div className="flex flex-1 flex-col items-center justify-center pb-20 text-center">
          <Package className="mb-4 h-12 w-12 text-faint" />
          <h3 className="mb-1.5 text-[14px] font-semibold text-tertiary">
            No packs configured
          </h3>
          <p className="text-[13px] text-muted">
            Create packs and assign skills to see the agent matrix view.
          </p>
        </div>
      </div>
    );
  }

  if (columns.length === 0) {
    return (
      <div className="app-page">
        <div className="app-page-header">
          <h1 className="app-page-title flex items-center gap-2">
            <Grid3X3 className="h-5 w-5 text-accent" />
            Agent Matrix
          </h1>
        </div>
        <div className="flex flex-1 flex-col items-center justify-center pb-20 text-center">
          <Grid3X3 className="mb-4 h-12 w-12 text-faint" />
          <h3 className="mb-1.5 text-[14px] font-semibold text-tertiary">
            No agents available
          </h3>
          <p className="text-[13px] text-muted">
            Enable at least one agent in Settings to use the matrix view.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div className="app-page">
      <div className="app-page-header pr-2 pb-1">
        <h1 className="app-page-title flex items-center gap-2">
          <Grid3X3 className="h-5 w-5 text-accent" />
          Agent Matrix
          <span className="app-badge">{packs.length} packs</span>
        </h1>
        {activeScenario && (
          <p className="app-page-subtitle text-tertiary">
            Showing toggles for scenario:{" "}
            <span className="font-medium text-secondary">
              {activeScenario.name}
            </span>
          </p>
        )}
        {!activeScenario && (
          <p className="app-page-subtitle text-amber-600 dark:text-amber-400">
            No active scenario. Select a scenario to manage toggles.
          </p>
        )}
      </div>

      <div className="overflow-x-auto rounded-xl border border-border-subtle bg-surface">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-border-subtle">
              <th className="sticky left-0 z-10 bg-surface px-4 py-3 text-left text-[13px] font-semibold text-secondary">
                Pack / Skill
              </th>
              {columns.map((tool) => (
                <th
                  key={tool.key}
                  className="px-2 py-3 text-center text-[12px] font-semibold text-muted"
                >
                  <div className="flex flex-col items-center gap-1">
                    <span className="truncate max-w-[80px]" title={tool.display_name}>
                      {tool.display_name}
                    </span>
                    {activeScenario && (
                      <button
                        onClick={() => handleToggleColumnAll(tool.key)}
                        disabled={!!togglingCell}
                        className="rounded px-1.5 py-0.5 text-[10px] font-medium text-faint transition-colors hover:bg-surface-hover hover:text-muted"
                      >
                        toggle all
                      </button>
                    )}
                  </div>
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {packs.map(({ pack, skills }) => {
              const isExpanded = expandedPacks.has(pack.id);
              const packData = { pack, skills };

              return (
                <PackRows
                  key={pack.id}
                  packData={packData}
                  isExpanded={isExpanded}
                  columns={columns}
                  onToggleExpand={() => togglePackExpand(pack.id)}
                  renderPackToolCell={renderPackToolCell}
                  renderSkillToolCell={renderSkillToolCell}
                />
              );
            })}
          </tbody>
        </table>
      </div>
    </div>
  );
}

// ── Sub-components ──

interface PackRowsProps {
  packData: PackWithSkills;
  isExpanded: boolean;
  columns: ToolInfo[];
  onToggleExpand: () => void;
  renderPackToolCell: (pack: PackWithSkills, tool: ToolInfo) => React.ReactNode;
  renderSkillToolCell: (
    skill: PackSkillRecord,
    tool: ToolInfo,
  ) => React.ReactNode;
}

function PackRows({
  packData,
  isExpanded,
  columns,
  onToggleExpand,
  renderPackToolCell,
  renderSkillToolCell,
}: PackRowsProps) {
  const { pack, skills } = packData;

  return (
    <>
      {/* Pack header row */}
      <tr
        className={cn(
          "group cursor-pointer border-b border-border-subtle transition-colors hover:bg-surface-hover",
          isExpanded && "bg-surface-hover/50",
        )}
        onClick={onToggleExpand}
      >
        <td className="sticky left-0 z-10 bg-inherit px-4 py-2.5">
          <div className="flex items-center gap-2">
            {isExpanded ? (
              <ChevronDown className="h-3.5 w-3.5 shrink-0 text-muted" />
            ) : (
              <ChevronRight className="h-3.5 w-3.5 shrink-0 text-muted" />
            )}
            <Package className="h-3.5 w-3.5 shrink-0 text-accent" />
            <span className="text-[13px] font-semibold text-primary">
              {pack.name}
            </span>
            <span className="text-[12px] text-faint">
              {skills.length} {skills.length === 1 ? "skill" : "skills"}
            </span>
          </div>
        </td>
        {columns.map((tool) => renderPackToolCell(packData, tool))}
      </tr>

      {/* Expanded skill rows */}
      {isExpanded &&
        skills.map((skill) => (
          <tr
            key={skill.id}
            className="border-b border-border-subtle/50 bg-bg-secondary/30"
          >
            <td className="sticky left-0 z-10 bg-inherit py-1.5 pl-12 pr-4">
              <span
                className="text-[13px] text-secondary"
                title={skill.description || undefined}
              >
                {skill.name}
              </span>
            </td>
            {columns.map((tool) => renderSkillToolCell(skill, tool))}
          </tr>
        ))}
    </>
  );
}
