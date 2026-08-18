# macOS Finder installation

Annoterm has two macOS entry points:

```sh
annoterm path/to/document.md
```

and `Annoterm.app`, which lets Finder open Markdown documents in Terminal.app.

## Build and install

Build the local app bundle from the repository root:

```sh
./scripts/build-macos-app.sh
```

This produces a universal Apple Silicon and Intel binary. It requires both Rust targets:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

For a local build that only needs the current Mac architecture, opt in explicitly:

```sh
./scripts/build-macos-app.sh --host-only
```

The result is `dist/Annoterm.app`. Install it into `/Applications`:

```sh
./scripts/install-macos-app.sh
```

The installer refuses to replace an existing application unless you explicitly pass `--replace`:

```sh
./scripts/install-macos-app.sh --replace
```

The bundle is ad-hoc signed for local use. It is not Developer ID signed or notarized.

## Make Annoterm the default Markdown application

1. In Finder, select a `.md` or `.markdown` document and choose **File > Get Info**.
2. Under **Open with**, select **Annoterm**.
3. Select **Change All** and confirm the dialog.

macOS registers Annoterm as an editor for `net.daringfireball.markdown`. The bundle declares `.md`, `.markdown`, and `text/markdown` as imported type tags.

## Terminal access

When Finder opens a Markdown document, the launcher asks Terminal.app to run the bundled Annoterm binary with the selected path. If macOS blocks this action, allow Annoterm to control Terminal in **System Settings > Privacy & Security > Automation**, then open the document again.

The launcher quotes both the binary and document paths before sending the command to Terminal. File names containing spaces, quotes, Unicode, or shell metacharacters are passed as file paths rather than shell code.

This first release opens files in Terminal.app only. It does not configure Finder automatically, change the default application automatically, or support iTerm2 and WezTerm launchers.
