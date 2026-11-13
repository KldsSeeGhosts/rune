# Downstream editor additions

This fork starts at `22fcaf167b5e8d74e2ab173a698b7cf2053c5889`.
It adds editor capabilities independently of any application's Markdown grammar or skin.
The existing exhaustive, `Copy` `Block` enum and literal-built `RichStyle` remain unchanged.

## Capability commits

Commit identifiers can be found with `git log --oneline --grep='R[1-4]'`.
Each entry below names the exact commit subject so this file does not need to contain
its own commit hash.
Every capability commit also updates this file.

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

### R2, logical blocks

`R2: preserve logical block identities and exact code source`

Adds `BlockId`, `LogicalBlock`, `BlockRangeError`, and `CodeAttributes`.
`logical_blocks()` enumerates content ranges without a separator from the next
block, including empty ranges. `set_logical_block` creates a fresh identity;
`update_logical_block` retains one. Setters require whole physical lines.
Multiline non-code logical blocks use the existing soft-break representation.

Code line splits retain their logical identity. Hard paragraph splits create
new identities. Joins retain the same left/right survivor as authored attributes.
Disjoint remainders after a structural replacement receive distinct ids.
Updating per-line source on an existing logical block updates all its lines.
Code source retains the exact opening fence, complete info string and optional
closing source. `None` and `Some("")` are different closing states.

Legacy documents keep the existing HTML output unless they use authored/logical
setters or contain separate adjacent code blocks. That legacy HTML allocates fresh
ids on import. Authored HTML retains ids, including on empty lines. Rich paste
always remaps ids so copying a fence does not alias the original block.
Separate legacy `<pre>` elements also import as separate logical code blocks.

Touched files:

- `crates/rune/src/edit.rs`
- `crates/rune/src/edit/{block_data,history,html,input,marks,replace,secure}.rs`
- `crates/rune/src/edit/html/{attributed,parser}.rs`
- `crates/rune/src/lib.rs`
- `crates/rune/src/edit/logical.rs`
- `crates/rune/src/edit/logical_tests.rs`
- `changelog/unreleased/2026-11-12-logical-blocks.md`

### R4, source transactions and selection notifications

`R4: add source transactions, app marks and selection notifications`

Adds `AppMark`, `EditMap`, `MapBias`, `TransactionOptions`, and `TransactionError`.
`set_app_mark`, `app_marks`, and `remove_app_mark` manage nonvisual ranges.
The existing non-exhaustive `Mark` enum gains `App`; its kind bit is always allowed,
as app-owned pills are. App marks survive code blocks and have independent
inclusive/exclusive start and end boundaries. HTML stores their line-relative
ranges in inert `data-rune-apps` attributes, including covered newline bytes.

`replace_range` edits a specified range without a preliminary selection change.
`transact(Recorded, ...)` records one step and returns the sequence's byte map.
`transact(Transient, ...)` preserves undo and redo and maps the incoming selection,
unless the closure explicitly sets it. Maps compose with `then` and reverse their
coordinates with `inverse`; bias resolves inserted/deleted boundary ambiguity.
Use model operations inside transactions, not direct public-field assignment.

Instead of rebasing old snapshots into a different Markdown representation,
transient mode retains their canonical document. Undo or redo restores that shape
before replay. A new recorded edit uses the canonical before-state and includes
the source rewrite in its after-state. This is the replay/rebase strategy required
by source reveal after undo. It preserves redo without clearing or rewriting the
stacks. Old snapshots remain in the shapes in which their edits were recorded.
Transactions reject active composition, secure fields, nested transactions and
open undo groups. A panic restores the document, history and detached baseline,
except that an explicit secure-mode transition retains its security change.

`on_input_selection_change` reports committed selection changes from pointer,
keyboard, accessibility, IME cursor, history and programmatic updates/polls.
It does not use `on_edit` calls as notifications. Callback-owned normalization
settles in that same notification; it does not recursively notify itself.
Secure fields do not report selection. A direct model mutation outside the
runtime reports on the next committed view, not synchronously in the setter.
Multiple pointer/programmatic changes before one commit coalesce. This is not a
per-mutation event stream. Exact same-frame caret leave/reentry normalization
therefore remains a gap. It needs model-level selection transaction events or
stable field bindings; retaining mutable accessors after arbitrary app updates
is unsafe because a field may have unmounted.

