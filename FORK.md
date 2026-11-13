# Downstream editor additions

This fork starts at `22fcaf167b5e8d74e2ab173a698b7cf2053c5889`.
It adds editor capabilities independently of any application's Markdown grammar or skin.
The existing exhaustive, `Copy` `Block` enum and literal-built `RichStyle` remain unchanged.

## Capability commits

Commit identifiers can be found with `git log --oneline --grep='R[1-4]'`.
Each entry below names the exact commit subject so this file does not need to contain
its own commit hash.

### R1, authored blocks

`R1: retain authored blocks through edits, history and HTML`

Adds `BlockKind`, `BlockAttributes`, and `BlockStyle`. Legacy `blocks()` projects
headings and rules to paragraphs, and tasks to bullets. `block_kind()` returns the
full kind. `set_block_kind`, `set_block_attributes`, and `set_task_checked` record
undo steps. Existing non-recording setters keep their old contracts.

The left paragraph survives a join, except when removing an empty prefix through
the next paragraph's start, in which case the right paragraph survives. Splits copy
authored source. A task continued with Return starts unchecked. Indent and outdent
change the task level without discarding authored source. Rune does not renumber
authored Markdown markers or change authored indent strings. Those grammar-specific
transformations belong to the app and can be grouped with the model edit.

List nesting uses all 256 representable `u8` levels and saturates at 255, rather than
overflowing or stopping at three. This is not an unbounded tree schema.

Attributed HTML uses inert `data-rune-line` JSON on each physical paragraph and
`data-rune-soft` for soft breaks. Import clamps heading levels and validates enum
shapes. Ordinary HTML still uses the existing parser. Authored exports preserve
whitespace, including empty lines. Untrusted metadata is data, never executable HTML.

Touched files:

- `crates/rune/Cargo.toml`
- `crates/rune/src/edit.rs`
- `crates/rune/src/edit/{block_data,block_style,block_tests,replace,history,html,input,marks,model,rich_tests,secure}.rs`
- `crates/rune/src/edit/html/{parser,attributed}.rs`
- `crates/rune/src/el/{input,text,text_spec}.rs`
- `crates/rune/src/runtime/paint/helpers.rs`
- `crates/rune/src/a11y/{actions,text,tree}.rs`
- `crates/rune/src/lib.rs`
- `changelog/unreleased/2026-11-12-authored-blocks.md`

## Rebase notes

The replacement engine and HTML parser were extracted without changing their
algorithms before adding hooks, to keep the owning files below the size limit.
When upstream changes these functions, transplant its implementation into
`edit/replace.rs` or `edit/html/parser.rs`, then reapply the narrow hooks below.

- `TextEdit` owns `block_data`. Include it in construction, reset, hashing,
  secure-mode clearing and history snapshots. Snapshot byte accounting includes
  owned source strings.
- `apply_text_and_attr_replace` skips the plain fast path when authored data exists
  and calls `map_block_data` before changing the legacy blocks.
- `sync_invariants` reconciles the authored data after legacy blocks. Legacy setters
  clear extended kinds deliberately, while retaining source attributes.
- Rich paste copies authored data under the same first-line merge rule as blocks.
- HTML import records line data after a block tag claims its paragraph. Authored
  text bypasses browser whitespace collapsing. HTML export takes a separate path
  only when authored data exists.
- `sync_input_rich` adds heading spans and copies full kinds to `TextCold`. Bidi
  shaping and IME display-index conversion continue to use upstream's functions.
- Block paint adds checkbox and rule decoration before legacy list markers.
  `BlockStyle` controls the new visual parameters without extending `RichStyle`.
- Accessibility wraps the existing text runs in heading, checkbox or separator
  nodes. Text-run identifiers and selection actions remain the existing ones.
  Checkbox actions edit the same model and history as keyboard editing.
  The per-element accessibility sub-id range `0xc000..=0xffff` belongs to blocks;
  links retain `0x8000..0xbfff`.

## Validation limits

Tests exercise the model and headless runtime. They do not verify native IME,
screen readers, Windows or Linux. A stored rule kind is not yet an atomic object
selection. Block widgets and transient source/history rewriting require their
own capabilities; this first commit does not claim to provide them.
