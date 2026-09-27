// The print dialog. Two doors into the same room:
//  - the Print button: pick the printer and options, then preview the PDF or
//    send it straight to the printer;
//  - "and on paper" in Morning delivery (and its gear): the same options, saved
//    for the scheduled morning run.
// Every change is saved at once (settings.json), so the gear always reopens on
// what the morning run will actually use.

import { useEffect, useState } from "react";
import type { PrintOptions, PrintStatus } from "../types";
import { printEdition, setPrintOptions } from "../api";

interface Props {
  mode: "print" | "daily";
  status: PrintStatus | null;
  hasEdition: boolean;
  onStatus: (s: PrintStatus) => void;
  onClose: () => void;
  onNotice: (msg: string) => void;
}

const PAGE_CAPS: Array<[number, string]> = [
  [4, "4 pages"],
  [6, "6 pages"],
  [8, "8 pages"],
  [12, "12 pages"],
  [0, "No limit"],
];

/** "Richard_s_Office___Brother_MFC_J1010DW" -> "Richard s Office   Brother MFC J1010DW", tidied. */
export function prettyPrinter(name: string): string {
  return name.replace(/_+/g, " ").replace(/\s+/g, " ").trim();
}

function optionsOf(s: PrintStatus): PrintOptions {
  return { printer: s.printerSetting, color: s.color, qr: s.qr, duplex: s.duplex, maxPages: s.maxPages, copies: s.copies };
}

export default function PrintDialog({ mode, status, hasEdition, onStatus, onClose, onNotice }: Props) {
  const [opts, setOpts] = useState<PrintOptions | null>(status ? optionsOf(status) : null);
  const [working, setWorking] = useState<null | "preview" | "printer">(null);
  const [error, setError] = useState<string | null>(null);

  // The status arrives (or refreshes) after the dialog opens.
  useEffect(() => {
    if (status && !opts) setOpts(optionsOf(status));
  }, [status, opts]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && !working) onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [working, onClose]);

  const change = (patch: Partial<PrintOptions>) => {
    if (!opts) return;
    const next = { ...opts, ...patch };
    setOpts(next);
    setError(null);
    setPrintOptions(next)
      .then(onStatus)
      .catch((e) => setError(String(e)));
  };

  const go = async (target: "preview" | "printer") => {
    setWorking(target);
    setError(null);
    try {
      const result = await printEdition(target);
      onNotice(
        target === "preview"
          ? "The paper edition is open in Preview. Print it from there, or use Print now next time."
          : `Sent to the printer. ${result}`
      );
      onClose();
    } catch (e) {
      setError(String(e));
    } finally {
      setWorking(null);
    }
  };

  const printerName = status?.printer ? prettyPrinter(status.printer) : null;
  const title = mode === "print" ? "Print the paper edition" : "The morning paper";
  const lede =
    mode === "print"
      ? "A three-column Letter-size newspaper: masthead, scoreboard, photos and a QR code per story. Look at it first, or send it straight to the printer."
      : "After each morning run the edition goes to this printer with these settings. Change them any time from the gear next to “and on paper”.";

  return (
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && !working && onClose()} role="presentation">
      <div className="modal" role="dialog" aria-modal="true" aria-labelledby="print-dialog-title">
        <div className="kicker">The paper edition</div>
        <h2 id="print-dialog-title">{title}</h2>
        <p className="modal-lede">{lede}</p>

        {status?.problem && <div className="form-error">{status.problem}</div>}

        {!opts || !status ? (
          <p className="modal-working">Checking the printers…</p>
        ) : (
          <div className="modal-fields">
            <label className="modal-field">
              <span className="lab">
                Printer
                {opts.printer === "" && status.defaultPrinter && <span className="hint">The Mac’s default: {prettyPrinter(status.defaultPrinter)}</span>}
                {opts.printer === "" && !status.defaultPrinter && status.printers.length > 1 && (
                  <span className="hint">No default is set. Pick one.</span>
                )}
              </span>
              <select value={opts.printer} disabled={!!working} onChange={(e) => change({ printer: e.target.value })}>
                <option value="">System default</option>
                {status.printers.map((p) => (
                  <option key={p} value={p}>
                    {prettyPrinter(p)}
                  </option>
                ))}
              </select>
            </label>

            <label className="modal-field">
              <span className="lab">
                Colour photos
                <span className="hint">Off is newsprint: black &amp; white, less ink.</span>
              </span>
              <span className="toggle">
                <input type="checkbox" checked={opts.color} disabled={!!working} onChange={(e) => change({ color: e.target.checked })} />
              </span>
            </label>

            <label className="modal-field">
              <span className="lab">
                QR codes
                <span className="hint">One per story, since paper can’t be clicked.</span>
              </span>
              <span className="toggle">
                <input type="checkbox" checked={opts.qr} disabled={!!working} onChange={(e) => change({ qr: e.target.checked })} />
              </span>
            </label>

            <label className="modal-field">
              <span className="lab">Double-sided</span>
              <span className="toggle">
                <input type="checkbox" checked={opts.duplex} disabled={!!working} onChange={(e) => change({ duplex: e.target.checked })} />
              </span>
            </label>

            <label className="modal-field">
              <span className="lab">
                Never more than
                <span className="hint">A long edition stops here rather than eating the paper tray.</span>
              </span>
              <select value={opts.maxPages} disabled={!!working} onChange={(e) => change({ maxPages: Number(e.target.value) })}>
                {PAGE_CAPS.map(([n, label]) => (
                  <option key={n} value={n}>
                    {label}
                  </option>
                ))}
              </select>
            </label>

            <label className="modal-field">
              <span className="lab">Copies</span>
              <select value={opts.copies} disabled={!!working} onChange={(e) => change({ copies: Number(e.target.value) })}>
                {[1, 2, 3, 4, 5].map((n) => (
                  <option key={n} value={n}>
                    {n}
                  </option>
                ))}
              </select>
            </label>
          </div>
        )}

        {error && <div className="form-error">{error}</div>}

        <div className="modal-actions">
          {working ? (
            <span className="modal-working">{working === "preview" ? "Laying out the paper edition…" : `Laying out and sending to ${printerName ?? "the printer"}…`}</span>
          ) : mode === "print" && !hasEdition ? (
            <span className="modal-working">There’s no edition to print yet. Hit Refresh first.</span>
          ) : mode === "daily" ? (
            <span className="modal-working">{printerName ? `Mornings go to ${printerName}.` : ""}</span>
          ) : null}
          <span className="spacer" />
          <button className="btn" type="button" onClick={onClose} disabled={!!working}>
            {mode === "daily" ? "Done" : "Close"}
          </button>
          {mode === "print" && (
            <>
              <button className="btn" type="button" onClick={() => void go("preview")} disabled={!!working || !hasEdition || !status?.browserFound}>
                Preview PDF
              </button>
              <button className="btn primary" type="button" onClick={() => void go("printer")} disabled={!!working || !hasEdition || !!status?.problem}>
                Print now
              </button>
            </>
          )}
        </div>
      </div>
    </div>
  );
}