Closing this is not a small notification hook.
`crates/rune/src/edit.rs::TextEdit::set_selection` has only a mutable editor, while
`crates/rune/src/runtime/input/editor_selections.rs::Runtime::next_editor_selection`
reads the final committed snapshot and invokes a callback with app state. A revision
counter would detect leave/reentry but could not deliver its intermediate positions
or normalize source between the operations. The enabling change is a bounded
per-model selection journal with revisioned event payloads, stable runtime bindings,
and transaction boundaries where the runtime can safely deliver each event. Direct
writes to the still-public `anchor` and `focus` fields would also need a setter-only
migration to guarantee every mutation. The existing callback contract is unchanged.

Touched files:

- `crates/rune/src/edit.rs`
- `crates/rune/src/edit/{app_mark,display,edit_map,history,html,marks,model,replace,secure,transaction,transaction_tests}.rs`
- `crates/rune/src/edit/html/{attributed,parser}.rs`
- `crates/rune/src/el/input.rs`
- `crates/rune/src/lib.rs`
- `crates/rune/src/runtime/input/{editor_selections,editor_selection_tests,action}.rs`
- `crates/rune/src/runtime/frame/updates.rs`
- `crates/rune/src/runtime/{frame,commit}.rs`
- `crates/rune/src/runtime.rs`
- `changelog/unreleased/2026-11-12-editor-transactions.md`

Runtime update/request functions moved
unchanged into `frame/updates.rs` before adding the update notification hook.

### R3, atomic rules

`R3: add atomic rule editing and document block-view layout gap`

Adds `EditorSelection`, `TextEdit::selection`, and `select_block`. A rule is an
atomic selection distinct from a caret at the same byte. Arrow keys cross it,
Backspace/Delete remove one object, typing replaces it in its own paragraph,
and undo/redo restore its identity, source and selection. Rich copy emits a rule;
plain copy uses its authored source or the object replacement character.
Selection callbacks also report object-only changes at an unchanged byte offset.
The example also exposed a history-notification gap: mark-only undo/redo can keep
the same offsets while changing source context. A non-content history movement
tag makes those operations notify too, without reporting ordinary checkbox edits
as selection changes.
Lifecycle tests also cover unmounting an observed editor and removing it from its
own callback. Notifications read current committed `InputSpec` snapshots, never
retained mutable accessors. After each callback the view settles before another
binding is used. A callback-owned rewrite of the same field does not recursively
notify during that commit transaction.
Capture and detached rewriting use separate event/effect state machines.
Entering secure mode inside a transaction invalidates capture and discards old
history, even when the closure turns plain again or panics. Turning plain restores
model invariants once, including stable identities for the new plain paragraphs.
Default task checkbox clicks toggle checked state without moving the selection.
This is independent of custom block widgets.

Accessibility keeps upstream text-run ids and text-selection actions. Headings
have heading levels, tasks are checkboxes with checked state and a click action,
and rules are separators with selection state and a click-to-select action.
Semantic block ids cover the first 16,384 paragraphs of a field; subsequent text
still uses the ordinary text-run path.

Touched files:

- `crates/rune/src/edit.rs`
- `crates/rune/src/edit/{block_data,history,input,logical,marks,object_selection,object_selection_tests,replace,secure,transaction,transaction_tests}.rs`
- `crates/rune/src/edit/transaction/{capture,rewrite}.rs`
- `crates/rune/src/a11y/{actions,text}.rs`
- `crates/rune/src/el/input.rs`
- `crates/rune/src/lib.rs`
- `crates/rune/src/runtime.rs`
- `crates/rune/src/runtime/{commit,frame}.rs`
- `crates/rune/src/runtime/frame/updates.rs`
- `crates/rune/src/runtime/input/{action,editor_selection_tests,editor_selections,rule_selection_tests}.rs`
- `crates/rune/src/runtime/selection/{blocks,click}.rs`
- `crates/rune/src/runtime/paint/{content,helpers}.rs`
- `crates/rune/examples/markdown_composer.rs`
- `crates/rune/examples/markdown_composer/tests.rs`
- `changelog/unreleased/2026-11-12-atomic-rules.md`

### R3, composite text geometry

`R3: reserve block-widget slots in composite text layout`

The internal slot table carries `BlockId`, displayed paragraph, position and measured
dimensions. Parley's line breaker reserves header gaps and leading insets in one
document, without synthetic characters or selection boxes. The cache includes the
slot table and width. Parley sums line heights without reserved gaps; Rune's input
measurement adds those gaps explicitly. An empty table takes the original path,
including the same cached layout object, dimensions and line geometry.

Touched files:

