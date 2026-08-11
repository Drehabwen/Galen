# Galen LLM, Context, and Tools Configuration

## Runtime

Galen's Agent is a Tauri-backed feature. `npm run dev` only starts the Vite web preview and cannot call Rust commands such as `send_message`, `get_models`, or workspace tools.

Use the desktop runtime for Agent work:

```powershell
cd D:\DEV\toolchains\claw-code\rust\crates\galen
npm run tauri dev
```

The browser preview at `http://127.0.0.1:1420/` is useful for UI work only.

## Model Config

Model discovery is implemented by `rust/crates/model-router`.

Lookup order:

1. `models.toml` in the current process working directory
2. `%USERPROFILE%\.claw\models.toml`
3. `%USERPROFILE%\models.toml`

Recommended OpenAI-compatible configuration:

```toml
[router]
default = "deepseek"
fast = "deepseek"
analysis = "deepseek"

[models.deepseek]
provider = "openai_compat"
api_key = "YOUR_API_KEY"
model_id = "deepseek-v4-pro"
base_url = "https://api.deepseek.com/v1"
description = "Default clinical research agent"
max_tokens = 4096
```

For OpenAI:

```toml
[router]
default = "gpt"
fast = "gpt"
analysis = "gpt"

[models.gpt]
provider = "openai_compat"
api_key = "YOUR_OPENAI_API_KEY"
model_id = "gpt-4.1"
base_url = "https://api.openai.com/v1"
max_tokens = 4096
```

Environment variables also work when `api_key` is omitted:

- `OPENAI_API_KEY` for `openai` or `openai_compat`
- `ANTHROPIC_API_KEY` for `anthropic`
- `DASHSCOPE_API_KEY` for DashScope-compatible Qwen/Kimi
- `XAI_API_KEY` for xAI

## Current Chat Pipeline

Frontend:

- `src/hooks/useChat.ts` calls Tauri command `send_message`.
- `src/App.tsx` loads model aliases through `get_models`.
- `ResearchWorkbench` sends workspace-aware prompts built from the selected package file list.

Backend:

- `src-tauri/src/commands.rs` receives `send_message`.
- `src-tauri/src/backend.rs` resolves the model, builds the client, injects the system prompt, attaches tools, streams text, executes tool calls, and emits UI events.
- `src-tauri/src/tools/mod.rs` registers built-in tools.

## Context Engineering

Do not send the whole project folder blindly. Build a deterministic context pack per task.

Recommended context layers:

1. Study manifest: package name, folders, file inventory, detected artifact types.
2. Task intent: capture, QC, cleaning, statistics, literature, writing, or collaboration.
3. Selected artifacts: only files the user selected or the workflow requires.
4. Derived summaries: schema, codebook, missingness, descriptive stats, query list, version log.
5. Output contract: what file or response should be produced, and whether it is a draft, report, script, or task list.

Implementation target:

- Add a `research_context` module under `src-tauri/src/`.
- Build a `ResearchContextPack` before `run_chat`.
- Add it as a system or user context message before the user's request.
- Keep raw dataset rows out of default context; use tools to inspect files on demand.

Suggested structure:

```rust
pub struct ResearchContextPack {
    pub package_root: PathBuf,
    pub manifest_summary: String,
    pub selected_files: Vec<PathBuf>,
    pub artifact_summary: String,
    pub task_policy: String,
}
```

## Tools

Current built-in tools:

- Literature: `search_pubmed`, `fetch_article`, `format_citation`
- Workspace files: `create_directory`, `write_file`, `read_file`, `list_files`, `save_paper`, `delete_file`, `delete_directory`, `move_file`
- Search: `search_files`
- Execution: `execute_command`

Registering tools:

1. Add a file under `src-tauri/src/tools/`, for example `research.rs`.
2. Implement the `Tool` trait.
3. Add `pub mod research;` in `tools/mod.rs`.
4. Register the tool in `ToolRegistry::register_builtin`.

Recommended research MVP tools:

- `inspect_dataset_schema`: read CSV/XLSX headers, infer variable types, row count.
- `profile_dataset`: missingness, unique values, numeric ranges, obvious outliers.
- `compare_codebook_dataset`: compare codebook variables with dataset columns.
- `create_cleaning_plan`: generate a versioned cleaning plan file.
- `run_stats_script`: execute R/Python scripts in the workspace with logged output.
- `build_table1`: generate Table 1 draft from a selected dataset and grouping variable.
- `write_research_note`: write task notes, query logs, or methods drafts to the package.

## Product Rule

The Agent is not a chat feature bolted onto the side. It should sit behind concrete actions:

- Check package readiness
- Read selected file
- Compare dataset with codebook
- Generate cleaning plan
- Run analysis
- Produce writing material
- Create collaboration handoff

Each action should specify the context pack and allowed tools explicitly.
