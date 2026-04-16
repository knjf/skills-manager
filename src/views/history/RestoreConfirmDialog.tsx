import type { VersionRecord } from "../../types/history";

interface Props {
  version: VersionRecord;
  onConfirm: () => void;
  onCancel: () => void;
  busy?: boolean;
}

export function RestoreConfirmDialog({
  version,
  onConfirm,
  onCancel,
  busy = false,
}: Props) {
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div
        className="absolute inset-0 bg-black/70 backdrop-blur-sm"
        onClick={busy ? undefined : onCancel}
      />
      <div
        className="relative bg-surface border border-border rounded-xl w-full max-w-md p-5 shadow-2xl mx-4"
        onClick={(e) => e.stopPropagation()}
      >
        <h2 className="text-[13px] font-semibold text-primary mb-3">
          Restore version v{version.version_no}?
        </h2>
        <p className="text-[13px] text-tertiary mb-5">
          This will write the content of v{version.version_no} back to the
          central library and re-sync the active scenario to every agent. The
          existing history is preserved — a new version is created pointing at
          this content.
        </p>
        <div className="flex justify-end gap-2">
          <button
            type="button"
            disabled={busy}
            onClick={onCancel}
            className="px-3 py-1.5 rounded-[4px] text-[13px] font-medium text-tertiary hover:text-secondary hover:bg-surface-hover transition-colors disabled:opacity-40 outline-none"
          >
            Cancel
          </button>
          <button
            type="button"
            disabled={busy}
            onClick={onConfirm}
            className="px-3 py-1.5 rounded-[4px] bg-accent-dark hover:bg-accent text-white text-[13px] font-medium transition-colors disabled:opacity-50 disabled:cursor-not-allowed border border-accent-border outline-none"
          >
            {busy ? "Restoring…" : "Restore"}
          </button>
        </div>
      </div>
    </div>
  );
}
