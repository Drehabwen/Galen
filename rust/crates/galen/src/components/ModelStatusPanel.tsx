import type { ModelStatus } from "../types";

interface ModelStatusPanelProps {
  statuses: ModelStatus[];
  onClose: () => void;
}

export function ModelStatusPanel({ statuses, onClose }: ModelStatusPanelProps) {
  return (
    <div className="cmd-overlay" onClick={onClose}>
      <div className="model-status-panel" onClick={(e) => e.stopPropagation()}>
        <div className="model-status-header">
          <h2>模型与密钥状态</h2>
          <button className="btn btn-ghost btn-sm" onClick={onClose}>
            ✕
          </button>
        </div>

        {statuses.length === 0 ? (
          <p className="model-status-empty">
            未配置任何模型。请在欢迎向导中保存 API Key，或编辑 ~/.galen/models.toml。
          </p>
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
                      ? `密钥已配置 · ${status.api_key_masked}`
                      : "密钥未配置"}
                  </span>
                </div>
                <div className="model-status-card-sub">
                  {status.model_id}
                  {status.description ? ` · ${status.description}` : ""}
                </div>
                <div className="model-status-card-sub">
                  {status.base_url ?? "（无 base_url）"}
                </div>
              </div>
            ))}
          </div>
        )}

        <p className="model-status-hint">配置文件：~/.galen/models.toml</p>
      </div>
    </div>
  );
}
