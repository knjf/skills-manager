import { useState, useEffect } from "react";
import { X, FileText, Folder } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { useTranslation } from "react-i18next";
import { getSkillDocument, type ManagedSkill, type SkillDocument } from "../lib/tauri";

interface Props {
  skill: ManagedSkill | null;
  onClose: () => void;
}

export function SkillDetailPanel({ skill, onClose }: Props) {
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

  if (!skill) return null;

  return (
    <div className="fixed inset-0 z-50 flex">
      <div className="absolute inset-0 bg-black/60 backdrop-blur-sm" onClick={onClose} />
      <div className="relative ml-auto w-full max-w-xl bg-bg-secondary border-l border-border-subtle h-full flex flex-col shadow-2xl animate-in slide-in-from-right duration-200">
        {/* Header */}
        <div className="flex items-start justify-between px-5 py-4 border-b border-border-subtle">
          <div className="min-w-0 mr-3">
            <h2 className="text-[14px] font-semibold text-primary truncate">{skill.name}</h2>
            {skill.description && (
              <p className="text-[12px] text-muted mt-0.5 line-clamp-2">{skill.description}</p>
            )}
          </div>
          <button
            onClick={onClose}
            className="text-muted hover:text-secondary p-1.5 rounded-[4px] hover:bg-surface-hover transition-colors outline-none shrink-0"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Meta */}
        <div className="px-5 py-2.5 border-b border-border-subtle flex items-center gap-4 text-[11px] text-muted">
          <div className="flex items-center gap-1.5">
            <FileText className="w-3 h-3" />
            {doc?.filename || "—"}
          </div>
          <div className="flex items-center gap-1.5 min-w-0">
            <Folder className="w-3 h-3 shrink-0" />
            <span className="font-mono truncate">{skill.central_path}</span>
          </div>
        </div>

        {/* Content */}
        <div className="flex-1 overflow-y-auto px-5 py-4 scrollbar-hide">
          {loading ? (
            <div className="text-[12px] text-muted text-center mt-12">{t("common.loading")}</div>
          ) : doc ? (
            <article className="prose prose-sm max-w-none dark:prose-invert prose-headings:text-secondary prose-p:text-tertiary prose-a:text-accent prose-code:text-accent-light prose-code:bg-surface-hover prose-code:px-1.5 prose-code:py-0.5 prose-code:rounded prose-pre:bg-background prose-pre:border prose-pre:border-border-subtle">
              <ReactMarkdown remarkPlugins={[remarkGfm]}>
                {doc.content}
              </ReactMarkdown>
            </article>
          ) : (
            <div className="text-[12px] text-muted text-center mt-12">{t("common.documentMissing")}</div>
          )}
        </div>
      </div>
    </div>
  );
}
