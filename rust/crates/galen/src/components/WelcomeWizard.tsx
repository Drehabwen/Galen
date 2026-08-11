import { useState } from "react";

interface WelcomeWizardProps {
  onApiKey: (key: string) => Promise<void>;
  onPickWorkspace: () => Promise<void>;
  onDone: () => void;
  envStatus?: {
    python: { installed: boolean };
    r: { installed: boolean };
    typst: { installed: boolean };
  } | null;
  mcpServers?: Array<{ connected: boolean }>;
}

export function WelcomeWizard({
  onApiKey,
  onPickWorkspace,
  onDone,
  envStatus,
  mcpServers,
}: WelcomeWizardProps) {
  const [step, setStep] = useState(0);
  const [apiKeyInput, setApiKeyInput] = useState("");
  const [apiKeySaved, setApiKeySaved] = useState(false);

  const handleSaveApiKey = async () => {
    const key = apiKeyInput.trim();
    if (!key) return;
    try {
      await onApiKey(key);
      setApiKeyInput("");
      setApiKeySaved(true);
      setTimeout(() => setStep(1), 600);
    } catch (e) {
      alert("保存失败: " + String(e));
    }
  };

  return (
    <div className="cmd-overlay" onClick={onDone}>
      <div className="welcome-card" onClick={(e) => e.stopPropagation()}>
        <div className="welcome-steps">
          {["连接模型", "选择工作区", "开始使用"].map((label, i) => (
            <div
              key={i}
              className={`welcome-step-dot ${i === step ? "active" : ""} ${i < step ? "done" : ""}`}
            >
              <span className="welcome-step-num">{i < step ? "✓" : i + 1}</span>
              <span className="welcome-step-label">{label}</span>
            </div>
          ))}
        </div>

        <div className="welcome-body">
          {step === 0 && (
            <div className="welcome-step-content">
              <h2>连接 AI 模型</h2>
              <p style={{ color: "var(--text-secondary)", marginBottom: 20 }}>
                粘贴 API Key 即刻开始。
              </p>
              <input
                type="password"
                className="welcome-key-input"
                placeholder="粘贴你的 API Key..."
                value={apiKeyInput}
                onChange={(e) => setApiKeyInput(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && handleSaveApiKey()}
                autoFocus
              />
              <span style={{ fontSize: "var(--text-xs)", color: "var(--text-tertiary)" }}>
                支持 DeepSeek、OpenAI 等兼容 API。
                <a href="https://platform.deepseek.com/api_keys" target="_blank"
                   style={{ color: "var(--accent)", marginLeft: 4 }}>
                  获取 Key →
                </a>
              </span>
              <button
                className="btn btn-primary welcome-btn"
                onClick={handleSaveApiKey}
                disabled={!apiKeyInput.trim() || apiKeySaved}
              >
                {apiKeySaved ? "已保存" : "保存"}
              </button>
            </div>
          )}

          {step === 1 && (
            <div className="welcome-step-content">
              <h2>选择工作区</h2>
              <p style={{ color: "var(--text-secondary)", marginBottom: 20 }}>
                每个项目对应一个文件夹。
              </p>
              <button
                className="btn btn-primary welcome-btn"
                onClick={() => onPickWorkspace().then(() => setStep(2))}
              >
                打开工作区
              </button>
            </div>
          )}

          {step === 2 && (
            <div className="welcome-step-content">
              <h2>准备就绪</h2>
              {envStatus && (
                <div className="welcome-env" style={{ margin: "12px 0" }}>
                  {(
                    [
                      ["Python", envStatus.python?.installed],
                      ["R", envStatus.r?.installed],
                      ["Typst", envStatus.typst?.installed],
                    ] as [string, boolean][]
                  ).map(([name, ok]) => (
                    <span key={name} className={`welcome-env-badge ${ok ? "ok" : ""}`}>
                      {ok ? "✓" : "✗"} {name}
                    </span>
                  ))}
                  {mcpServers && mcpServers.length > 0 && (
                    <span className="welcome-env-badge ok">
                      MCP {mcpServers.filter((s) => s.connected).length}/{mcpServers.length}
                    </span>
                  )}
                </div>
              )}
              <button className="btn btn-primary welcome-btn" onClick={onDone}>
                开始使用
              </button>
            </div>
          )}
        </div>

        <div className="welcome-footer">
          <button className="btn btn-ghost" onClick={onDone}>跳过</button>
          <div style={{ display: "flex", gap: 8 }}>
            {step > 0 && (
              <button className="btn btn-ghost" onClick={() => setStep((s) => s - 1)}>
                上一步
              </button>
            )}
            {step < 2 && (
              <button
                className="btn btn-primary"
                onClick={() => step === 0 && apiKeySaved ? setStep(1) : step === 1 && setStep(2)}
                disabled={step === 0 && !apiKeySaved}
              >
                下一步
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
