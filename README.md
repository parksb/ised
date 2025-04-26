# ised

> WARNING: This project is under active development. Use only in environments where changes can be easily undone (e.g., version-controlled directories).

![A terminal window running a Rust project using the cargo tool. The interface shows a file list on the left and a visual diff of test file changes on the right. The diff highlights changes from importing highlight_diff_lines to diff::highlight_lines, and updates to function calls in several test cases accordingly. At the bottom, filter and navigation options are visible with key hints.](images/ised.png)

**ised** (interactive sed) helps you search and replace text across large sets of files using regular expressions. It provides a live preview of changes, lets you navigate through affected files, and gives you full control over whether changes are applied—one by one or all at once.

- **Live, interactive preview**: View diffs for each match in real time, similar to `git diff`. Decide what to change before writing anything to disk.
- **Bulk editing with confirmation**: Apply changes to the currently selected file, or confirm and apply changes to all matching files at once.
- **Regex-based workflow**: Use regular expressions to filter files and match content. Supports flexible patterns for large-scale refactoring.
- **Safe by default**: No changes are applied without confirmation. Accidental replacements are avoided entirely.
- **Minimal and focused**: Designed to do one job well. No external dependencies. Runs entirely in your terminal.

## Installation

If you are on macOS, you can install ised using Homebrew.

```sh
$ brew install parksb/x/ised
```

If you have Rust installed, you can install ised directly from crates.io.

```sh
$ cargo install ised
```

Prebuilt binaries for Linux and macOS are available on the [GitHub Releases](https://github.com/parksb/ised/releases) page. Download the appropriate archive for your platform, extract it, and move the binary into your `PATH`.

```sh
$ tar -xzf ised-x86_64-unknown-linux-gnu.tar.gz # for Linux x86_64. if you are on another platform, use the appropriate archive.
$ mv ised-x86_64-unknown-linux-gnu /usr/local/bin/ised
```

Alternatively, you can build from source manually.

```sh
$ git clone https://github.com/parksb/ised.git
$ cd ised
$ cargo build --release
$ ./target/release/ised
```

## Layout

ised splits the screen into five main regions:

| Section             | Description |
|---------------------|-------------|
| File List       | Displays a list of files (recursively from the current directory) matching your filters. Use ↑/↓ or `j`/`k` to move between files. |
| Glob Filter     | Enter a glob pattern to narrow down which files are shown in the File List. |
| Diff            | Shows a live `git diff`-style preview of what will change in the selected file. Scroll with ↑/↓ or `j`/`k`. |
| From            | Enter a regular expression pattern here. Files without a match will disappear from the File List. |
| To              | Enter a replacement string. Captured groups (e.g. `$1`, `$2`) are supported and substituted accordingly. |

## Keyboard Shortcuts

| Shortcut          | Action |
|-------------------|--------|
| `Tab`             | Cycle focus between regions |
| `Ctrl+L`          | Focus on **File [L]ist** |
| `Ctrl+G`          | Focus on **[G]lob Filter** (Glob) |
| `Ctrl+D`          | Focus on **[D]iff** |
| `Ctrl+F`          | Focus on **[F]rom** (Regex) |
| `Ctrl+T`          | Focus on **[T]o** (Replacement) |
| `Enter`           | Confirm and apply change to the selected file |
| `Ctrl+A`          | Confirm and apply changes to all matching files |
| `Ctrl+C`          | Quit ised safely |

## Replacement

- The `<From>` field accepts any valid regex (via [`regex`](https://docs.rs/regex/)).
- If your regex contains capture groups, the replacement will only affect the matched group, not the entire match.
  - `<From>`: `highlight_(match|diff)`  
  - `<To>`: `new`  
  - Input: `highlight_match` → Output: `highlight_new`
- You can also use `$1`, `$2`, etc. in `<To>` to refer to capture groups:
  - `<From>`: `(\d+)\s+(\w+)`  
  - `<To>`: `$2:$1`  
  - Input: `123 abc` → Output: `abc:123`

## Configuration

You can define default filters and behaviors in an optional config file `ised.config.toml`. These are searched starting from the current directory and walking upward to the root, stopping at the first match.

```toml
[files]
glob_filter = [
  "!**/.git/**",
  "*.rs"
]
```

- `files.glob_filter`: A list of glob patterns used to pre-filter files on launch. Use `!` prefix to exclude files (e.g., `!**/*.md`). Multiple patterns are joined with `,` at runtime (i.e. `*.rs,!**/mod.rs`)
- More configuration options may be introduced in the future, including key bindings, ignored patterns, ...

## License

This project is licensed under the terms of the [AGPL-3.0](LICENSE) license.
