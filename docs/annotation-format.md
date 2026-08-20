# Annotation sidecar format

Annoterm stores review data outside the Markdown source. For `guide.md`, the default file is `~/.annoterm/<sha256-of-canonical-document-path>.annotations.json`. The filename is a SHA-256 hash of the canonical document path.

On interactive open, Annoterm checks for a legacy `.annoterm/guide.md.annotations.json` file. If no user-local sidecar exists, Annoterm copies that file. It leaves the legacy file unchanged. Remove the old `.annoterm` directory after you verify the copy.

## Schema version 1

```json
{
  "schema_version": 1,
  "document": {
    "id": "019c...",
    "path": "../guide.md",
    "fingerprint": "sha256:..."
  },
  "annotations": [
    {
      "id": "019c...",
      "status": "open",
      "anchor_state": "anchored",
      "anchor": {
        "source_range": {
          "start": { "byte": 12, "line": 3, "column": 1 },
          "end": { "byte": 31, "line": 3, "column": 20 }
        },
        "quote": "Selected source text",
        "context_before": "Nearby text before the selection",
        "context_after": "Nearby text after the selection",
        "document_fingerprint": "sha256:...",
        "block_kind": "paragraph"
      },
      "comment": "Explain this term.",
      "created_at": "2026-08-17T12:00:00Z",
      "updated_at": "2026-08-17T12:00:00Z"
    }
  ]
}
```

UUID version 7 values identify documents and annotations. Status is `open` or `resolved`. Anchor state is `anchored` or `orphaned`. These fields are separate so a resolved comment can still become orphaned after an edit.

Positions use zero-based UTF-8 byte offsets and one-based line and Unicode-scalar columns. Ranges are half-open. Context stores at most two surrounding lines and 512 UTF-8 bytes on each side.

## Versioning

Readers accept schema version 1 only. They reject both older and newer versions instead of guessing at a migration. A future release must add an explicit migration that preserves unknown input until conversion succeeds.

## Moves, renames, and edits

The document path is relative to the sidecar. Moving or renaming a document changes its default annotation path. The original annotations remain in `~/.annoterm`. To continue using them, open the existing file explicitly:

```sh
annoterm renamed.md --annotations ~/.annoterm/<existing-sidecar>.annotations.json
```

Annoterm keeps the existing document and annotation identifiers. It writes the new relative document path when it opens the sidecar.

Source edits change the SHA-256 fingerprint. Annoterm then re-anchors comments from their quote and nearby context. A unique match updates the range, context, and anchor fingerprint. A missing or ambiguous match becomes orphaned and remains in the file until a user repairs or deletes it.

Annoterm fingerprints the sidecar bytes when it loads them. Every later comment write takes an advisory lock and compares that revision inside the lock. A mismatch stops the write instead of replacing external changes. Other Annoterm processes use the same adjacent `.lock` file.

## Git safety

The format contains source quotes and review comments. It does not intentionally store account details or absolute host paths. It is suitable for Git when the quoted source and comments are safe for that repository. Teams should treat sidecars as review content and apply the same secret-scanning rules used for Markdown files.

Sidecar comments become instructions in the generated coding-agent prompt. Review sidecars from the same trust boundary as source code before copying or exporting their feedback.
