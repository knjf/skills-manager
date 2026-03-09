import { useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { X, FileText, Folder, CheckCircle2, Circle, Loader2 } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { useTranslation } from "react-i18next";
import { cn } from "../utils";
import { getSkillDocument, type ManagedSkill, type SkillDocument } from "../lib/tauri";

export interface SyncMeta {
  syncedToolKeys: string[];
  syncedToolLabels: string[];
  pendingToolKeys: string[];
  pendingToolLabels: string[];
}

interface Props {
  skill: ManagedSkill | null;
  onClose: () => void;
  syncMeta?: SyncMeta | null;
  syncing?: boolean;
  onSync?: (mode: "sync" | "unsync") => void;
}

function stripFrontmatter(content: string) {
  if (!content.startsWith("---\n")) return content;

  const end = content.indexOf("\n---\n", 4);
  if (end === -1) return content;

  return content.slice(end + 5).trimStart();
}

export function SkillDetailPanel({ skill, onClose, syncMeta, syncing, onSync }: Props) {
  const { t } = useTranslation();
  const [doc, setDoc] = useState<SkillDocument | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!skill) {
      setDoc(null);
      return;
    }
    setLoading(true);
    getSkillDocument(skill.id)
      .then(setDoc)
      .catch(() => setDoc(null))
      .finally(() => setLoading(false));
  }, [skill]);

  const markdown = useMemo(() => {
    if (!doc) return "";
    return stripFrontmatter(doc.content);
  }, [doc]);

  if (!skill) return null;

  return createPortal(
    <div className="fixed inset-y-0 right-0 left-[220px] z-50 flex">
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={onClose} />
      <div className="relative flex h-full min-h-0 w-full flex-col border-l border-border-subtle bg-bg-secondary shadow-2xl animate-in slide-in-from-right duration-200">
        <div className="flex items-start justify-between border-b border-border-subtle px-5 py-4">
          <div className="min-w-0 mr-3">
            <h2 className="text-[14px] font-semibold text-primary truncate">{skill.name}</h2>
            {skill.description && (
              <p className="text-[13px] text-muted mt-0.5 line-clamp-2">{skill.description}</p>
            )}
          </div>
          <button
            onClick={onClose}
            className="text-muted hover:text-secondary p-1.5 rounded-[4px] hover:bg-surface-hover transition-colors outline-none shrink-0"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="flex items-center gap-4 border-b border-border-subtle px-5 py-2.5 text-[13px] text-muted">
          <div className="flex items-center gap-1.5">
            <FileText className="w-3 h-3" />
            {doc?.filename || "—"}
          </div>
          <div className="flex items-center gap-1.5 min-w-0">
            <Folder className="w-3 h-3 shrink-0" />
            <span className="font-mono truncate">{skill.central_path}</span>
          </div>
        </div>

        {syncMeta && onSync && (
          <div className="flex items-center justify-between border-b border-border-subtle px-5 py-2 text-[13px]">
            <div className="flex items-center gap-2 text-muted">
              {syncMeta.syncedToolKeys.length > 0 ? (
                <CheckCircle2 className="w-3 h-3 text-emerald-500" />
              ) : (
                <Circle className="w-3 h-3 text-faint" />
              )}
              <span>
                {t("mySkills.syncSummary", {
                  synced: syncMeta.syncedToolKeys.length,
                  total: syncMeta.syncedToolKeys.length + syncMeta.pendingToolKeys.length,
                })}
              </span>
              {syncMeta.syncedToolLabels.length > 0 && (
                <span className="text-muted">({syncMeta.syncedToolLabels.join(", ")})</span>
              )}
            </div>
            <div className="flex items-center gap-1.5">
              {syncMeta.pendingToolKeys.length > 0 && (
                <button
                  onClick={() => onSync("sync")}
                  disabled={syncing}
                  className={cn(
                    "rounded px-2 py-0.5 text-[13px] font-medium transition-colors outline-none",
                    "text-muted hover:bg-surface-hover hover:text-secondary",
                    syncing && "opacity-50"
                  )}
                >
                  {syncing ? <Loader2 className="h-3 w-3 animate-spin" /> : t("mySkills.syncMissing", { count: syncMeta.pendingToolKeys.length })}
                </button>
              )}
              {syncMeta.syncedToolKeys.length > 0 && (
                <button
                  onClick={() => onSync("unsync")}
                  disabled={syncing}
                  className={cn(
                    "rounded px-2 py-0.5 text-[13px] font-medium transition-colors outline-none",
                    "text-faint hover:bg-red-500/10 hover:text-red-400",
                    syncing && "opacity-50"
                  )}
                >
                  {t("mySkills.unsyncSelected", { count: syncMeta.syncedToolKeys.length })}
                </button>
              )}
            </div>
          </div>
        )}

        <div className="min-h-0 flex-1 overflow-y-auto px-5 py-5 scrollbar-hide">
          {loading ? (
            <div className="text-[13px] text-muted text-center mt-12">{t("common.loading")}</div>
          ) : doc ? (
            <article className="mx-auto w-full max-w-[1240px] text-[13px] leading-6 text-secondary">
              <ReactMarkdown
                remarkPlugins={[remarkGfm]}
                components={{
                  h1: ({ className, ...props }) => (
                    <h1
                      className={cn("mb-4 text-[28px] font-semibold leading-tight text-primary", className)}
                      {...props}
                    />
                  ),
                  h2: ({ className, ...props }) => (
                    <h2
                      className={cn("mb-3 mt-8 text-[20px] font-semibold leading-tight text-primary", className)}
                      {...props}
                    />
                  ),
                  h3: ({ className, ...props }) => (
                    <h3
                      className={cn("mb-2 mt-6 text-[16px] font-semibold leading-tight text-primary", className)}
                      {...props}
                    />
                  ),
                  p: ({ className, ...props }) => (
                    <p className={cn("mb-4 text-[13px] leading-6 text-secondary", className)} {...props} />
                  ),
                  a: ({ className, ...props }) => (
                    <a
                      className={cn("text-accent-light underline decoration-accent-border underline-offset-4", className)}
                      target="_blank"
                      rel="noreferrer"
                      {...props}
                    />
                  ),
                  ul: ({ className, ...props }) => (
                    <ul className={cn("mb-4 list-disc space-y-1 pl-5 text-secondary", className)} {...props} />
                  ),
                  ol: ({ className, ...props }) => (
                    <ol className={cn("mb-4 list-decimal space-y-1 pl-5 text-secondary", className)} {...props} />
                  ),
                  li: ({ className, ...props }) => (
                    <li className={cn("pl-1 marker:text-muted", className)} {...props} />
                  ),
                  blockquote: ({ className, ...props }) => (
                    <blockquote
                      className={cn(
                        "mb-4 border-l-2 border-accent-border bg-surface/70 px-4 py-2 text-tertiary italic",
                        className
                      )}
                      {...props}
                    />
                  ),
                  hr: ({ className, ...props }) => (
                    <hr className={cn("my-6 border-border-subtle", className)} {...props} />
                  ),
                  code: ({ className, children, ...props }) => {
                    const isBlock = String(className || "").includes("language-");
                    if (isBlock) {
                      return (
                        <code className={cn("block text-[13px] leading-6 text-secondary", className)} {...props}>
                          {children}
                        </code>
                      );
                    }

                    return (
                      <code
                        className={cn(
                          "rounded bg-surface-hover px-1.5 py-0.5 font-mono text-[13px] text-accent-light",
                          className
                        )}
                        {...props}
                      >
                        {children}
                      </code>
                    );
                  },
                  pre: ({ className, ...props }) => (
                    <pre
                      className={cn(
                        "mb-4 overflow-x-auto rounded-xl border border-border-subtle bg-background px-4 py-3",
                        className
                      )}
                      {...props}
                    />
                  ),
                  table: ({ className, ...props }) => (
                    <div className="mb-4 overflow-x-auto rounded-xl border border-border-subtle">
                      <table className={cn("min-w-full border-collapse text-left text-[13px]", className)} {...props} />
                    </div>
                  ),
                  thead: ({ className, ...props }) => (
                    <thead className={cn("bg-surface-hover text-primary", className)} {...props} />
                  ),
                  th: ({ className, ...props }) => (
                    <th className={cn("border-b border-border-subtle px-3 py-2 font-medium", className)} {...props} />
                  ),
                  td: ({ className, ...props }) => (
                    <td className={cn("border-b border-border-subtle px-3 py-2 text-secondary", className)} {...props} />
                  ),
                }}
              >
                {markdown}
              </ReactMarkdown>
            </article>
          ) : (
            <div className="text-[13px] text-muted text-center mt-12">{t("common.documentMissing")}</div>
          )}
        </div>
      </div>
    </div>,
    document.body
  );
}
