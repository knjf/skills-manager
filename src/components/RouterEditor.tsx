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
      ? "text-green-600"
      : combinedLen <= 1536
      ? "text-yellow-600"
      : "text-red-600";
  const canSave = desc.trim().length > 0 && combinedLen <= 1536;

  return (
    <div className="space-y-3">
      <label className="block">
        <span className="text-sm font-medium">Router description</span>
        <textarea
          className="w-full border rounded p-2 font-mono text-sm"
          rows={3}
          value={desc}
          onChange={(e) => setDesc(e.target.value)}
          aria-label="Router description"
        />
      </label>

      <label className="block">
        <span className="text-sm font-medium">When to use (trigger phrases)</span>
        <textarea
          className="w-full border rounded p-2 font-mono text-sm"
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
        <span className="text-sm font-medium">
          Body (optional — leave empty for auto-render)
        </span>
        <textarea
          className="w-full border rounded p-2 font-mono text-sm"
          rows={8}
          value={body}
          onChange={(e) => setBody(e.target.value)}
          aria-label="Router body"
        />
      </label>

      <div className="flex gap-2">
        <button
          type="button"
          className="px-3 py-1 bg-blue-600 text-white rounded disabled:opacity-50"
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
            className="px-3 py-1 border rounded"
            onClick={onGenerate}
          >
            Generate with Claude Code
          </button>
        )}
        {onPreview && (
          <button
            type="button"
            className="px-3 py-1 border rounded"
            onClick={onPreview}
          >
            Preview Sync Output
          </button>
        )}
      </div>
    </div>
  );
}
