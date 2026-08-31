import { useMemo, useRef, useState } from "react";

export type WizardMode = "plan" | "auto";

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
  mode?: WizardMode;
  modes?: Array<{ id: string; label: string; description: string }>;
  onSwitchMode?: (mode: WizardMode) => void;
}

type TestState =
  | { kind: "idle" }
  | { kind: "testing" }
  | { kind: "ok"; message: string }
  | { kind: "fail"; message: string; errorClass?: "invalid" | "network" | "unknown" };

const MODELS = [
  { id: "deepseek-v4-flash", label: "DeepSeek V4 Flash", desc: "默认使用，快速响应" },
  { id: "deepseek-v4-pro", label: "DeepSeek V4 Pro", desc: "深度研究，复杂科研任务" },
] as const;

type StepKey = "model" | "workspace" | "mode" | "env";

const STEP_ORDER: StepKey[] = ["model", "workspace", "mode", "env"];

function classifyError(message: string): "invalid" | "network" | "unknown" {
  if (/status\s*401|status\s*403|invalid|unauthorized|forbidden/i.test(message)) {
    return "invalid";
  }
  if (/network|unreachable|timeout|dial|econnrefused|connect/i.test(message)) {
    return "network";
  }
  return "unknown";
}

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
  mode,
  modes = [],
  onSwitchMode,
}: WelcomeWizardProps) {
  const [step, setStep] = useState<StepKey>(STEP_ORDER[initialStep] ?? "model");
  const [apiKeyInput, setApiKeyInput] = useState("");
  const [selectedModel, setSelectedModel] = useState<string>("deepseek-v4-flash");
  const [apiKeySaved, setApiKeySaved] = useState(false);
  const [testState, setTestState] = useState<TestState>({ kind: "idle" });
  const [workspacePath, setWorkspacePath] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement>(null);

  const keyConfigured = hasApiKey || apiKeySaved;
  const currentMode = mode ?? "auto";

  const stepStatus = useMemo(() => {
    const status: Record<StepKey, "done" | "warn" | "todo"> = {
      model: keyConfigured ? "done" : "warn",
      workspace: workspacePath ? "done" : "todo",
      mode: "done",
      env: "todo",
    };
    return status;
  }, [keyConfigured, workspacePath]);

  const handleSaveApiKey = async () => {
    const key = apiKeyInput.trim();
    if (!key) return;
    try {
      await onApiKey(key, selectedModel);
      setApiKeyInput("");
      setApiKeySaved(true);
      setTestState({ kind: "testing" });
      try {
        const message = await onTestConnection();
        setTestState({ kind: "ok", message });
      } catch (e) {
        const message = String(e);
        setTestState({ kind: "fail", message, errorClass: classifyError(message) });
      }
    } catch (e) {
      const message = String(e);
      setTestState({ kind: "fail", message, errorClass: classifyError(message) });
    }
  };

  const handleTestConnection = async () => {
    setTestState({ kind: "testing" });
    try {
      const message = await onTestConnection();
      setTestState({ kind: "ok", message });
    } catch (e) {
      const message = String(e);
      setTestState({ kind: "fail", message, errorClass: classifyError(message) });
    }
  };

  const handlePickWorkspace = async () => {
    const path = await onPickWorkspace();
    if (path) setWorkspacePath(path);
  };

  const handleStart = () => {
    if (!keyConfigured) {
      const ok = window.confirm(
        "尚未配置 AI 模型，主界面对话功能将不可用。\n\n确定直接进入吗？（可稍后点击顶部「模型状态 → 打开设置向导」配置）",
      );
      if (!ok) return;
    }
    onDone();
  };

  const stepTitles: Record<StepKey, string> = {
    model: "连接 AI 模型",
    workspace: "选择工作区",
    mode: "选择工作模式",
    env: "检查科研环境",
  };

  return (
    <div className="cmd-overlay welcome-overlay" role="dialog" aria-modal="true" aria-labelledby="welcome-title">
      <div className="welcome-card" onClick={(e) => e.stopPropagation()}>
        {/* Header */}
        <div className="welcome-header">
          <div>
            <h2 id="welcome-title">欢迎使用 Galen</h2>
            <p className="welcome-header-sub">
              面向康复科研的闭环工作台：采集 → 处理 → 分析 → 成文 → 签核
            </p>
          </div>
          <button className="btn btn-primary" onClick={handleStart}>
            开始使用
          </button>
        </div>

        <div className="welcome-layout">
          {/* Left rail: direct navigation (VS Code style index) */}
          <nav className="welcome-rail" aria-label="设置步骤">
            {STEP_ORDER.map((key) => {
              const status = stepStatus[key];
              return (
                <button
                  key={key}
                  className={`welcome-rail-item ${step === key ? "active" : ""}`}
                  onClick={() => setStep(key)}
                >
                  <span className={`welcome-rail-icon ${status}`}>
                    {status === "done" ? "✓" : status === "warn" ? "!" : "○"}
                  </span>
                  <span className="welcome-rail-label">{stepTitles[key]}</span>
                </button>
              );
            })}
            <div className="welcome-rail-spacer" />
            <button className="welcome-rail-skip" onClick={handleStart}>
              跳过，直接进入
            </button>
          </nav>

          {/* Right: step content */}
          <div className="welcome-body">
            {step === "model" && (
              <div className="welcome-step-content">
                <h3>{stepTitles.model}</h3>
                <p className="welcome-hint">
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

                {keyConfigured ? (
                  <div className="welcome-key-ok">
                    <span className="welcome-key-ok-icon">✓</span>
                    <div>
                      <strong>模型已配置</strong>
                      {apiKeySaved && (
                        <span className="welcome-key-ok-note">
                          本次会话已保存新密钥
                        </span>
                      )}
                    </div>
                  </div>
                ) : (
                  <>
                    <label className="welcome-field-label" htmlFor="welcome-api-key">
                      DeepSeek API Key
                    </label>
                    <input
                      id="welcome-api-key"
                      ref={inputRef}
                      type="password"
                      className="welcome-key-input"
                      placeholder="粘贴 DeepSeek API Key..."
                      value={apiKeyInput}
                      onChange={(e) => {
                        setApiKeyInput(e.target.value);
                        if (testState.kind === "fail") setTestState({ kind: "idle" });
                      }}
                      onKeyDown={(e) => e.key === "Enter" && handleSaveApiKey()}
                      autoComplete="off"
                      spellCheck={false}
                      autoFocus
                    />
                    <button
                      className="btn btn-primary welcome-save-btn"
                      onClick={handleSaveApiKey}
                      disabled={!apiKeyInput.trim() || testState.kind === "testing"}
                    >
                      {testState.kind === "testing" ? "测试连接中…" : "保存并测试连接"}
                    </button>
                  </>
                )}

                {keyConfigured && (
                  <button
                    className="btn btn-ghost welcome-test-btn"
                    onClick={handleTestConnection}
                    disabled={testState.kind === "testing"}
                  >
                    {testState.kind === "testing" ? "测试中…" : "重新测试连接"}
                  </button>
                )}

                {testState.kind === "ok" && (
                  <div className="welcome-msg ok">
                    <span>✓</span> {testState.message}
                  </div>
                )}
                {testState.kind === "fail" && (
                  <div className="welcome-msg fail" role="alert">
                    <span>✕</span>
                    <div>
                      {testState.errorClass === "invalid"
                        ? "密钥无效（401/403），请检查是否复制完整。"
                        : testState.errorClass === "network"
                          ? "网络连接失败，请检查网络后重试。"
                          : testState.message}
                      <div className="welcome-msg-detail">{testState.message}</div>
                    </div>
                  </div>
                )}

                <div className="welcome-links">
                  <a href="https://platform.deepseek.com/api_keys" target="_blank" rel="noreferrer">
                    获取 Key：platform.deepseek.com →
                  </a>
                </div>
              </div>
            )}

            {step === "workspace" && (
              <div className="welcome-step-content">
                <h3>{stepTitles.workspace}</h3>
                <p className="welcome-hint">
                  每个科研项目对应一个文件夹，计划、证据、记忆都会自动保存到里面。
                </p>

                {workspacePath ? (
                  <div className="welcome-workspace-ok">
                    <span className="welcome-key-ok-icon">✓</span>
                    <div>
                      <strong>已选择工作区</strong>
                      <code className="welcome-workspace-path">{workspacePath}</code>
                      {memoryExists && (
                        <div className="welcome-key-ok-note">
                          检测到已有项目记忆（GALEN.md），将自动载入上下文。
                        </div>
                      )}
                    </div>
                  </div>
                ) : (
                  <div className="welcome-workspace-empty">
                    <p>尚未选择。现在选择或稍后从顶部工具栏进入均可。</p>
                  </div>
                )}

                <div className="welcome-actions">
                  <button className="btn btn-primary welcome-btn" onClick={handlePickWorkspace}>
                    {workspacePath ? "重新选择工作区" : "打开工作区"}
                  </button>
                </div>
              </div>
            )}

            {step === "mode" && (
              <div className="welcome-step-content">
                <h3>{stepTitles.mode}</h3>
                <p className="welcome-hint">
                  模式随时可切换（顶部按钮或 Ctrl+1/2/3）。先选一个你常用的起步：
                </p>

                <div className="welcome-mode-list">
                  {modes.length > 0
                    ? modes.map((m) => (
                        <button
                          key={m.id}
                          className={`welcome-mode-option ${currentMode === m.id ? "active" : ""}`}
                          onClick={() => onSwitchMode?.(m.id as WizardMode)}
                        >
                          <strong>{m.label}</strong>
                          <span>{m.description}</span>
                        </button>
                      ))
                    : (
                        (["auto", "plan"] as const).map((id) => (
                          <button
                            key={id}
                            className={`welcome-mode-option ${currentMode === id ? "active" : ""}`}
                            onClick={() => onSwitchMode?.(id)}
                          >
                            <strong>
                              {id === "plan" ? "计划" : "自动"}
                            </strong>
                            <span>
                              {id === "plan"
                                  ? "制定方案：列出步骤，确认后执行"
                                  : "自主执行：自动拆解目标，并行执行，汇总产出"}
                            </span>
                          </button>
                        ))
                      )}
                </div>

                <p className="welcome-note">
                  当前模式：<strong>{currentMode}</strong>。
                </p>
              </div>
            )}

            {step === "env" && (
              <div className="welcome-step-content">
                <h3>{stepTitles.env}</h3>
                <p className="welcome-hint">环境缺失不影响核心功能，可随时安装后重启生效。</p>

                {envStatus && (
                  <div className="welcome-env">
                    {(
                      [
                        ["Python（数据分析）", envStatus.python?.installed, "缺失时数据分析工具不可用"],
                        ["R（统计）", envStatus.r?.installed, "缺失时统计脚本不可用"],
                        ["Typst（排版导出）", envStatus.typst?.installed, "缺失时 PDF 排版导出不可用"],
                      ] as [string, boolean, string][]
                    ).map(([name, ok, hint]) => (
                      <span
                        key={name}
                        className={`welcome-env-badge ${ok ? "ok" : ""}`}
                        title={ok ? "已就绪" : hint}
                      >
                        {ok ? "✓" : "✕"} {name}
                      </span>
                    ))}
                    {mcpServers && mcpServers.length > 0 && (
                      <span className="welcome-env-badge ok">
                        MCP {mcpServers.filter((s) => s.connected).length}/{mcpServers.length}
                      </span>
                    )}
                  </div>
                )}

                <ul className="welcome-env-legend">
                  <li>Python / R 缺失只影响数据分析</li>
                  <li>Typst 缺失只影响 PDF 导出</li>
                  <li>一切都可以边用边装，不阻塞研究</li>
                </ul>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
