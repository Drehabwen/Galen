import { useCallback, useEffect, useRef, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

type UpdatePhase = "idle" | "checking" | "available" | "downloading" | "error";

export function UpdateManager() {
  const [phase, setPhase] = useState<UpdatePhase>("idle");
  const [update, setUpdate] = useState<Update | null>(null);
  const [progress, setProgress] = useState(0);
  const [error, setError] = useState<string | null>(null);
  const checkedRef = useRef(false);

  const checkForUpdate = useCallback(async (manual = false) => {
    if (!isTauri() || phase === "checking" || phase === "downloading") return;
    setPhase("checking");
    setError(null);
    try {
      const next = await check({ timeout: 15_000 });
      if (next) {
        setUpdate(next);
        setPhase("available");
      } else {
        setUpdate(null);
        setPhase("idle");
        if (manual) window.alert("当前已经是最新版本。");
      }
    } catch (reason) {
      setError(String(reason));
      setPhase(manual ? "error" : "idle");
    }
  }, [phase]);

  useEffect(() => {
    if (checkedRef.current || !isTauri()) return;
    checkedRef.current = true;
    const timer = window.setTimeout(() => void checkForUpdate(false), 5_000);
    return () => window.clearTimeout(timer);
  }, [checkForUpdate]);

  const installUpdate = async () => {
    if (!update) return;
    setPhase("downloading");
    setError(null);
    setProgress(0);
    let downloaded = 0;
    let total = 0;
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") total = event.data.contentLength ?? 0;
        if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          if (total > 0) setProgress(Math.min(100, Math.round((downloaded / total) * 100)));
        }
        if (event.event === "Finished") setProgress(100);
      }, { timeout: 10 * 60_000 });
      await relaunch();
    } catch (reason) {
      setError(String(reason));
      setPhase("error");
    }
  };

  if (!isTauri()) return null;

  return (
    <>
      <button className={`galen-update-button ${phase === "available" ? "available" : ""}`} type="button" onClick={() => void checkForUpdate(true)} disabled={phase === "checking" || phase === "downloading"}>
        <span className="galen-update-icon">↻</span>
        {phase === "checking" ? "检查更新" : phase === "available" ? `发现 ${update?.version}` : phase === "downloading" ? `${progress}%` : "更新"}
      </button>

      {(phase === "available" || phase === "downloading" || phase === "error") && (
        <div className="galen-update-backdrop" role="presentation">
          <section className="galen-update-dialog" role="dialog" aria-modal="true" aria-label="Galen 软件更新">
            <span className="galen-update-kicker">GALEN UPDATE</span>
            <h2>{phase === "error" ? "更新没有完成" : phase === "downloading" ? "正在安装新版本" : `Galen ${update?.version} 已发布`}</h2>
            {phase === "error" ? (
              <p className="galen-update-error">{error || "暂时无法连接更新服务，请稍后重试。"}</p>
            ) : phase === "downloading" ? (
              <div className="galen-update-progress"><div style={{ width: `${progress}%` }} /><span>{progress}% · 下载完成后将自动重启</span></div>
            ) : (
              <>
                <p className="galen-update-version">当前 {update?.currentVersion} → 最新 {update?.version}</p>
                <div className="galen-update-notes">{update?.body?.trim() || "包含最新功能、稳定性修复和科研工作流改进。"}</div>
              </>
            )}
            <footer>
              {phase !== "downloading" && <button type="button" className="btn btn-ghost" onClick={() => { setPhase("idle"); setError(null); }}>稍后</button>}
              {phase === "available" && <button type="button" className="btn btn-primary" onClick={() => void installUpdate()}>下载并安装</button>}
              {phase === "error" && <button type="button" className="btn btn-primary" onClick={() => void checkForUpdate(true)}>重新检查</button>}
            </footer>
          </section>
        </div>
      )}
    </>
  );
}