- `crates/rune/src/text.rs`
- `crates/rune/src/text/{composite,composite_tests,line_break,rich_layout}.rs`
- `crates/rune/src/el/text_spec.rs`
- `crates/rune/src/runtime/commit/text_layout.rs`
- `changelog/unreleased/2026-11-12-composite-input-layout.md`

### R3, ordinary block views

`R3: attach ordinary block views with composite input layout`

Adds `InputBlockView`, `InputBlockPosition` and `El::input_block_view`. Each logical
block has at most one view; another attachment replaces it. Registration order
does not affect document order. `Header` reserves the measured element height
above the first line, across the full block width including its text inset. `Leading` reserves its
natural width beside the first line, and any height beyond that line. Leading
slots follow the paragraph direction and replace default list/task markers.
Missing identities, secure inputs and single-line inputs omit their views.
Secure transitions discard old widget visuals immediately rather than fading
potentially sensitive labels out.

Widget subtrees remain children in Rune's runtime, but are separate roots in the
layout solver. Both `LayoutStore` and `BaselineLayoutStore` already support
measured leaves and independent subtree roots, so neither solver needs a forked
container algorithm. `solve` first measures the text, measures widgets at their
block widths, then remeasures text with the reserved table. After shaping, one
placement pass moves each complete widget subtree into its slot. The ordinary
runtime parent chain supplies scrolling, clipping, transforms and accessibility.
No widget text enters the editor's bytes, clipboard, selection or history.
Slot layout lands with text reflow instead of springing independently. Ordinary
entrance/exit fades and hover colors remain; `animated_changes` is an explicit opt-in.

Views use ordinary handlers and accessibility builders. Pointer presses, including
blank slot space, do not focus a view or change the editor's caret/composition.
Tab visits the editor and then views in document order, even when native settings
normally skip buttons. A view's `focusable(false)` and ordinary Tab overrides still
apply. With views attached, Tab traverses instead of performing default code/list
indentation; an app can handle an editing chord through `on_input_key`.
Escape returns keyboard focus to the editor. Removing a focused view also returns
focus to its surviving editor. Unmounted widgets follow Rune's ordinary exit
presentation, but stop accepting events immediately.

Block anchors map through an IME preedit splice without changing model identity.
A composition that hides a block boundary temporarily omits its widget. Canceling
the composition restores that attachment. The input snapshot carries these derived
anchors, avoiding document clones during measurement and placement.

Tests compare no-view layout, measurement and paint bytes; cover header and leading
dimensions, wrapping, resize, scrolling, transforms, selection, copy, pointer/key
events, Tab/Escape, focus opt-out, accessibility actions, IME continuity, empty code,
identity through history, block removal, and secure/single-line transitions.
Selection and block-view tests share one fixture state to avoid duplicating the
generic runtime in the native test executable.

Touched files:

- `crates/rune/src/el.rs`
- `crates/rune/src/el/{input,text}.rs`
- `crates/rune/src/el/input/block_view.rs`
- `crates/rune/src/edit/logical.rs`
- `crates/rune/src/lib.rs`
- `crates/rune/src/runtime.rs`
- `crates/rune/src/runtime/commit/{apply,block_views,block_views_tests,flatten,relayout}.rs`
- `crates/rune/src/runtime/input/{editor_selection_tests,focus,keys,pointer}.rs`
- `crates/rune/src/runtime/selection/{blocks,click}.rs`
- `crates/rune/src/runtime/paint/helpers.rs`
- `crates/rune/src/text/{composite,composite_tests,line_break}.rs`
- `changelog/unreleased/2026-11-12-input-block-views.md`

### R3, composer block controls

`R3: demonstrate code headers and leading task controls`

The `markdown_composer` example attaches one pressable language header to every
code block and an ordinary checkbox element to every task's leading slot.
Language presses cycle Rust, TypeScript and Python through `CodeAttributes`,
preserving the info-string suffix, opening fence and optional exact close.
Checkbox presses update the same logical task. Both operations record history
without moving the caret or canceling a code block's composition.

The existing source-reveal example still uses transient transactions and selection
notifications. Example tests cover language cycling, exact source retention,
checkbox undo/redo, source reveal with language history and stable attachments,
and a language press during simulated composition. This is a small API example,
not the reference application's searchable language picker or Markdown grammar.

Touched files:

- `crates/rune/examples/markdown_composer.rs`
- `crates/rune/examples/markdown_composer/{blocks,block_views_tests}.rs`
- `changelog/unreleased/2026-11-13-composer-block-controls.md`

