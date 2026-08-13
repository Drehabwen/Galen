import { useState } from "react";

interface WelcomeWizardProps {
  onApiKey: (key: string) => Promise<void>;
  onPickWorkspace: () => Promise<string | null>;
  onTestConnection: () => Promise<string>;
  onDone: () => void;
  envStatus?: {
    python: { installed: boolean };
    r: { installed: boolean };
    typst: { installed: boolean };
  } | null;
  mcpServers?: Array<{ connected: boolean }>;
}

type TestState =
  | { kind: "idle" }
  | { kind: "testing" }
  | { kind: "ok"; message: string }
  | { kind: "fail"; message: string };

export function WelcomeWizard({
  onApiKey,
  onPickWorkspace,
  onTestConnection,
  onDone,
  envStatus,
  mcpServers,
}: WelcomeWizardProps) {
  const [step, setStep] = useState(0);
  const [apiKeyInput, setApiKeyInput] = useState("");
  const [apiKeySaved, setApiKeySaved] = useState(false);
  const [testState, setTestState] = useState<TestState>({ kind: "idle" });
  const [workspacePath, setWorkspacePath] = useState<string | null>(null);

  const handleSaveApiKey = async () => {
    const key = apiKeyInput.trim();
    if (!key) return;
    try {
      await onApiKey(key);
      setApiKeyInput("");
      setApiKeySaved(true);
      setTestState({ kind: "idle" });
    } catch (e) {
      alert("保存失败: " + String(e));
    }
  };

  const handleTestConnection = async () => {
    setTestState({ kind: "testing" });
    try {
      const message = await onTestConnection();
      setTestState({ kind: "ok", message });
    } catch (e) {
      setTestState({ kind: "fail", message: String(e) });
    }
  };

  const handlePickWorkspace = async () => {
    const path = await onPickWorkspace();
    if (path) {
      setWorkspacePath(path);
    }
  };

  const stepLabels = ["欢迎", "连接模型", "选择工作区", "开始使用"];

  return (
    <div className="cmd-overlay">
      <div className="welcome-card" onClick={(e) => e.stopPropagation()}>
        <div className="welcome-steps">
          {stepLabels.map((label, i) => (
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
              <h2>欢迎使用 Galen</h2>
              <p style={{ color: "var(--text-secondary)", marginBottom: 16 }}>
                面向康复科研的闭环工作台：采集、处理、分析、成文、签核——AI 自主推进，你只做计划把关与最终签核。
              </p>
              <ul style={{ color: "var(--text-secondary)", fontSize: "var(--text-sm)", lineHeight: 2, paddingLeft: 20 }}>
                <li>🔁 任务闭环：计划画布 → 节点自动执行 → 证据回流 → 自动成文</li>
                <li>🧠 科研品味：主编人格 + 装配版科研技能库，而不是通用聊天</li>
                <li>📊 数据驱动：量表 / 评估 / 视频 / 语音统一挂到证据链</li>
              </ul>
            </div>
          )}

          {step === 1 && (
            <div className="welcome-step-content">
              <h2>连接 AI 模型</h2>
              <p style={{ color: "var(--text-secondary)", marginBottom: 16 }}>
                默认使用 DeepSeek V4 Pro。粘贴 API Key 即刻开始，密钥只保存在本机。
              </p>
              <input
                type="password"
                className="welcome-key-input"
                placeholder="粘贴 DeepSeek API Key..."
                value={apiKeyInput}
                onChange={(e) => setApiKeyInput(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && handleSaveApiKey()}
                autoFocus
              />
              <div style={{ display: "flex", alignItems: "center", gap: 8, marginTop: 4 }}>
                <span style={{ fontSize: "var(--text-xs)", color: "var(--text-tertiary)" }}>
                  获取 Key：
                  <a href="https://platform.deepseek.com/api_keys" target="_blank"
                     rel="noreferrer" style={{ color: "var(--accent)", marginLeft: 4 }}>
                    platform.deepseek.com →
                  </a>
                </span>
              </div>

              <div style={{ display: "flex", gap: 8, marginTop: 14 }}>
                <button
                  className="btn btn-primary welcome-btn"
                  onClick={handleSaveApiKey}
                  disabled={!apiKeyInput.trim() || apiKeySaved}
                >
                  {apiKeySaved ? "已保存 ✓" : "保存"}
                </button>
                {apiKeySaved && (
                  <button
                    className="btn welcome-btn"
                    onClick={handleTestConnection}
                    disabled={testState.kind === "testing"}
                  >
                    {testState.kind === "testing" ? "测试中…" : "测试连接"}
                  </button>
                )}
              </div>

              {testState.kind === "ok" && (
                <div style={{ marginTop: 10, color: "var(--success)", fontSize: "var(--text-sm)" }}>
                  ✓ {testState.message}
                </div>
              )}
              {testState.kind === "fail" && (
                <div style={{ marginTop: 10, color: "var(--danger)", fontSize: "var(--text-sm)", wordBreak: "break-all" }}>
                  ✗ {testState.message}
                </div>
              )}
            </div>
          )}

          {step === 2 && (
            <div className="welcome-step-content">
              <h2>选择工作区</h2>
              <p style={{ color: "var(--text-secondary)", marginBottom: 16 }}>
                每个科研项目对应一个文件夹，计划、证据、记忆都会自动保存到里面。
              </p>
              <button
                className="btn btn-primary welcome-btn"
                onClick={handlePickWorkspace}
              >
                {workspacePath ? "重新选择" : "打开工作区"}
              </button>
              {workspacePath && (
                <div style={{ marginTop: 10, fontSize: "var(--text-sm)", color: "var(--success)", wordBreak: "break-all" }}>
                  ✓ {workspacePath}
                </div>
              )}
              <p style={{ fontSize: "var(--text-xs)", color: "var(--text-tertiary)", marginTop: 12 }}>
                也可以稍后点击顶栏「选择工作区」随时切换。
              </p>
            </div>
          )}

          {step === 3 && (
            <div className="welcome-step-content">
              <h2>准备就绪</h2>
              {envStatus && (
                <div className="welcome-env" style={{ margin: "12px 0" }}>
                  {(
                    [
                      ["Python（数据分析）", envStatus.python?.installed],
                      ["R（统计）", envStatus.r?.installed],
                      ["Typst（排版导出）", envStatus.typst?.installed],
                    ] as [string, boolean][]
                  ).map(([name, ok]) => (
                    <span key={name} className={`welcome-env-badge ${ok ? "ok" : ""}`} title={ok ? "已就绪" : "缺失：对应功能暂不可用，不影响核心对话"}>
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
              <p style={{ fontSize: "var(--text-xs)", color: "var(--text-tertiary)" }}>
                环境缺失不影响核心功能：可随时在工作台里继续配置。
              </p>
            </div>
          )}
        </div>

        <div className="welcome-footer">
          {step > 0 ? (
            <button className="btn btn-ghost" onClick={() => setStep((s) => s - 1)}>
              上一步
            </button>
          ) : (
            <button className="btn btn-ghost" onClick={onDone}>
              直接进入
            </button>
          )}
          <div style={{ display: "flex", gap: 8 }}>
            {step === 0 && (
              <button className="btn btn-primary" onClick={() => setStep(1)}>
                开始配置
              </button>
            )}
            {step === 1 && (
              <button
                className="btn btn-primary"
                onClick={() => setStep(2)}
                disabled={!apiKeySaved}
              >
                下一步
              </button>
            )}
            {step === 2 && (
              <button className="btn btn-primary" onClick={() => setStep(3)}>
                {workspacePath ? "下一步" : "跳过，稍后选择"}
              </button>
            )}
            {step === 3 && (
              <button className="btn btn-primary" onClick={onDone}>
                开始使用
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
