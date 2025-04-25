# ised

**ised** (interactive sed) helps you search and replace text across large sets of files using regular expressions. It provides a live preview of changes, lets you navigate through affected files, and gives you full control over whether changes are applied—one by one or all at once.

- Live, interactive preview: View diffs for each match in real time, similar to `git diff`. Decide what to change before writing anything to disk.
- Bulk editing with confirmation: Apply changes to the currently selected file, or confirm and apply changes to all matching files at once.
- Regex-based workflow: Use regular expressions to filter files and match content. Supports flexible patterns for large-scale refactoring.
- Safe by default: No changes are applied without confirmation. Accidental replacements are avoided entirely.
- Minimal and focused: Designed to do one job well. No external dependencies. Runs entirely in your terminal.