### R3, text-field widget presses

`R3: preserve editor composition when pressing field widgets`

Text fields inside a block view follow the same pointer-focus rule as other
widgets. Their ordinary press handlers run, but a pointer press does not enter
their text-selection path or cancel the containing editor's composition.
Keyboard focus remains available through ordinary Tab traversal.

Touched files:

- `crates/rune/src/runtime/input/pointer.rs`
- `crates/rune/src/runtime/input/editor_selection_tests.rs`
- `crates/rune/src/runtime/commit/block_views_tests.rs`
- `changelog/unreleased/2026-11-13-block-view-field-focus.md`

### R1/R3, optional block styling

`R1/R3: add optional heading, checkbox and rule styling`

`BlockStyle` adds six optional controls. `checkbox_checked_fill` colors the checked
box's interior. `checkbox_mark_color` draws a two-stroke tick instead of the legacy
inner square. Setting only a fill retains that square. The tick scales with the
checkbox size and uses its stroke width.

`heading_line_height` contains one optional unitless multiplier per level, relative
to that heading's font size. `heading_color` contains one optional text color per
level. Unset heights retain the field's existing ratio; unset colors inherit the
text color. Non-positive or non-finite height multipliers inherit too.

`rule_margin_top` and `rule_margin_bottom` reserve points above and below a rule.
Unset, negative and non-finite margins mean zero. These gaps use the composite text
layout, including intrinsic height, caret geometry and pointer hit testing.
They add to block-view slots without inserting document bytes or changing history.
All six controls default to unset. `BlockStyle::DEFAULT` is the const equivalent
of `Default::default()`, shared by the text's empty cold record.

The shaping methods moved unchanged from `text.rs` into `text/shaping.rs` before
adding the internal line-height hook. The existing `Span` and `TextStyle` remain
unchanged. Tests cover default layout-cache identity and legacy checkbox geometry,
independent checkbox colors and tick segments, all heading levels, empty headings,
rule margins on empty/final rules, widget-slot composition and margin hit targets.

Touched files:

- `crates/rune/src/edit/block_style.rs`
- `crates/rune/src/el/text_spec.rs`
- `crates/rune/src/text.rs`
- `crates/rune/src/text/{block_layout,block_style_tests,composite,line_break,rich_layout,shaping}.rs`
- `crates/rune/src/runtime/input/rule_selection_tests.rs`
- `crates/rune/src/runtime/commit/flatten.rs`
- `crates/rune/src/runtime/paint/helpers.rs`
- `crates/rune/src/runtime/selection/{blocks,click}.rs`
- `changelog/unreleased/2026-11-13-block-style-options.md`

## Rebase notes

For optional block styling, retain the per-range heading-height hook in
`text/shaping.rs::layout_keyed_heights` when merging upstream shaping changes.
The no-override wrapper keeps its previous signature and cache key. Heading metrics
belong in the shape key; rule gaps belong in both the break key and the leaf's
measurement key in `runtime/commit/flatten.rs`. Keep margin accounting
in `TextSpec::measured_height` and the rule hit-target expansion together with
`line_break::break_lines_styled`. Do not reserve margins with painted padding alone.
Keep heading separator font sizes with the line-height overrides, or a newline's
different span box can enlarge the line. An invisible zero-width strut box gives
the final empty paragraph its own metrics instead of the preceding newline's.
It adds no bytes or caret stops and is not a pill or a widget slot.

For block views, keep three hooks together:

- Flatten widget wrappers into runtime children, but do not push those roots into
  the input's solver child list. Retain their solver slots just like other nodes;
  memo reuse and virtual remount already visit every runtime node.
- Run `block_views::measure` after root measurement, including after anchored-fit
  changes. Run `block_views::place` after every node's text is shaped. Slot dimensions
  and direction belong in the break-cache key, not the shaping key. Keep the
  original empty-slot path unchanged.
- Parley does not include explicit line-y gaps in `Layout::height`. Input intrinsic
  height, text extent and negative-x pointer clamping must use the composite height.
  Caret, selection, IME and text-run accessibility otherwise read the same Parley
  line geometry. Do not add synthetic characters to reserve widget space.

