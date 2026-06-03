interface Props {
  input: string;
  onInputChange: (v: string) => void;
  onSend: () => void;
  sending: boolean;
}

export function StatusBar({ input, onInputChange, onSend, sending }: Props) {
  return (
    <div className="bottom-bar">
      <input
        type="text"
        className="chat-input"
        placeholder="输入消息，Ctrl+Enter 发送..."
        value={input}
        onChange={(e) => onInputChange(e.target.value)}
        onKeyDown={(e) => {
          if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
            onSend();
          }
        }}
      />
      <button
        className="btn btn-primary"
        disabled={sending || !input.trim()}
        onClick={onSend}
      >
        发送
      </button>
    </div>
  );
}
