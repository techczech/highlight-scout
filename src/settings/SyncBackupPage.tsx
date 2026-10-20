// Custom page of the Settings window: the actions that are not settings. Sync now with the per-source table, and the
// Cloudflare R2 test / back up / restore buttons. The R2 values and keys are ordinary settings and slots on their own pages.
import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getSyncErrorTimes, getSyncStatus, r2BackupNow, r2RestoreNow, syncNow, testR2Connection } from "../lib/api";
import type { SyncStatus } from "../types";
import { SyncTable } from "../components/SyncTable";
import type { SettingsView } from "./host";

export function SyncBackupPage({ view }: { view: SettingsView }) {
  const [status, setStatus] = useState<SyncStatus | null>(null);
  const [times, setTimes] = useState<Record<string, string>>({});
  const [busy, setBusy] = useState(false);
  const [r2Line, setR2Line] = useState("");

  useEffect(() => {
    const load = () => {
      getSyncStatus().then(setStatus).catch(() => {});
      getSyncErrorTimes().then(setTimes).catch(() => {});
    };
    load();
    const un = listen("sync:finished", load);
    return () => { un.then((f) => f()); };
  }, []);

  const enabled = view.state.settings["backup.r2Enabled"] === true;
  const hasKeys = ["accounts.r2AccessKeyId", "accounts.r2SecretAccessKey"].every((id) => view.slots.find((s) => s.id === id)?.status === "connected");

  const sync = async () => {
    setBusy(true);
    try { await syncNow(); } catch { /* the table shows the failure */ }
    setBusy(false);
    getSyncStatus().then(setStatus).catch(() => {});
  };
  const r2 = async (label: string, fn: () => Promise<{ message: string }>) => {
    setBusy(true);
    setR2Line(`${label}…`);
    try { setR2Line((await fn()).message); } catch (e) { setR2Line(`Failed: ${e instanceof Error ? e.message : String(e)}`); }
    setBusy(false);
  };

  return (
    <div className="hs-sbp">
      <div className="bsw-card">
        <div className="bsw-ch">Sync<span className="bsw-sp" /></div>
        <div className="hs-sbp-body">
          <p className="hs-sbp-note">Highlight Scout syncs every connected source (Readwise highlights, Readwise saved tweets, Zotero when its database is found) in the background.</p>
          <button className="bsw-btn" disabled={busy || status?.running} onClick={sync}>{status?.running || busy ? "Syncing…" : "Sync all sources now"}</button>
          {status ? <SyncTable status={status} errorTimes={times} /> : <p className="hs-sbp-note">Loading sync status…</p>}
        </div>
      </div>
      <div className="bsw-card">
        <div className="bsw-ch">Cloudflare R2 backup<span className="bsw-sp" /></div>
        <div className="hs-sbp-body">
          <p className="hs-sbp-note">{enabled ? "Backup is on." : "Backup is off; turn it on under Backup."}{hasKeys ? "" : " The R2 keys are missing; add them under Accounts and keys."}</p>
          <div className="hs-sbp-row">
            <button className="bsw-btn" disabled={busy} onClick={() => r2("Testing R2", testR2Connection)}>Test connection</button>
            <button className="bsw-btn" disabled={busy || !enabled || !hasKeys} onClick={() => r2("Backing up to R2", r2BackupNow)}>Back up now</button>
            <button className="bsw-btn" disabled={busy || !enabled || !hasKeys} onClick={() => r2("Restoring from R2", r2RestoreNow)}>Restore from R2</button>
          </div>
          {r2Line && <p className="hs-sbp-note" role="status">{r2Line}</p>}
        </div>
      </div>
    </div>
  );
}
