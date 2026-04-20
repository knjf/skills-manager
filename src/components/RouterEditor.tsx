import { useState } from "react";

type Initial = {
  description: string;
  body?: string | null;
  whenToUse?: string | null;
};

type Props = {
  packId: string;
  initial: Initial;
  onSave: (v: {
    description: string;
    body: string | null;
    whenToUse: string | null;
  }) => Promise<void>;
  onGenerate?: () => void;
  onPreview?: () => void;
};

const textareaClass =
  "w-full bg-background border border-border-subtle rounded-[4px] px-3 py-2 font-mono text-[13px] text-secondary focus:outline-none focus:border-border transition-all placeholder-faint";

const buttonBase =
  "px-3 py-1.5 rounded-[4px] text-[13px] transition-colors";
const buttonPrimary = `${buttonBase} bg-accent text-white hover:opacity-90 disabled:opacity-50`;
const buttonSecondary = `${buttonBase} border border-border-subtle text-secondary hover:bg-surface-hover`;

export function RouterEditor({
  initial,
  onSave,
  onGenerate,
  onPreview,
}: Props) {
  const [desc, setDesc] = useState(initial.description);
  const [body, setBody] = useState(initial.body ?? "");
  const [whenToUse, setWhenToUse] = useState(initial.whenToUse ?? "");

  const combinedLen = desc.length + whenToUse.length;
  const color =
    combinedLen <= 1400
      ? "text-emerald-600 dark:text-emerald-400"
      : combinedLen <= 1536
      ? "text-amber-600 dark:text-amber-400"
      : "text-red-600 dark:text-red-400";
  const canSave = desc.trim().length > 0 && combinedLen <= 1536;

  return (
    <div className="space-y-3">
      <label className="block">
        <span className="text-sm font-medium text-primary">Router description</span>
        <textarea
          className={`${textareaClass} mt-1`}
          rows={3}
          value={desc}
          onChange={(e) => setDesc(e.target.value)}
          aria-label="Router description"
        />
      </label>

      <label className="block">
        <span className="text-sm font-medium text-primary">When to use (trigger phrases)</span>
        <textarea
          className={`${textareaClass} mt-1`}
          rows={2}
          value={whenToUse}
          onChange={(e) => setWhenToUse(e.target.value)}
          aria-label="When to use"
          placeholder="Use when user says '...', '...'"
        />
      </label>

      <div data-testid="char-counter" className={`text-xs ${color}`}>
        {combinedLen} / 1536 chars (description + when_to_use)
      </div>

      <label className="block">
        <span className="text-sm font-medium text-primary">
          Body (optional — leave empty for auto-render)
        </span>
        <textarea
          className={`${textareaClass} mt-1`}
          rows={8}
          value={body}
          onChange={(e) => setBody(e.target.value)}
          aria-label="Router body"
        />
      </label>

      <div className="flex gap-2">
        <button
          type="button"
          className={buttonPrimary}
          disabled={!canSave}
          onClick={() =>
            onSave({
              description: desc.trim(),
              body: body.trim() || null,
              whenToUse: whenToUse.trim() || null,
            })
          }
        >
          Save
        </button>
        {onGenerate && (
          <button
            type="button"
            className={buttonSecondary}
            onClick={onGenerate}
          >
            Generate with Claude Code
          </button>
        )}
        {onPreview && (
          <button
            type="button"
            className={buttonSecondary}
            onClick={onPreview}
          >
            Preview Sync Output
          </button>
        )}
      </div>
    </div>
  );
}
