// Settings → Sync as a table (HS-3A): per source, the last synced time and
// the result of its last run or its current error. Hook-free.
import type { SyncStatus } from "../types";
import { syncRows, syncTableHeading } from "../lib/syncTable";

export function SyncTable({ status, errorTimes, now }: { status: SyncStatus; errorTimes: Record<string, string>; now?: Date }) {
  const rows = syncRows(status, errorTimes, now);
  return (
    <div data-testid="sync-table">
      <div className="text-[11px] font-semibold uppercase tracking-wide text-zinc-400">{syncTableHeading(status, now)}</div>
      <table className="sync">
        <thead>
          <tr><th>Source</th><th>Last synced</th><th>Last result</th></tr>
        </thead>
        <tbody>
          {rows.map((r) => (
            <tr key={r.key} data-source={r.key}>
              <td><span className={`sdot ${r.state === "ok" ? "" : r.state}`} />{r.label}</td>
              <td>
                {r.lastSynced}
                {r.lastTry && <div className="try">{r.lastTry}</div>}
              </td>
              <td className={r.state === "failed" ? "bad" : r.state === "off" ? "try" : ""}>{r.result}</td>
            </tr>
          ))}
        </tbody>
      </table>
      <p className="mt-2 text-xs text-zinc-400">A failed source keeps its red line in the main window until a later sync of that source succeeds.</p>
    </div>
  );
}
