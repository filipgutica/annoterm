# Terminal support

Annoterm targets current macOS and Linux terminals. Crossterm also provides the Windows event and terminal layer, and the clipboard code supports `clip.exe`, but the first release does not publish or test a Windows package.

## Rendered Markdown

| Input | Terminal result |
| --- | --- |
| Headings, paragraphs, emphasis, and strong text | Styled or visibly marked text |
| Ordered and unordered lists | Text markers and indentation |
| Task lists | `[x]` and `[ ]` markers |
| Tables | Aligned text columns with a bold header row |
| Links | Link label followed by the URL |
| Blockquotes | Lines prefixed with `│` |
| Fenced code | A labeled block with Syntect highlighting for known languages |
| Footnotes | Footnote labels and definitions as text |
| Raw HTML | Escaped, literal HTML text. Annoterm never executes it. |
| Images | `[image: alt text — URL]` |
| Inline and display math | `[math]` followed by the source expression |
| Mermaid | A visible notice followed by the fenced source |
| Unknown constructs | Terminal-safe literal text when available |

Set `NO_COLOR` to disable syntax colors. Unknown code-fence languages use plain text. Core meaning never depends on color, Unicode box drawing, image protocols, hyperlinks, or mouse support. Mouse wheels and trackpads move the same rendered block selection or raw cursor as the keyboard when the terminal reports scroll events.

Annoterm removes terminal escape bytes from rendered Markdown and drops unsafe control characters. Raw mode displays control bytes as visible safe symbols while preserving the original bytes for editing and saving.

## Clipboard order

- macOS: `pbcopy`
- Wayland: `wl-copy`
- X11: `xclip -selection clipboard`
- Windows builds: `clip.exe`
- Final fallback: OSC 52

Annoterm writes clipboard commands through standard input and checks their exit status. It reports failures in the TUI or command output. OSC 52 has a 100,000-byte limit and no reliable acknowledgement. Terminal multiplexers, SSH sessions, and terminal policy may block or truncate it.

The annotation sidecar is durable storage. The clipboard is not.

## Layout and resizing

Wide terminals show the document and comments side by side. Narrow terminals keep the document panel and hide the comment panel. Terminals smaller than 30 columns by 6 rows show a resize notice. Ratatui redraws after terminal resize events.

Grapheme movement prevents the cursor from splitting UTF-8 text. Display placement uses terminal-cell width, including common wide CJK and emoji characters. Exact appearance can still differ across terminal emulators for zero-width joiners and newly assigned Unicode code points.
