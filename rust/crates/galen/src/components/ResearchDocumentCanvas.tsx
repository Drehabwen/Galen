import { useState, useRef, useCallback } from "react";
import { SelectionActionMenu } from "./ui/primitives";

// ---------------------------------------------------------------------------
// Props
// ---------------------------------------------------------------------------
interface ResearchDocumentCanvasProps {
  title?: string;
  content?: string;
  tags?: Array<{ type: "phase" | "status" | "evidence" | "risk"; label: string }>;
  readOnly?: boolean;
  /** Called when user selects text and chooses an AI revision action */
  onRevisionRequest?: (actionId: string, selectedText: string) => void;
}

// ---------------------------------------------------------------------------
// Default sample document
// ---------------------------------------------------------------------------
const SAMPLE_DOC = `# 二甲双胍对 2 型糖尿病患者心血管结局的影响：一项回顾性队列研究

## 方法学

### 研究设计与人群

本研究为单中心回顾性队列研究，纳入 2018 年 1 月至 2023 年 12 月期间
在本院内分泌科确诊的 2 型糖尿病患者。纳入标准包括：(1) 年龄 ≥ 18 岁；
(2) 符合 WHO 1999 年糖尿病诊断标准；(3) 至少接受 6 个月二甲双胍治疗。
排除标准包括：(1) 1 型糖尿病；(2) 妊娠期糖尿病；(3) 严重肝肾功能不全；
(4) 既往心血管事件史。

### 数据收集

从电子病历系统中提取以下数据：人口学特征、病程、合并症、
实验室检查结果（HbA1c、空腹血糖、血脂谱、肾功能）、用药记录、
以及心血管事件发生情况。主要结局指标为主要不良心血管事件（MACE），
包括心血管死亡、非致死性心肌梗死和非致死性卒中。

### 统计分析

连续变量以均值 ± 标准差或中位数（四分位距）表示，分类变量以频数
（百分比）表示。采用 Cox 比例风险模型评估二甲双胍使用与 MACE 发生
风险之间的关联，校正年龄、性别、病程、基线 HbA1c、BMI、血压、
血脂和合并用药等混杂因素。

## 结果

共纳入 1,247 例患者，平均年龄 58.4 ± 11.2 岁，男性占 54.3%。
中位随访时间 3.8 年。在随访期间，共发生 89 例 MACE 事件。
多因素 Cox 回归分析显示，规律使用二甲双胍与 MACE 风险降低显著相关
（HR = 0.72, 95% CI: 0.56–0.91, P = 0.007）。

## 讨论

本研究结果与既往文献一致，进一步支持二甲双胍在 2 型糖尿病患者中的
心血管保护作用。但本研究存在以下局限：单中心设计可能影响外推性；
回顾性设计可能存在选择偏倚和混杂；药物依从性基于处方记录而非
实际服药情况。`;

const SAMPLE_TITLE = "二甲双胍心血管结局研究";

// ---------------------------------------------------------------------------
// Component
// ---------------------------------------------------------------------------
export function ResearchDocumentCanvas({
  title = SAMPLE_TITLE,
  content = SAMPLE_DOC,
  tags = [],
  readOnly = false,
  onRevisionRequest,
}: ResearchDocumentCanvasProps) {
  const [docTitle, setDocTitle] = useState(title);
  const [docContent, setDocContent] = useState(content);
  const [selection, setSelection] = useState<{
    text: string;
    position: { x: number; y: number };
  } | null>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Handle text selection
  const handleSelect = useCallback(() => {
    const el = textareaRef.current;
    if (!el) return;

    const start = el.selectionStart;
    const end = el.selectionEnd;
    const selectedText = el.value.substring(start, end).trim();
    if (!selectedText || selectedText.length < 5) {
      setSelection(null);
      return;
    }

    // Calculate approximate position for the floating menu
    const rect = el.getBoundingClientRect();
    // Use a simple heuristic: position near the middle of the selection
    const lineHeight = 22;
    const textBefore = el.value.substring(0, start);
    const lines = textBefore.split("\n").length;
    const col = start - textBefore.lastIndexOf("\n") - 1;

    setSelection({
      text: selectedText,
      position: {
        x: rect.left + Math.min(col * 8, rect.width - 200),
        y: rect.top + lines * lineHeight - el.scrollTop - 40,
      },
    });
  }, []);

  // Handle revision action
  const handleRevisionAction = useCallback(
    (actionId: string) => {
      if (selection && onRevisionRequest) {
        onRevisionRequest(actionId, selection.text);
      }
      setSelection(null);
    },
    [selection, onRevisionRequest],
  );

  // Dismiss selection menu
  const handleDismissSelection = useCallback(() => {
    setSelection(null);
  }, []);

  return (
    <div className="doc-canvas">
      {/* ── Document toolbar ── */}
      <div className="doc-toolbar">
        <div className="doc-toolbar-left">
          {tags.map((tag, i) => (
            <span key={i} className={`galen-tag galen-tag-${tag.type}`}>
              {tag.label}
            </span>
          ))}
        </div>
        <div className="doc-toolbar-right">
          <span className="doc-stat">字数: {docContent.length}</span>
          <span className="doc-stat">
            保存时间: {new Date().toLocaleTimeString("zh-CN", { hour: "2-digit", minute: "2-digit" })}
          </span>
        </div>
      </div>

      {/* ── Title ── */}
      <input
        className="doc-title-input"
        value={docTitle}
        onChange={(e) => setDocTitle(e.target.value)}
        readOnly={readOnly}
        placeholder="文档标题"
      />

      {/* ── Content area ── */}
      <div className="doc-content-area">
        <textarea
          ref={textareaRef}
          className="doc-content-textarea"
          value={docContent}
          onChange={(e) => setDocContent(e.target.value)}
          onMouseUp={handleSelect}
          onKeyUp={handleSelect}
          readOnly={readOnly}
          placeholder="在此编写或粘贴文档内容... 选中文本可调用 AI 修订。"
        />
      </div>

      {/* ── Selection action menu ── */}
      {selection && (
        <SelectionActionMenu
          position={selection.position}
          onAction={handleRevisionAction}
          onDismiss={handleDismissSelection}
        />
      )}

      {/* ── Footer ── */}
      <div className="doc-footer">
        <span className="doc-footer-stat">
          字数: {docContent.length} · 段落: {docContent.split(/\n\n+/).filter(Boolean).length}
        </span>
        <span className="doc-footer-hint">
          选中文档文本 → AI 修订
        </span>
      </div>
    </div>
  );
}
