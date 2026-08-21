# Annoterm

Review Markdown in the terminal. Send precise feedback to Codex, Claude Code, or another coding agent.

[![Rust 1.88+](https://img.shields.io/badge/Rust-1.88%2B-000000?logo=rust)](https://www.rust-lang.org/)
[![CI](https://github.com/filipgutica/annoterm/actions/workflows/ci.yml/badge.svg)](https://github.com/filipgutica/annoterm/actions/workflows/ci.yml)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

Annoterm renders Markdown, lets you attach comments to blocks or source selections, and turns open comments into a ready-to-paste prompt.

Your document stays clean. Annoterm stores comments in a local JSON sidecar and re-anchors them when the source moves.

## Why Annoterm?

- Read CommonMark and GitHub Flavored Markdown without leaving the terminal.
- Edit and save the source when you need to make a direct change.
- Comment on a rendered block or an exact raw-text selection.
- Generate feedback with file paths, line numbers, quoted text, and stable comment IDs.
- Keep the full workflow local. Annoterm needs no account, server, database, or model API.

Annoterm supports macOS and Linux.

## Quick start

Install Rust 1.88 or newer. Then clone and install Annoterm:

```sh
git clone https://github.com/filipgutica/annoterm.git
cd annoterm
cargo install --locked --path .
```

Open a Markdown file:

```sh
annoterm docs/design.md
```

Annoterm starts in rendered mode. Use the arrow keys, a mouse wheel, or a left click to select a block. Then press `a` to comment on it.

## A typical review

1. Open the document with `annoterm <file.md>`.
2. Select a rendered block with `Up`, `Down`, or a left click.
3. Press `a`, write the comment, and press `Enter`.
4. Repeat for each issue.
5. Paste the generated feedback prompt into your coding agent.

While you write a comment, use `Left`, `Right`, `Home`, or `End` to move the cursor. Hold `Option` or `Ctrl` with `Left` or `Right` to move between words. `Backspace` removes the previous character.

Annoterm copies the updated prompt after each comment change. When you quit with open annotations, Annoterm copies the prompt again and reports the result. Run this command if another application replaces your clipboard:

```sh
annoterm copy-feedback docs/design.md
```

You can also export the prompt:

```sh
annoterm export docs/design.md --output feedback.md
```

Add `--force` to replace an existing export.

## What the agent receives

Annoterm turns each open comment into structured Markdown:

```md
# Annoterm feedback

Document: `docs/design.md`

Update the document to address every open annotation. Preserve unrelated content.

## Annotation `019c...`

Location: lines 18-20
State: anchored

Selected text:

> Retries use a fixed five-second delay.

Comment:

Explain why this uses a fixed delay instead of exponential backoff.
```

Stable annotation IDs let the agent identify each requested change in its response.

## Rendered and raw modes

Rendered mode is read-only. It supports headings, lists, task lists, tables, links, blockquotes, fenced code, footnotes, and syntax highlighting.

A solid dot (`●`) marks an exact current anchor. A hollow dot (`◌`) marks an outdated comment at an approximate location.

Press `Ctrl+R` to open raw mode. Raw mode edits the Markdown source directly.

To comment on an exact source range:

1. Press `Ctrl+R`.
2. Select text with `Shift` and the arrow keys.
3. Press `Ctrl+K`.
4. Write the comment and press `Enter`.

Press `Ctrl+S` to save raw changes. Annoterm saves the document atomically and stops if another process changed the file.

## Comment controls

In rendered mode, comment controls work immediately. In raw mode, press `Ctrl+W` to focus the Comments panel first.

Annoterm shows shortcuts for the active pane in one bar at the bottom of the terminal. Press `?` in rendered mode or the Comments panel to show all shortcuts. Press `F1` in raw mode or while you write a comment so that you can insert `?` as text.

| Key | Action |
| --- | --- |
| `Up`, `Down` or `[`, `]` | Select a comment |
| Mouse wheel or trackpad | Move through comments when the panel is focused |
| `j` | Jump to the selected comment |
| `e` | Edit the selected comment |
| `x` | Resolve or reopen the selected comment |
| `d` | Delete the selected comment |
| `o` | Repair an outdated or detached comment at the current selection |
| `Ctrl+W` or `Esc` | Return focus to the document |
| `?` | Show all shortcuts |

The Comments panel shows the active focus and keeps the selected comment visible.
Open comments appear in generated feedback. Resolved comments stay in the sidecar but do not appear in feedback.

## Document controls

| Key | Action |
| --- | --- |
| `Ctrl+R` | Toggle rendered and raw modes |
| `Ctrl+S` | Save in raw mode |
| Arrow keys | Move the raw cursor |
| `Shift` and arrow keys | Select source text in raw mode |
| `Ctrl+Z`, `Ctrl+Y` | Undo or redo a raw edit |
| `F1` | Show all shortcuts in raw mode |
| `Down`, `Tab` | Select the next rendered block |
| `Up`, `Shift+Tab` | Select the previous rendered block |
| Left click | Select a rendered block without scrolling |
| `a` | Comment on the selected rendered block |
| `Ctrl+K` | Comment on the raw selection or rendered block |
| `q` | Quit from the rendered document |
| `Esc` | Return focus or quit |

## Local annotation files

Annoterm stores comments outside the Markdown source in your home directory. The default sidecar for `docs/guide.md` is:

```text
~/.annoterm/<sha256-of-canonical-document-path>.annotations.json
```

The filename is a SHA-256 hash of the canonical document path. This keeps your Git worktree clean.

The sidecar stores anchors, comments, status, and source snapshots. Annoterm deduplicates snapshots by document fingerprint and removes unreferenced versions. On Unix, the default `~/.annoterm` directory is mode `0700`, and its sidecar and lock files are mode `0600`.

On interactive open, Annoterm checks for a legacy project sidecar. If no user-local sidecar exists, Annoterm copies that file. It leaves the legacy file unchanged. Remove the old `.annoterm` directory after you verify the copy.

Annoterm first searches for a safe current match. If that fails, it maps the old range through the saved source snapshot when the diff retains an unchanged boundary. The comment becomes outdated and remains jumpable at an approximate location.

Version 1 sidecars have no snapshots. Annoterm uses the last known line and starts snapshot tracking from the current document.

If the document was completely replaced or has no other usable location, the comment becomes detached. Select new text and press `o` to repair it.

To use a shareable sidecar in a repository, choose its path explicitly:

```sh
annoterm docs/guide.md --annotations reviews/guide.annotations.json
```

Explicit sidecars are safe to commit when their comments belong in the repository. See [the annotation format](docs/annotation-format.md) for the full schema.

## Markdown and terminal support

Annoterm uses visible text fallbacks when a terminal cannot display a Markdown feature:

- Images become `[image: alt text - URL]`.
- Raw HTML stays escaped and never runs.
- Math shows its source expression.
- Mermaid shows a notice and the fenced source.
- Unknown code languages use plain text.

Annoterm supports color and plain terminal styles. Set `NO_COLOR` to disable colors.

See [terminal support](docs/terminal-support.md) for the complete fallback rules.

## Use Annoterm from Finder on macOS

Build a host-only app bundle for your current Mac:

```sh
./scripts/build-macos-app.sh --host-only
./scripts/install-macos-app.sh
```

Then make Annoterm the default Markdown application:

1. Select a `.md` file in Finder and choose **File > Get Info**.
2. Select **Annoterm** under **Open with**.
3. Select **Change All**.

The local bundle opens the selected file in Terminal.app. See [the macOS installation guide](docs/macos-installation.md) for universal builds and permissions.

## Safety and storage

- Annoterm saves Markdown and sidecars atomically.
- It checks for external file and sidecar changes before replacement.
- The clipboard is a convenience, not persistent storage.
- The application does not send documents or comments over the network.

## Build and test

Build a release binary:

```sh
cargo build --locked --release
```

Run the project checks:

```sh
cargo fmt --all --check
cargo test --locked --all-targets --all-features
cargo clippy --locked --all-targets --all-features -- -D warnings
```

See [the architecture guide](docs/architecture.md) for module boundaries and data flow.

## Current limits

- Raw editing has bounded in-memory undo and redo, but no search or configurable keymap.
- Rendered comments cover complete top-level blocks. Raw comments can cover arbitrary UTF-8 selections.
- External changes stop a save. Annoterm does not include an interactive merge view.
- Finder integration uses Terminal.app. The local app bundle is ad hoc signed and is not notarized.

## License

Annoterm is available under the [MIT License](LICENSE).
