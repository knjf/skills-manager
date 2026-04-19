import { useCallback, useEffect, useState } from "react";
import {
  Plug,
  Loader2,
  RefreshCw,
  AlertTriangle,
  ToggleLeft,
  ToggleRight,
  Search,
} from "lucide-react";
import { toast } from "sonner";
import { cn } from "../utils";
import { useApp } from "../context/AppContext";
import { getErrorMessage } from "../lib/error";
import * as api from "../lib/tauri";
import type { ManagedPlugin, ScenarioPlugin } from "../lib/tauri";

export function PluginsView() {
  const { activeScenario } = useApp();

  const [plugins, setPlugins] = useState<ManagedPlugin[]>([]);
  const [scenarioPlugins, setScenarioPlugins] = useState<ScenarioPlugin[]>([]);
  const [loading, setLoading] = useState(true);
  const [scanning, setScanning] = useState(false);
  const [togglingPlugin, setTogglingPlugin] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [phase3Available, setPhase3Available] = useState(true);

  // ── Data loading ──

  const loadPlugins = useCallback(async () => {
    setLoading(true);
    try {
      const managed = await api.getManagedPlugins();
      setPlugins(managed);
      setPhase3Available(true);
    } catch {
      // Phase 3 commands not available yet
      setPlugins([]);
      setPhase3Available(false);
    } finally {
      setLoading(false);
    }
  }, []);

  const loadScenarioPlugins = useCallback(async () => {
    if (!activeScenario) {
      setScenarioPlugins([]);
      return;
    }
    try {
      const sp = await api.getScenarioPlugins(activeScenario.id);
      setScenarioPlugins(sp);
    } catch {
      setScenarioPlugins([]);
    }
  }, [activeScenario]);

  useEffect(() => {
    loadPlugins();
  }, [loadPlugins]);

  useEffect(() => {
    if (phase3Available) {
      loadScenarioPlugins();
    }
  }, [phase3Available, loadScenarioPlugins]);

  // ── Handlers ──

  const handleScan = async () => {
    setScanning(true);
    try {
      await api.scanPlugins();
      await loadPlugins();
      await loadScenarioPlugins();
      toast.success("Plugin scan completed");
    } catch (error) {
      toast.error(getErrorMessage(error, "Failed to scan plugins"));
    } finally {
      setScanning(false);
    }
  };

  const handleTogglePlugin = async (pluginId: string, enabled: boolean) => {
    if (!activeScenario) return;
    setTogglingPlugin(pluginId);
    try {
      await api.setScenarioPluginEnabled(
        activeScenario.id,
        pluginId,
        enabled,
      );
      await loadScenarioPlugins();
      const plugin = plugins.find((p) => p.id === pluginId);
      toast.success(
        enabled
          ? `${plugin?.display_name || "Plugin"} enabled`
          : `${plugin?.display_name || "Plugin"} disabled`,
      );
    } catch (error) {
      toast.error(getErrorMessage(error, "Failed to toggle plugin"));
    } finally {
      setTogglingPlugin(null);
    }
  };

  // ── Derived state ──

  const isPluginEnabled = (pluginId: string): boolean => {
    const sp = scenarioPlugins.find((p) => p.plugin.id === pluginId);
    return sp?.enabled ?? false;
  };

  const filtered = plugins.filter((plugin) => {
    const name = (plugin.display_name || plugin.plugin_key).toLowerCase();
    return name.includes(search.toLowerCase());
  });

  // ── Render ──

  if (!phase3Available) {
    return (
      <div className="app-page">
        <div className="app-page-header">
          <h1 className="app-page-title flex items-center gap-2">
            <Plug className="h-5 w-5 text-accent" />
            Plugins
          </h1>
        </div>
        <div className="flex flex-1 flex-col items-center justify-center pb-20 text-center">
          <AlertTriangle className="mb-4 h-12 w-12 text-amber-400" />
          <h3 className="mb-1.5 text-[14px] font-semibold text-tertiary">
            Plugin management not available yet
          </h3>
          <p className="max-w-md text-[13px] text-muted">
            The plugin management backend (Phase 3) has not been merged yet.
            Once plugin commands are available, this view will allow you to
            discover, enable, and disable plugins per scenario.
          </p>
          <div className="mt-6 rounded-lg border border-border-subtle bg-surface p-4 text-left">
            <h4 className="mb-2 text-[13px] font-semibold text-secondary">
              Planned Features
            </h4>
            <ul className="space-y-1.5 text-[13px] text-muted">
              <li className="flex items-start gap-2">
                <Search className="mt-0.5 h-3.5 w-3.5 shrink-0 text-faint" />
                Scan and discover installed plugins
              </li>
              <li className="flex items-start gap-2">
                <ToggleRight className="mt-0.5 h-3.5 w-3.5 shrink-0 text-faint" />
                Enable/disable plugins per scenario
              </li>
              <li className="flex items-start gap-2">
                <Plug className="mt-0.5 h-3.5 w-3.5 shrink-0 text-faint" />
                View plugin scope, install path, and status
              </li>
            </ul>
          </div>
        </div>
      </div>
    );
  }

  if (loading) {
    return (
      <div className="app-page">
        <div className="app-page-header">
          <h1 className="app-page-title flex items-center gap-2">
            <Plug className="h-5 w-5 text-accent" />
            Plugins
          </h1>
        </div>
        <div className="flex flex-1 items-center justify-center">
          <Loader2 className="h-8 w-8 animate-spin text-muted" />
        </div>
      </div>
    );
  }

  return (
    <div className="app-page">
      <div className="app-page-header pr-2 pb-1">
        <h1 className="app-page-title flex items-center gap-2">
          <Plug className="h-5 w-5 text-accent" />
          Plugins
          <span className="app-badge">{plugins.length}</span>
        </h1>
        {activeScenario && (
          <p className="app-page-subtitle text-tertiary">
            Managing plugins for scenario:{" "}
            <span className="font-medium text-secondary">
              {activeScenario.name}
            </span>
          </p>
        )}
      </div>

      <div className="app-toolbar">
        <div className="flex flex-1 gap-3">
          <div className="relative w-full max-w-[280px]">
            <Search className="absolute left-3 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted" />
            <input
              type="text"
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              placeholder="Search plugins..."
              className="app-input w-full pl-9 font-medium"
              autoCapitalize="none"
              autoCorrect="off"
              spellCheck={false}
            />
          </div>
        </div>
        <button
          onClick={handleScan}
          disabled={scanning}
          className="inline-flex items-center gap-1.5 rounded-md px-3 py-2 text-[13px] font-medium text-muted transition-colors hover:bg-surface-hover hover:text-secondary disabled:opacity-50"
        >
          <RefreshCw
            className={cn("h-3.5 w-3.5", scanning && "animate-spin")}
          />
          {scanning ? "Scanning..." : "Scan Plugins"}
        </button>
      </div>

      {filtered.length === 0 ? (
        <div className="flex flex-1 flex-col items-center justify-center pb-20 text-center">
          <Plug className="mb-4 h-12 w-12 text-faint" />
          <h3 className="mb-1.5 text-[14px] font-semibold text-tertiary">
            {plugins.length === 0
              ? "No plugins discovered"
              : "No plugins match the search"}
          </h3>
          <p className="text-[13px] text-muted">
            {plugins.length === 0
              ? "Click \"Scan Plugins\" to discover installed plugins."
              : "Try a different search term."}
          </p>
        </div>
      ) : (
        <div className="flex flex-col gap-1 pb-8">
          {filtered.map((plugin) => {
            const enabled = isPluginEnabled(plugin.id);
            const isToggling = togglingPlugin === plugin.id;

            return (
              <div
                key={plugin.id}
                className={cn(
                  "app-panel group flex items-center gap-3.5 rounded-xl border-transparent px-4 py-3.5 transition-all hover:border-border hover:bg-surface-hover",
                  enabled && "border-l-2 border-l-emerald-500",
                )}
              >
                {/* Icon */}
                <div
                  className={cn(
                    "flex h-8 w-8 shrink-0 items-center justify-center rounded-lg",
                    enabled
                      ? "bg-emerald-500/10 text-emerald-500"
                      : "bg-surface-hover text-muted",
                  )}
                >
                  <Plug className="h-4 w-4" />
                </div>

                {/* Info */}
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <h3 className="truncate text-[14px] font-semibold text-primary">
                      {plugin.display_name || plugin.plugin_key}
                    </h3>
                    <span
                      className={cn(
                        "shrink-0 rounded-full px-2 py-0.5 text-[11px] font-medium",
                        "bg-violet-500/10 text-violet-600 dark:text-violet-400",
                      )}
                    >
                      {plugin.plugin_key.includes("@") ? plugin.plugin_key.split("@")[1] : "local"}
                    </span>
                  </div>
                  <p
                    className="mt-0.5 truncate text-[12px] text-faint"
                    title={plugin.plugin_key}
                  >
                    {plugin.plugin_key}
                  </p>
                </div>

                {/* Status indicator */}
                <div className="flex shrink-0 items-center gap-3">
                  <span
                    className={cn(
                      "text-[13px] font-medium",
                      enabled
                        ? "text-emerald-600 dark:text-emerald-400"
                        : "text-muted",
                    )}
                  >
                    {enabled ? "Active" : "Disabled"}
                  </span>

                  {/* Toggle */}
                  <button
                    onClick={() => handleTogglePlugin(plugin.id, !enabled)}
                    disabled={isToggling || !activeScenario}
                    className={cn(
                      "inline-flex h-8 w-8 items-center justify-center rounded-lg transition-colors",
                      enabled
                        ? "text-emerald-500 hover:bg-emerald-500/10"
                        : "text-faint hover:bg-surface-hover hover:text-muted",
                      isToggling && "opacity-50",
                    )}
                    title={enabled ? "Disable plugin" : "Enable plugin"}
                  >
                    {isToggling ? (
                      <Loader2 className="h-4 w-4 animate-spin" />
                    ) : enabled ? (
                      <ToggleRight className="h-5 w-5" />
                    ) : (
                      <ToggleLeft className="h-5 w-5" />
                    )}
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
