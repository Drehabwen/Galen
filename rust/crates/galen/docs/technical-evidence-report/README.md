# Galen 技术佐证报告

本目录保存技术佐证报告的可复现源文件与机器可读验证产物。

## 内容

- `galen-technical-evidence.tex`：LaTeX 主文档。
- `assets/`：真实工作台、连接器导入及论文预览截图。
- `context-memory-probe.json`：三轮连续约束修订探针，9/9 通过。
- `fatigue-scope-probe.json`：运动疲劳范围切换探针，8/8 通过。

## 编译

在本目录执行：

```powershell
latexmk -xelatex -interaction=nonstopmode -halt-on-error `
  -outdir="../../output/pdf" galen-technical-evidence.tex
```

最终文件为 `output/pdf/galen-technical-evidence.pdf`。报告使用 XeLaTeX，需可用的 SimSun、SimHei 与 Times New Roman 字体。
