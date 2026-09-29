# Annoterm

Review Markdown in the terminal. Send precise feedback to Codex, Claude Code, or another coding agent.

[![Release](https://img.shields.io/github/v/release/filipgutica/annoterm?color=2563eb)](https://github.com/filipgutica/annoterm/releases)
[![CI](https://github.com/filipgutica/annoterm/actions/workflows/ci.yml/badge.svg)](https://github.com/filipgutica/annoterm/actions/workflows/ci.yml)
[![Rust 1.88+](https://img.shields.io/badge/Rust-1.88%2B-000000?logo=rust)](Cargo.toml)
[![macOS and Linux](https://img.shields.io/badge/platform-macOS%20%7C%20Linux-555555)](docs/terminal-support.md)
[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

**[Website](https://filipgutica.github.io/annoterm/)** · **[Quick start](#quick-start)** · **[Controls](#document-controls)** · **[Architecture](docs/architecture.md)**

Annoterm renders Markdown, lets you attach comments to blocks or source selections, and turns open comments into a ready-to-paste prompt.

Your document stays clean. Annoterm stores comments in a local JSON sidecar and re-anchors them when the source moves.

## What you can do

- Read CommonMark and GitHub Flavored Markdown without leaving the terminal.
- Edit and save the source when you need to make a direct change.
- Comment on a rendered block or an exact raw-text selection.
- Generate feedback with file paths, line numbers, quoted text, and stable comment IDs.
- Keep the full workflow local. Annoterm needs no account, server, database, or model API.

Annoterm supports macOS and Linux.

## Quick start

Install with Homebrew on macOS or Linux:

```sh
brew install filipgutica/tap/annoterm
```

Open a Markdown file:

```sh
annoterm README.md
```

Annoterm starts in rendered mode. Use `Up`, `Down`, a mouse wheel, or a left click to select a block. Press `a` to comment.

Press `q` when you finish. Annoterm copies the open comments to your clipboard as a prompt for your coding agent.

## A typical review

1. Open the document with `annoterm <file.md>`.
2. Select a rendered block with `Up`, `Down`, or a left click.
3. Press `a`, write the comment, and press `Enter`.
4. Repeat for each issue.
5. Press `q` to quit and copy the current feedback.
6. Paste the feedback into your coding agent.

While you write a comment, use `Left`, `Right`, `Home`, or `End` to move the cursor. Hold `Option` or `Ctrl` with `Left` or `Right` to move between words. `Backspace` removes the previous character.

Annoterm copies the updated prompt after each comment change. When you quit with open annotations, Annoterm copies the prompt again and reports the result. Run this command if another application replaces your clipboard:

```sh
annoterm copy-feedback README.md
```

You can also export the prompt:

```sh
annoterm export README.md --output feedback.md
```

Add `--force` to replace an existing export.

On Linux, install `wl-copy` for Wayland or `xclip` for X11 if clipboard copy fails. Annoterm uses OSC 52 as a fallback. See [terminal support](docs/terminal-support.md) for details.

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

Press `Ctrl+F` to search the Markdown source in either mode. Search is literal and case-insensitive. In rendered mode, `/` also opens search.
Use `Enter` or `Down` for the next match. Use `Shift+Enter` or `Up` for the previous match.
Press `Esc` to close the prompt. Raw mode selects the exact match. Rendered mode selects its block.

To comment on an exact source range:

1. Press `Ctrl+R`.
2. Select text with `Shift` and the arrow keys.
3. Press `Ctrl+K`.
4. Write the comment and press `Enter`.

Press `Ctrl+S` to save raw changes. Annoterm saves the document atomically and stops if another process changed the file.

## Comment controls

In rendered mode, use `[` and `]` to select comments without leaving the document. `Up` and `Down` continue to select Markdown blocks.

Press `Ctrl+W` to focus the Comments panel. While it is focused, either `Up` and `Down` or `[` and `]` select comments. Raw mode requires this focus before you can select or change a comment.

Annoterm shows shortcuts for the active pane in one bar at the bottom of the terminal. Press `?` in rendered mode or the Comments panel to show all shortcuts. Press `F1` in raw mode or while you write a comment so that you can insert `?` as text.

| Key | Action |
| --- | --- |
| `[`, `]` | Select a comment in rendered mode or the focused Comments panel |
| `Up`, `Down` | Select a comment only while the Comments panel is focused |
| Mouse wheel or trackpad | Move through comments when the panel is focused |
| `j` | Jump to the selected comment |
| `e` | Edit the selected comment |
| `x` | Resolve or reopen the selected comment |
| `d` | Delete the selected comment |
| `o` | Repair an outdated or detached comment at the current selection |
| `Ctrl+W` | Switch focus between the document and Comments panel |
| `Esc` | Return focus to the document |
| `?` | Show all shortcuts |

The Comments panel shows the active focus and keeps the selected comment visible.
Open comments appear in generated feedback. Resolved comments stay in the sidecar but do not appear in feedback.

## Document controls

| Key | Action |
| --- | --- |
| `Ctrl+F`, `/` in rendered mode | Search the Markdown source |
| `Ctrl+R` | Toggle rendered and raw modes |
| `Ctrl+S` | Save in raw mode |
| Arrow keys | Move the raw cursor |
| `Shift` and arrow keys | Select source text in raw mode |
| `Ctrl+Z`, `Ctrl+Y` | Undo or redo a raw edit |
| `F1` | Show all shortcuts in raw mode |
| `Down`, `Tab` | Select the next rendered block |
| `Up`, `Shift+Tab` | Select the previous rendered block |
| `Left`, `Right` | Scroll the selected rendered table horizontally |
| Left click | Select a rendered block without scrolling |
| `a` | Comment on the selected rendered block |
| `Ctrl+K` | Comment on the raw selection or rendered block |
| `q` | Quit from the rendered document |
| `Ctrl+Q` | Quit from rendered or raw mode |
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

Annoterm requires Rust 1.88 or newer. Install Rust with [rustup](https://rustup.rs/) if `cargo` is not available.

Clone and install from source:

```sh
git clone https://github.com/filipgutica/annoterm.git
cd annoterm
cargo install --locked --path .
annoterm --help
```

Cargo installs `annoterm` in its binary directory, usually `~/.cargo/bin`. Restart your shell if it cannot find the command.
The Homebrew formula also builds from source, using a tagged release.

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

## Contributing and releases

Submit changes through pull requests and squash merge them with a [Conventional Commit](https://www.conventionalcommits.org/en/v1.0.0/) title. Use `fix:` for a patch release, `feat:` for a minor release, and `!` after the type or scope for a major release, such as `feat!: change the command interface`. These rules also apply before version 1.0.0. A `BREAKING CHANGE:` footer in the squash commit body also triggers a major release.

Other accepted types are `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, and `revert`. Documentation and maintenance changes alone do not create a release; `perf:` changes create a patch release.

After a releasable change reaches `main`, Release Please opens or updates a release pull request with the version and changelog changes. Review that pull request and merge it after its required checks pass. The release workflow then creates the version tag and GitHub release. Update the formula in [the Homebrew tap](https://github.com/filipgutica/homebrew-tap) to use the new tag and source archive checksum.

Release Please uses the repository's `GITHUB_TOKEN`. For its pull requests, GitHub puts CI runs in an approval-required state. A maintainer with write access must select **Approve workflows to run** in the pull request merge box before the required checks can run. See [GitHub's workflow trigger documentation](https://docs.github.com/en/actions/how-tos/write-workflows/choose-when-workflows-run/trigger-a-workflow#triggering-a-workflow-from-a-workflow).

## Current limits

- Raw editing has bounded in-memory undo and redo. The keymap is not configurable.
- Rendered comments cover complete top-level blocks. Raw comments can cover arbitrary UTF-8 selections.
- External changes stop a save. Annoterm does not include an interactive merge view.
- Finder integration uses Terminal.app. The local app bundle is ad hoc signed and is not notarized.

## License

Annoterm is available under the [MIT License](LICENSE).
