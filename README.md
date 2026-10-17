# mikocok

**A minimal Microsoft Word and Excel for macOS** — two tiny native apps, **mikocokWord** and **mikocokExcel**, for opening and editing `.docx` and `.xlsx` files. About 5 MB each, no Office, no runtime, no account.

*Bahasa Indonesia: versi minimalis MS Word dan MS Excel untuk Mac. Buka dan edit `.docx` / `.xlsx` dengan aplikasi native yang ringan (±5 MB).*

Written in Rust with [egui](https://github.com/emilk/egui). Both apps are the same binary; the launcher decides whether it runs as the Word or the Excel flavor.

## mikocokWord

Shows a `.docx` as pages, the way Word does, and lets you type straight into it.

- **Real pages** — paper with the document's own size and margins, automatic page breaks between pages, explicit page breaks honored, page counter in the status bar, zoom.
- **Formatting** — fonts, sizes, bold, italic, underline, strikethrough, colors, highlight, super/subscript; heading and paragraph styles; alignment (including justify), spacing, indents.
- **Lists, tables, images** — bullets and numbering, tables with borders and cell shading, PNG/JPEG images.
- **Editing** — click and type. `Enter` splits a paragraph (new one inherits the formatting), `Backspace` at the start joins it to the previous one, arrow keys move between paragraphs. Edits keep the formatting of the text they touch.

## mikocokExcel

A spreadsheet grid for `.xlsx` with sheet tabs, a formula bar, and in-place cell editing. Typing `=…` stores a formula; numbers are stored as numbers, everything else as text.

## Your files stay intact

mikocok edits the XML inside the file and rewrites only what you changed. Styles, images, charts, headers/footers, comments and everything else it doesn't understand are copied through untouched — it never re-exports the whole document. Saving writes to a temporary file first, so a failed save can't truncate your original.

## Install

Requires macOS and a [Rust toolchain](https://rustup.rs).

```sh
git clone https://github.com/luthfirahmn09/mikocok.git
cd mikocok
./build_app.sh            # builds and installs to ~/Applications
./build_app.sh --default  # ...and makes them the default apps for .docx / .xlsx
```

Then right-click a file → **Open With → mikocokWord / mikocokExcel**, double-click it (if set as default), or drag a file onto the window. The apps are ad-hoc signed; if macOS blocks the first launch, right-click → Open once.

Shortcuts: `⌘O` open, `⌘S` save, `⇧⌘S` save as.

## Limitations

This is a *minimal* editor, not a Word replacement.

- Word: no bold/italic/style toolbar yet (existing formatting is preserved and displayed, but you can't apply new formatting); no headers/footers, columns, text boxes, floating-image wrapping, or track changes. `Shift+Enter` line breaks aren't supported. List numbers after a newly created item refresh on reopen.
- Page breaks are computed by mikocok's own layout and can differ from Word's — paragraphs aren't split across pages, and fonts missing on your Mac (e.g. Calibri without Office) are substituted with Arial.
- Excel: no charts, number/date formatting, merged-cell display, or formula evaluation — formula cells show their last saved value, and Excel recalculates when you next open the file.
- Only PNG and JPEG images are displayed.

## Development

```sh
python3 tools/make_samples.py   # writes sample .docx/.xlsx fixtures to $TMPDIR
cargo test
cargo run --release -- path/to/file.docx
```

| File | What it does |
|---|---|
| `src/docx.rs` | Word model: styles, numbering, tables, parsing, and the XML rewriter (edit / split / join paragraphs) |
| `src/docview.rs` | Page rendering, pagination, in-place editing with egui |
| `src/xlsx.rs` | Excel reader and cell-level XML rewriter |
| `src/ooxml.rs` | Zip package and XML helpers |
| `src/fonts.rs` | Loads system fonts so documents use the fonts Word would |
| `build_app.sh` | Builds the two `.app` bundles with "Open With" support |

Tested on macOS with unit tests for parsing, edit round-trips, and headless page layout. Visual fidelity against real Word documents is best-effort — issues and sample files are welcome.