`InputSpec::block_views` owns attachment metadata. Clear it and its wrapper children
when secure or single-line builders run. Derive display anchors from the snapshot's
composition, not the secure-mask-only `display_index` helper. Preserve pointer focus
for widget descendants; keep Tab traversal, Escape and removed-view focus fallback
separate from text editing. Default marker paint and task hit targets must not
duplicate a leading view. Ordinary widget accessibility needs no separate tree.
The block-view pointer guard must precede `textbox_input` dispatch as well as the
ordinary control-focus path, since a widget can itself contain text fields.
Keep the apply hook that unmounts old widget descendants when an input becomes
secure, including widgets already in an exit presentation.
The example's attachment builder and handlers live in `markdown_composer/blocks.rs`;
they need no private layout or runtime API. Preserve the code info suffix and close
when adapting the language control.

The replacement engine and HTML parser were extracted without changing their
algorithms before adding hooks, to keep the owning files below the size limit.
When upstream changes these functions, transplant its implementation into
`edit/replace.rs` or `edit/html/parser.rs`, then reapply the narrow hooks below.

- `TextEdit` owns `block_data`. Include it in construction, reset, hashing,
  secure-mode clearing and history snapshots. Snapshot byte accounting includes
  owned source strings.
- R2 also owns parallel `block_ids`. Map them before the replacement engine mutates
  text. A code/soft split shares its id; a hard paragraph split allocates one.
  Snapshots include ids and their byte cost. Structural setters repair disjoint ids.
  HTML's line record carries the persisted id, but the live identity table owns it.
  Rich paste remaps ids with one map per pasted fragment, not one map per line.
- R4's replacement hook captures the actual pill-expanded range in `EditMap`.
  App marks map their own boundaries rather than inheriting a typing style.
  Keep them out of the code-block mark stripping and intrinsic style generation.
- Snapshot recording consumes a detached canonical baseline only for a real
  recorded edit. Undo/redo restore it after composition cancellation and before
  checking the snapshot's expected text. `clear_history`, secure mode and full
  replacement discard that detached baseline.
- Keep transaction capture invalidation in `set_secure(true)`. A transient
  transaction must not reattach its saved history after that event, including
  when it unwinds. The capture/rewrite machines return effects; the editor applies
  them. Plain mode must synchronize the document/identity tables before returning.
- Selection observers belong to `editor_selections`, not to `on_edit`. Compare
  committed input snapshots after applying nodes. Deliver one callback, rebuild,
  then resolve another binding against that current tree. Never call an old
  accessor after an arbitrary update or callback: an editor may have unmounted.
  The existing pre-key commit boundary preserves notifications for consecutive
  queued keys. Callback-owned changes settle within the same commit transaction.
  Fields without observers incur no per-action tree scan.
- R3's object selection is a separate enum mode. Include it in snapshots and
  reset/secure paths, but not in document equality when deciding to record history.
  Hash and selection observers use the public `EditorSelection` so selecting an
  empty rule still redraws/notifies.
- Keep the atomic replacement hook ahead of the ordinary empty-text no-op.
  Deleting a last rule after an empty paragraph must retain that paragraph's
  identity/source instead of applying the generic right-survivor rule to the rule.
- Pointer block hit testing runs before ordinary caret placement and only on a
  non-Shift press. Default checkbox hit geometry must match block painting.
  Atomic rule highlighting suppresses the ordinary collapsed caret.
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
screen readers, Windows or Linux. Exact per-mutation selection notifications remain
the R4 gap described above. The example's source grammar handles one balanced bold
region, not general Markdown parsing.

The fork passes the required three-package suite, 3,162 tests passed
and 76 ignored, including doctests, and `cargo check --workspace --all-targets`.
All 24 block-view runtime tests and 11 focused block-style/rule tests also pass
with `baseline-taffy`. The styling change adds nine regression tests. The example has
six passing tests with each layout engine. File-size and folder-shape checks have
no errors. Repository-wide `cargo fmt --check` reports existing formatting drift;
unrelated files have not been reformatted.

On macOS, the large library-test executable can hit Apple's compact-unwind linker
assertion or fail to spawn near 2 GB. Editor tests share their fixture state to limit
runtime monomorphizations. These invocation-only settings reduce debug information
and bound test concurrency without changing the unoptimized debug test profile:

```sh
CARGO_PROFILE_TEST_DEBUG=0 RUST_TEST_THREADS=1 cargo test \
  -p rune -p rune-mermaid -p rune-kit \
  --features rune-kit/markdown,rune-kit/mermaid
```

This changes no repository or machine configuration. Debug assertions and overflow
checks remain enabled. Run without a concurrent build if the existing AppKit
clipboard conversion tests time out.
Do not disable compact unwind generation: that made Rust panic unwinding fail in
the existing transaction rollback test.
