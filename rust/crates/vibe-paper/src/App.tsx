import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useChat } from "./hooks/useChat";
import { ChatPanel } from "./components/ChatPanel";
import { PaperPanel } from "./components/PaperPanel";
import { WorkspaceTree } from "./components/WorkspaceTree";
import { TabBar } from "./components/TabBar";
import { StatusBar } from "./components/StatusBar";
import type { ModelConfig, LeftTab } from "./types";

export default function App() {
  const chat = useChat();
  const [input, setInput] = useState("");
  const [models, setModels] = useState<ModelConfig[]>([]);
  const [model, setModel] = useState("");
  const [leftTab, setLeftTab] = useState<LeftTab>("workspace");
  const [wsRoot, setWsRoot] = useState<string | null>(null);
  const [wsPicker, setWsPicker] = useState(false);
  const [wsInput, setWsInput] = useState("");

  // Load models and workspace on mount
  useEffect(() => {
    invoke<ModelConfig[]>("get_models").then(setModels).catch(console.error);
    invoke<string | null>("get_workspace_root").then(setWsRoot).catch(console.error);
  }, []);

  useEffect(() => {
    if (!model && models.length > 0) setModel(models[0].name);
  }, [models]);

  const handleSend = () => {
    if (!input.trim() || chat.sending) return;
    chat.send(input, model || "sonnet");
    setInput("");
  };

  const handleSetWorkspace = async () => {
    const path = wsInput.trim();
    if (!path) return;
    try {
      await invoke("set_workspace", { path });
      setWsRoot(path);
      setWsPicker(false);
      setWsInput("");
    } catch (e) {
      alert(String(e));
    }
  };

  return (
    <div className="app-container">
      {/* Top Bar */}
      <div className="top-bar">
        <span className="app-title">🦞 VIBE Paper</span>

        <div className="top-bar-section">
          <span className="top-label">模型:</span>
          <select
            value={model}
            onChange={(e) => setModel(e.target.value)}
            className="model-select"
          >
            {models.map((m) => (
              <option key={m.name} value={m.name}>
                {m.name}
              </option>
            ))}
          </select>
        </div>

        <div className="top-bar-section">
          {wsRoot ? (
            <span className="ws-label" title={wsRoot}>
              📂 {wsRoot.split(/[/\\]/).pop()}
            </span>
          ) : null}
          <button
            className="btn btn-sm"
            onClick={() => setWsPicker(!wsPicker)}
          >
            📁 {wsRoot ? "切换" : "选择工作区"}
          </button>
        </div>

        <div className="top-bar-spacer" />

        {chat.sending && <span className="thinking-indicator">思考中...</span>}
        <button className="btn btn-sm" onClick={chat.clear}>
          新对话
        </button>
      </div>

      {/* Workspace Picker */}
      {wsPicker && (
        <div className="ws-picker-bar">
          <span>工作区路径:</span>
          <input
            type="text"
            className="ws-input"
            placeholder="D:\research\my-project"
            value={wsInput}
            onChange={(e) => setWsInput(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && handleSetWorkspace()}
          />
          <button className="btn btn-sm" onClick={handleSetWorkspace}>
            确定
          </button>
          <button className="btn btn-sm" onClick={() => setWsPicker(false)}>
            取消
          </button>
        </div>
      )}

      {/* Main content */}
      <div className="main-content">
        {/* Left Panel */}
        <div className="left-panel">
          <TabBar active={leftTab} onChange={setLeftTab} />
          <div className="left-panel-content">
            {leftTab === "workspace" && (
              <WorkspaceTree
                wsRoot={wsRoot}
                files={chat.wsFileList}
                content={chat.wsFileContent}
                onReadFile={(path) => {
                  chat.send(`请读取工作区文件: ${path}`, model || "sonnet");
                }}
              />
            )}
            {leftTab === "papers" && (
              <PaperPanel papers={chat.searchResults} />
            )}
            {leftTab === "notes" && (
              <div className="placeholder">
                <p>笔记功能即将推出</p>
                <p>AI 可以将研究笔记保存到工作区</p>
              </div>
            )}
          </div>
        </div>

        {/* Right Panel - Chat */}
        <div className="chat-panel">
          <ChatPanel
            messages={chat.messages}
            streaming={chat.streaming}
            sending={chat.sending}
            error={chat.error}
          />
        </div>
      </div>

      {/* Bottom Bar */}
      <StatusBar
        input={input}
        onInputChange={setInput}
        onSend={handleSend}
        sending={chat.sending}
      />
    </div>
  );
}
