# Architecture

Annoterm keeps parsing, storage, terminal effects, and application state separate. The Markdown file and annotation sidecar are the only durable inputs.

## Module boundaries

| Boundary | Files | Responsibility |
| --- | --- | --- |
| CLI | `src/cli.rs` | Parse open, copy, and export commands. Resolve the sidecar path. Connect durable effects to the TUI. |
| Document | `src/document/mod.rs` | Load UTF-8 Markdown, record BOM and line-ending details, detect external changes, and replace the file atomically. |
| Markdown | `src/markdown/` | Parse CommonMark and GFM into source-positioned rendered blocks. Define terminal-safe fallbacks. |
| Application | `src/app/` | Own mode, cursor, selection, undo history, selected block, comments, and user commands. It does not write files or access the clipboard. |
| Terminal UI | `src/ui/` | Map keys to application commands. Draw responsive document and comment panels. Apply syntax styles with Syntect. |
| Annotations | `src/annotations/` | Define schema version 2, save sidecars atomically, capture anchors, re-anchor comments, and preserve orphans. |
| Feedback | `src/feedback/` | Generate deterministic Markdown prompts, copy through platform backends, and export atomically. |
| macOS launcher | `macos/`, `scripts/` | Build a Finder document application that launches the bundled binary in Terminal.app. |

## Data flow

Opening a file follows this sequence:

1. `Document` loads the exact bytes and records the file fingerprint and format.
2. The Markdown parser creates top-level `RenderBlock` values with byte and line ranges.
3. The CLI loads the selected sidecar and re-anchors each annotation against the current source.
4. `App` owns the source, rendered blocks, selections, and durable annotation values during the session.
5. The terminal UI sends commands to `App`. It invokes callbacks only for document, sidecar, clipboard, and export effects.

Adding or changing a comment saves the full sidecar first. The save acquires an advisory lock, checks the loaded sidecar revision, and then replaces the file atomically. Annoterm then generates the prompt and tries to copy it. A clipboard error does not roll back the saved comment.

Saving source text compares the current disk fingerprint with the load fingerprint. A mismatch stops the save. A successful save uses a temporary file in the destination directory and an atomic replace. It then refreshes the document fingerprint and re-anchors comments.

## Source mapping

The Markdown parser provides one-based line and column positions plus UTF-8 byte offsets. Annoterm keeps byte ranges as the canonical source coordinates. Rendered-mode comments attach to a complete top-level block. Raw-mode comments attach to an arbitrary selection whose endpoints are valid UTF-8 boundaries.

Terminal cell positions are derived only for display. They never replace source offsets. This avoids mixing byte offsets, Unicode scalar positions, grapheme movement, and wide terminal cells.

## Re-anchoring policy

Annoterm uses conservative stages:

1. Keep the old range when it still contains the exact quote.
2. Find a unique exact quote and use nearby context to resolve candidates.
3. Accept a bounded changed quote when its similarity passes the fixed threshold.
4. Map the old range through its saved source snapshot when text matching fails and the diff retains an unchanged boundary.
5. Use the last known line when a version 1 sidecar has no snapshot.
6. Mark the annotation outdated and store its approximate current range.
7. Mark the annotation detached when the current document has no usable range, including a total replacement with no unchanged boundary.

An outdated annotation keeps its original anchor and a current navigation hint. Repair captures the current selection as a new exact anchor.

## Dependency choices

- Ratatui and Crossterm provide rendering, events, resizing, and test buffers.
- `markdown-rs` provides a CommonMark and GFM AST with byte, line, and column positions.
- Syntect highlights recognized fenced-code languages. Unknown languages remain readable plain text.
- `unicode-segmentation` and `unicode-width` keep cursor movement and terminal placement safe for graphemes and wide characters.
- `similar` maps annotation lines between saved source versions.
- `atomic-write-file` performs same-directory atomic replacement.

The application owns the editor state instead of exposing a third-party editor API. This keeps the first-release keymap and save format small and testable.
