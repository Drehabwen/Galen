import { useMemo, useState } from "react";
import type { ModelStatus } from "../types";

interface ModelStatusPanelProps {
  statuses: ModelStatus[];
  onClose: () => void;
  onOpenWizard: () => void;
  onSaveApiKey: (apiKey: string, defaultModel?: string) => Promise<void>;
  onTestConnection: () => Promise<string>;
}

type SaveState =
  | { kind: "idle" }
  | { kind: "saving" }
  | { kind: "ok"; message: string }
  | { kind: "error"; message: string };

export function ModelStatusPanel({
  statuses,
  onClose,
  onOpenWizard,
  onSaveApiKey,
  onTestConnection,
}: ModelStatusPanelProps) {
  const initialModel = useMemo(
    () => statuses.find((status) => status.is_default)?.name ?? statuses[0]?.name ?? "deepseek-v4-flash",
    [statuses],
  );
  const [apiKey, setApiKey] = useState("");
  const [defaultModel, setDefaultModel] = useState(initialModel);
  const [showKey, setShowKey] = useState(false);
  const [saveState, setSaveState] = useState<SaveState>({ kind: "idle" });

  const handleSave = async () => {
    const nextKey = apiKey.trim();
    if (!nextKey) return;
    setSaveState({ kind: "saving" });
    try {
      await onSaveApiKey(nextKey, defaultModel);
      setApiKey("");
      const result = await onTestConnection();
      setSaveState({ kind: "ok", message: result });
    } catch (error) {
      setSaveState({ kind: "error", message: String(error) });
    }
  };

  const handleTest = async () => {
    setSaveState({ kind: "saving" });
    try {
      const result = await onTestConnection();
      setSaveState({ kind: "ok", message: result });
    } catch (error) {
      setSaveState({ kind: "error", message: String(error) });
    }
  };

  return (
    <div className="cmd-overlay settings-overlay" onClick={onClose} role="presentation">
      <div
        className="model-status-panel settings-panel"
        onClick={(event) => event.stopPropagation()}
        role="dialog"
        aria-modal="true"
        aria-labelledby="settings-title"
      >
        <div className="model-status-header">
          <div>
            <span className="settings-kicker">LOCAL MODEL VAULT</span>
            <h2 id="settings-title">设置</h2>
            <p>管理 Galen 使用的模型与本机 API Key。</p>
          </div>
          <button className="btn btn-ghost btn-sm" onClick={onClose} aria-label="关闭设置">
            ✕
          </button>
        </div>

        <div className="settings-scroll-area">
          <section className="settings-section" aria-labelledby="settings-key-title">
            <div className="settings-section-heading">
              <div>
                <h3 id="settings-key-title">DeepSeek API Key</h3>
                <p>新 Key 会替换当前模型使用的密钥，并立即重新载入。</p>
              </div>
              <span className="settings-local-badge">仅保存在本机</span>
            </div>

            <label className="welcome-field-label" htmlFor="settings-api-key">
              输入新 Key
            </label>
            <div className="settings-secret-field">
              <input
                id="settings-api-key"
                type={showKey ? "text" : "password"}
                className="welcome-key-input"
                placeholder="粘贴新的 DeepSeek API Key"
                value={apiKey}
                onChange={(event) => {
                  setApiKey(event.target.value);
                  if (saveState.kind !== "idle") setSaveState({ kind: "idle" });
                }}
                onKeyDown={(event) => event.key === "Enter" && void handleSave()}
                autoComplete="new-password"
                spellCheck={false}
              />
              <button
                type="button"
                className="btn btn-ghost btn-sm settings-reveal-key"
                onClick={() => setShowKey((visible) => !visible)}
                aria-pressed={showKey}
              >
                {showKey ? "隐藏" : "显示"}
              </button>
            </div>

            <label className="welcome-field-label" htmlFor="settings-default-model">
              保存后默认使用
            </label>
            <select
              id="settings-default-model"
              className="settings-model-select"
              value={defaultModel}
              onChange={(event) => setDefaultModel(event.target.value)}
            >
              {(statuses.length > 0
                ? statuses
                : [
                    { name: "deepseek-v4-flash", model_id: "deepseek-v4-flash" },
                    { name: "deepseek-v4-pro", model_id: "deepseek-v4-pro" },
                  ]
              ).map((status) => (
                <option key={status.name} value={status.name}>
                  {status.name} · {status.model_id}
                </option>
              ))}
            </select>

            <div className="settings-actions">
              <button
                className="btn btn-primary"
                onClick={() => void handleSave()}
                disabled={!apiKey.trim() || saveState.kind === "saving"}
              >
                {saveState.kind === "saving" ? "正在验证…" : "保存并验证"}
              </button>
              <button
                className="btn btn-ghost"
                onClick={() => void handleTest()}
                disabled={saveState.kind === "saving"}
              >
                测试当前连接
              </button>
            </div>

            {saveState.kind === "ok" && (
              <div className="welcome-msg ok" role="status">
                <span>✓</span> Key 已保存并生效。{saveState.message}
              </div>
            )}
            {saveState.kind === "error" && (
              <div className="welcome-msg fail" role="alert">
                <span>✕</span>
                <div>
                  保存或连接测试失败。
                  <div className="welcome-msg-detail">{saveState.message}</div>
                </div>
              </div>
            )}
          </section>

          <section className="settings-section" aria-labelledby="settings-models-title">
            <div className="settings-section-heading">
              <div>
                <h3 id="settings-models-title">模型状态</h3>
                <p>这里只显示掩码；Galen 不会把现有 Key 回填到界面。</p>
              </div>
            </div>

            {statuses.length === 0 ? (
              <p className="model-status-empty">尚未配置模型，请在上方保存 API Key。</p>
            ) : (
              <div className="model-status-list">
                {statuses.map((status) => (
                  <div
                    key={status.name}
                    className={`model-status-card ${status.is_default ? "default" : ""}`}
                  >
                    <div className="model-status-card-head">
                      <strong>{status.name}</strong>
                      {status.is_default && (
                        <span className="model-status-default-badge">默认</span>
                      )}
                      <span
                        className={`model-status-key ${status.api_key_present ? "ok" : "missing"}`}
                      >
                        {status.api_key_present
                          ? `已配置 · ${status.api_key_masked}`
                          : "未配置"}
                      </span>
                    </div>
                    <div className="model-status-card-sub">
                      {status.model_id}
                      {status.description ? ` · ${status.description}` : ""}
                    </div>
                    <div className="model-status-card-sub">
                      {status.base_url ?? "未设置服务地址"}
                    </div>
                  </div>
                ))}
              </div>
            )}
          </section>
        </div>

        <div className="settings-footer">
          <span>配置文件：~/.galen/models.toml</span>
          <button className="btn btn-sm btn-ghost" onClick={onOpenWizard}>
            工作区与环境设置
          </button>
        </div>
      </div>
    </div>
  );
}
