import { useState } from "react";
import type { Api } from "../api";
import { Card, ErrorBanner } from "../components/ui";
import { fmtBytes, fmtTime } from "../lib/format";
import type { ApiError, CleanupPreview, CleanupResult } from "../types";

export default function Cleanup({ api }: { api: Api }) {
  const [preview, setPreview] = useState<CleanupPreview | null>(null);
  const [selected, setSelected] = useState<string[]>([]);
  const [scanning, setScanning] = useState(false);
  const [deleting, setDeleting] = useState(false);
  const [confirm, setConfirm] = useState(false);
  const [error, setError] = useState<ApiError | null>(null);
  const [result, setResult] = useState<CleanupResult | null>(null);

  const scan = async () => {
    setScanning(true); setError(null); setResult(null); setSelected([]); setConfirm(false);
    try { setPreview(await api.previewTempCleanup()); }
    catch (e) {
      const message = e && typeof e === "object" && "message" in e ? String((e as { message: unknown }).message) : String(e);
      setError({ code: "cleanup_preview_failed", message });
    } finally { setScanning(false); }
  };

  const toggle = (id: string, checked: boolean) => setSelected((old) => checked ? [...old, id] : old.filter((x) => x !== id));
  const selectAll = () => setSelected((old) => preview && old.length === preview.items.length ? [] : (preview?.items.map((item) => item.id) ?? []));

  const remove = async () => {
    if (!preview || selected.length === 0) return;
    setDeleting(true); setError(null); setConfirm(false);
    try {
      const done = await api.deleteTempFiles(selected);
      setResult(done);
      setSelected([]);
      // Refresh with a new backend preview so the displayed selection always matches
      // the deletion cache, including when some in-use files were skipped.
      try {
        setPreview(await api.previewTempCleanup());
      } catch (e) {
        setPreview(null);
        const message = e && typeof e === "object" && "message" in e ? String((e as { message: unknown }).message) : String(e);
        setError({ code: "cleanup_refresh_failed", message });
      }
    } catch (e) {
      const message = e && typeof e === "object" && "message" in e ? String((e as { message: unknown }).message) : String(e);
      setError({ code: "cleanup_delete_failed", message });
    } finally { setDeleting(false); }
  };

  const selectedSize = preview?.items.filter((item) => selected.includes(item.id)).reduce((sum, item) => sum + item.size_bytes, 0) ?? 0;
  const allSelected = !!preview && preview.items.length > 0 && selected.length === preview.items.length;

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center justify-between gap-3"><div><h1 className="text-2xl font-semibold text-slate-100">Temporary File Cleanup</h1><p className="text-sm text-slate-400">Preview first. Select individual files. Confirm before deletion.</p></div><button onClick={scan} disabled={scanning || deleting} className="rounded-lg bg-accent px-4 py-2.5 text-sm font-semibold text-ink-950 disabled:opacity-50">{scanning ? "Scanning…" : preview ? "Refresh preview" : "Scan temp files"}</button></div>
      {error && <ErrorBanner message={error.message} onRetry={scan} />}
      {result && <div role="status" className="rounded-lg border border-emerald-500/30 bg-emerald-500/10 p-4"><div className="font-semibold text-emerald-300">Cleanup finished: {result.deleted_count} file(s), {fmtBytes(result.deleted_bytes)} removed</div><p className="mt-1 text-sm text-slate-300">{result.skipped_count} file(s) skipped. {result.message}</p></div>}
      <Card title="Safety rules">
        <ul className="list-disc space-y-1 pl-5 text-sm text-slate-300"><li>Only regular files directly inside your current user's temp folder are considered; folders and symbolic links are never followed.</li><li>Only files at least 7 days old appear in the preview. TurboFix does not empty Recycle Bin or touch Downloads, Documents, Windows folders or browser profiles.</li><li>Only explicitly selected files from the latest preview can be deleted. Files used by a running app may be skipped.</li></ul>
      </Card>
      {preview && <>
        <div className="grid grid-cols-1 gap-3 sm:grid-cols-3"><Card title="Eligible files in preview"><div className="text-2xl font-semibold">{preview.files_found}</div><p className="mt-1 text-xs text-slate-500">{preview.truncated ? "Showing the 100 largest eligible files" : "All eligible direct child files listed"}</p></Card><Card title="Preview size"><div className="text-2xl font-semibold">{fmtBytes(preview.total_bytes)}</div><p className="mt-1 text-xs text-slate-500">Potential space from listed files</p></Card><Card title="File age"><div className="text-2xl font-semibold">{preview.min_age_days}+ days</div><p className="mt-1 text-xs text-slate-500">Based on last modified time</p></Card></div>
        <Card title={`${preview.root_label} · scanned ${fmtTime(preview.generated_at)}`} right={preview.items.length > 0 ? <button onClick={selectAll} className="text-xs text-accent hover:underline">{allSelected ? "Clear selection" : "Select all listed"}</button> : undefined}>
          <p className="mb-3 text-xs text-slate-500">{preview.note}</p>
          {preview.items.length === 0 ? <p className="py-4 text-sm text-slate-400">No eligible files were found. Nothing will be deleted.</p> : <div className="overflow-x-auto"><table className="w-full text-left text-sm"><thead className="text-xs text-slate-500"><tr><th className="w-10 py-2">Select</th><th className="pr-3">Filename</th><th className="pr-3">Size</th><th>Last modified</th></tr></thead><tbody>{preview.items.map((item) => <tr key={item.id} className="border-t border-ink-700"><td className="py-2"><input type="checkbox" aria-label={`Select ${item.name}`} checked={selected.includes(item.id)} onChange={(e) => toggle(item.id, e.target.checked)} /></td><td className="max-w-[320px] truncate pr-3 font-mono text-xs" title={item.name}>{item.name}</td><td className="whitespace-nowrap pr-3">{fmtBytes(item.size_bytes)}</td><td className="whitespace-nowrap text-xs text-slate-400">{fmtTime(item.modified_at)}</td></tr>)}</tbody></table></div>}
          <div className="mt-4 flex flex-wrap items-center justify-between gap-3"><p className="text-sm text-slate-400">{selected.length} selected · {fmtBytes(selectedSize)}</p><button onClick={() => setConfirm(true)} disabled={selected.length === 0 || deleting} className="rounded-lg bg-red-500 px-4 py-2 text-sm font-semibold text-white disabled:opacity-40">Delete selected…</button></div>
          {confirm && <div role="alertdialog" aria-label="Confirm temporary file deletion" className="mt-4 rounded-lg border border-red-500/30 bg-red-500/10 p-4"><h3 className="font-semibold text-red-200">Delete {selected.length} selected file(s)?</h3><p className="mt-1 text-sm text-slate-300">This permanently removes the selected temp files (estimated {fmtBytes(selectedSize)}). This action cannot be undone.</p><div className="mt-3 flex gap-2"><button onClick={remove} disabled={deleting} className="rounded-lg bg-red-500 px-4 py-2 text-sm font-semibold text-white disabled:opacity-50">{deleting ? "Deleting…" : "Confirm delete"}</button><button onClick={() => setConfirm(false)} disabled={deleting} className="rounded-lg border border-ink-600 px-4 py-2 text-sm">Cancel</button></div></div>}
        </Card>
      </>}
    </div>
  );
}
