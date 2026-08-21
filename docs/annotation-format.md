# Annotation sidecar format

Annoterm stores review data outside the Markdown source. For `guide.md`, the default file is `~/.annoterm/<sha256-of-canonical-document-path>.annotations.json`. The filename is a SHA-256 hash of the canonical document path.

On interactive open, Annoterm checks for a legacy `.annoterm/guide.md.annotations.json` file. If no user-local sidecar exists, Annoterm copies that file. It leaves the legacy file unchanged. Remove the old `.annoterm` directory after you verify the copy.

## Schema version 2

```json
{
  "schema_version": 2,
  "document": {
    "id": "019c...",
    "path": "../guide.md",
    "fingerprint": "sha256:..."
  },
  "snapshots": {
    "sha256:...": "# Guide\n\nSelected source text\n"
  },
  "annotations": [
    {
      "id": "019c...",
      "status": "open",
      "anchor_state": "outdated",
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
      "navigation_hint": {
        "source_range": {
          "start": { "byte": 18, "line": 3, "column": 1 },
          "end": { "byte": 42, "line": 3, "column": 25 }
        },
        "document_fingerprint": "sha256:current"
      },
      "comment": "Explain this term.",
      "created_at": "2026-08-17T12:00:00Z",
      "updated_at": "2026-08-17T12:00:00Z"
    }
  ]
}
```

UUID version 7 values identify documents and annotations. Status is `open` or `resolved`.

Anchor state is `anchored`, `outdated`, or `orphaned`. An outdated annotation has an approximate current `navigation_hint`. An orphaned annotation has no usable current location.

Positions use zero-based UTF-8 byte offsets and one-based line and Unicode-scalar columns. Ranges are half-open. Context stores at most two surrounding lines and 512 UTF-8 bytes on each side.

The `snapshots` object stores source versions by document fingerprint. Annoterm retains versions referenced by an anchor or navigation hint.

## Versioning

Readers accept schema versions 1 and 2. Annoterm loads version 1 with no snapshots. An interactive open or annotation write can save version 2.

Readers reject unsupported versions instead of guessing at a migration.

## Moves, renames, and edits

The document path is relative to the sidecar. Moving or renaming a document changes its default annotation path. The original annotations remain in `~/.annoterm`. To continue using them, open the existing file explicitly:

```sh
annoterm renamed.md --annotations ~/.annoterm/<existing-sidecar>.annotations.json
```

Annoterm keeps the existing document and annotation identifiers. It writes the new relative document path when it opens the sidecar.

Source edits change the SHA-256 fingerprint. Annoterm first uses the quote and nearby context to find a unique current match.

If text matching fails, Annoterm maps the old range through its snapshot when the diff retains an unchanged boundary. It stores the result as an outdated navigation hint. A total replacement with no unchanged boundary remains orphaned instead of pointing at unrelated text.

Version 1 annotations have no snapshot. Annoterm uses their last known line as the first approximate location. It records the current snapshot for later edits.

An annotation becomes orphaned only when Annoterm cannot produce a current location. A repair replaces the old anchor with the current selection.

Annoterm fingerprints the sidecar bytes when it loads them. Every later comment write takes an advisory lock and compares that revision inside the lock. A mismatch stops the write instead of replacing external changes. Other Annoterm processes use the same adjacent `.lock` file.

## Git safety

The format contains source snapshots, quotes, and review comments. It does not intentionally store account details or absolute host paths.

Treat sidecars as copies of the reviewed document. On Unix, Annoterm creates its default user-local directory as mode `0700` and its sidecar and lock files as mode `0600`. Explicit `--annotations` paths retain caller-managed permissions. Apply the same secret scanning rules used for the Markdown source.

Sidecar comments become instructions in the generated coding-agent prompt. Review sidecars from the same trust boundary as source code before copying or exporting their feedback.
