# Agent Guidelines (AGENTS.md)

If you are an AI assistant (like Claude Code or Antigravity) working in this repository, you must read and follow these rules.

## 1. Version Control via Jujutsu (`jj`)

The user uses **Jujutsu (`jj`)** for version control.

- **Do NOT use `git` commands** (e.g., `git commit`, `git add`, `git checkout`).
- Use `jj` for repository operations. Every Jujutsu command displaying output (specifically `log`, `status`, `diff`, and `bookmark` commands) **MUST** use the `--no-pager` global option to prevent terminal interactive hangs (e.g., `jj --no-pager status`, `jj --no-pager diff`, `jj --no-pager log`).
- Jujutsu automatically tracks file changes as you write them. When you want to set a commit message for the active change, use:
  ```bash
  jj --no-pager describe -m "Your descriptive commit message"
  ```
- If you need to start a new logical change, use `jj new`.

## 4. Coding & Cleanliness

- Do not introduce unnecessary dependencies. Keep the codebase lightweight.
