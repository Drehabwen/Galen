import { useState } from "react";

interface WelcomeWizardProps {
  initialStep?: number;
  onApiKey: (key: string, defaultModel?: string) => Promise<void>;
  onPickWorkspace: () => Promise<string | null>;
  onTestConnection: () => Promise<string>;
  onDone: () => void;
  hasApiKey: boolean;
  memoryExists?: boolean;
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

const MODELS = [
  { id: "deepseek-v4-pro", label: "DeepSeek V4 Pro", desc: "最强推理，复杂科研任务（默认）" },
  { id: "deepseek-v4-flash", label: "DeepSeek V4 Flash", desc: "快速响应，简单问题" },
] as const;

export function WelcomeWizard({
  initialStep = 0,
  onApiKey,
  onPickWorkspace,
  onTestConnection,
  onDone,
  hasApiKey,
  memoryExists,
  envStatus,
  mcpServers,
}: WelcomeWizardProps) {
  const [step, setStep] = useState(initialStep);
  const [apiKeyInput, setApiKeyInput] = useState("");
  const [selectedModel, setSelectedModel] = useState<string>("deepseek-v4-pro");
  const [apiKeySaved, setApiKeySaved] = useState(false);
  const [testState, setTestState] = useState<TestState>({ kind: "idle" });
  const [workspacePath, setWorkspacePath] = useState<string | null>(null);

  const handleSaveApiKey = async () => {
    const key = apiKeyInput.trim();
    if (!key) return;
    try {
      await onApiKey(key, selectedModel);
      setApiKeyInput("");
      setApiKeySaved(true);
      // 保存后自动测试连接（P1-4）
      setTestState({ kind: "testing" });
      try {
        const message = await onTestConnection();
        setTestState({ kind: "ok", message });
      } catch (e) {
        setTestState({ kind: "fail", message: String(e) });
      }
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

  const handleSkipEntry = () => {
    if (!hasApiKey && !apiKeySaved) {
      const ok = window.confirm(
        "尚未配置 AI 模型，主界面的对话功能将不可用。\n\n确定直接进入吗？（可稍后点击顶栏「模型状态 → 打开设置向导」配置）",
      );
      if (!ok) return;
    }
    onDone();
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
              <p style={{ color: "var(--text-secondary)", marginBottom: 12 }}>
                密钥只保存在本机（~/.galen/models.toml），不会上传到任何服务器。
              </p>
              <div className="welcome-model-picker">
                {MODELS.map((m) => (
                  <button
                    key={m.id}
                    className={`welcome-model-option ${selectedModel === m.id ? "active" : ""}`}
                    onClick={() => setSelectedModel(m.id)}
                  >
                    <strong>{m.label}</strong>
                    <span>{m.desc}</span>
                  </button>
                ))}
              </div>
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

              <div className="welcome-actions" style={{ marginTop: 14 }}>
                <button
                  className="btn btn-primary"
                  onClick={handleSaveApiKey}
                  disabled={!apiKeyInput.trim() || apiKeySaved}
                >
                  {apiKeySaved ? "已保存 ✓" : "保存并测试"}
                </button>
                {apiKeySaved && (
                  <button
                    className="btn"
                    onClick={handleTestConnection}
                    disabled={testState.kind === "testing"}
                  >
                    {testState.kind === "testing" ? "测试中…" : "重新测试"}
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
                  ✗ {testState.message}（可稍后点击「重新测试」）
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
                  {memoryExists && (
                    <div style={{ marginTop: 4, color: "var(--accent-text)" }}>
                      检测到已有项目记忆（GALEN.md），将自动载入上下文。
                    </div>
                  )}
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
                      ["Python（数据分析）", envStatus.python?.installed, "缺失时数据分析工具不可用"],
                      ["R（统计）", envStatus.r?.installed, "缺失时统计脚本不可用"],
                      ["Typst（排版导出）", envStatus.typst?.installed, "缺失时 PDF 排版导出不可用"],
                    ] as [string, boolean, string][]
                  ).map(([name, ok, hint]) => (
                    <span key={name} className={`welcome-env-badge ${ok ? "ok" : ""}`} title={ok ? "已就绪" : hint}>
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
                环境缺失不影响核心功能：Python / R 缺失只影响数据分析，Typst 缺失只影响 PDF 导出，
                可随时安装后重启生效。
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
            <button className="btn btn-ghost" onClick={handleSkipEntry}>
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
                disabled={!apiKeySaved && !hasApiKey}
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
