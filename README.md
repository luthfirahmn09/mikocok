# mikocok 🍜

**A replacement for Numbers & Pages, for people who just want to edit an office file.**

You know the moment. You're in a hurry. You double-click `report.xlsx` and what opens is... **Numbers**. Or `contract.docx` lands in **Pages**, where the layout slides a thousand pixels to the left and your table relocates to another dimension.

That, friends, is why *mikocok* exists.

Two tiny Mac apps, about 5 MB each. (That's megabytes. Not gigabytes. Go ahead, check again, we'll wait.)

| App | Replaces | Does |
|---|---|---|
| **mikocokWord** | Pages (the one that wandered into your `.docx`) | Opens & edits `.docx` |
| **mikocokExcel** | Numbers (the one that wandered into your `.xlsx`) | Opens & edits `.xlsx` |

No Office. No subscription. No login. No "Start your 30-day free trial" popup. No AI assistant offering to *"summarize this document?"* when all you wanted was to fix a typo.

Written in Rust with [egui](https://github.com/emilk/egui). Both apps are secretly **the same binary** wearing two different hats. Very sustainable. Very startup.

---

## mikocokWord: looks like Word, not like a ransom note

Documents show up as **actual pages**: white paper, the real page size and margins from your file, not one endless scroll like a Twitter thread.

- **Real pages**: automatic page breaks, manual breaks respected, a page counter, zoom.
- **Formatting you can see**: fonts, sizes, bold, italic, underline, strikethrough, colors, highlight, headings, justified text, spacing, indents.
- **Lists, tables, images**: bullets, numbering, tables with borders and shaded cells, PNG/JPEG images.
- **Type right on the page**: click, type, done. No "enter edit mode" ceremony.
  - `Enter` makes a new paragraph (it inherits the formatting, like family money).
  - `Backspace` at the start of a paragraph merges it into the one above.
  - Arrow keys hop between paragraphs.
  - Text you change **keeps its bold, italics, and color**, because we have manners.

## mikocokExcel: a grid, formulas, zero drama

A `.xlsx` spreadsheet grid with sheet tabs, a formula bar, and in-place cell editing. Type `=...` and it's a formula. Type a number and it's a number. Type your feelings and they're text. Simple.

---

## Your files are safe (the important part, seriously)

mikocok does **not** "open it, re-export everything, and hand back something that looks vaguely similar." It edits the XML inside the file and **rewrites only what you changed**. Styles, images, charts, headers, footers, comments, and everything else it doesn't understand are left exactly as they were. The policy is: *if you don't understand it, don't touch it.* Good life advice, too.

Saving goes to a temp file first, so if something fails halfway, your original isn't left truncated and sad.

---

## Install

Needs macOS and [Rust](https://rustup.rs).

```sh
git clone https://github.com/luthfirahmn09/mikocok.git
cd mikocok
./build_app.sh            # builds and installs to ~/Applications
./build_app.sh --default  # ...and makes them the default for .docx / .xlsx
```

Then right-click a file → **Open With → mikocokWord / mikocokExcel**, double-click it if you set the defaults, or drag a file onto the window.

If macOS gets dramatic on first launch (the apps are only ad-hoc signed, not blessed by Apple), right-click → Open, once. Peace follows.

Shortcuts: `⌘O` open, `⌘S` save, `⇧⌘S` save as.

---

## Honesty section: what it can't do

This is a **minimal** editor, not a Microsoft killer. Do not expect it to build pivot tables while you make coffee.

- **Word**: no bold/italic/style toolbar yet. Existing formatting is displayed and preserved, but you can't apply new formatting. No headers/footers, columns, text boxes, or track changes. `Shift+Enter` doesn't work. List numbers after a freshly created item only fix themselves when you reopen the file.
- **Page breaks** come from mikocok's own layout, so they can differ slightly from Word's: paragraphs aren't split across pages, and fonts missing on your Mac (say, Calibri without Office) are swapped for Arial.
- **Excel**: no charts, number/date formatting, merged-cell display, or formula evaluation. Formula cells show their last saved value, and Excel recalculates when you open the file there.
- **Images**: PNG and JPEG only.

If you need more than that, well, Office is still out there. It isn't going anywhere. Believe us, we checked.

---

## For the tinkerers

```sh
python3 tools/make_samples.py   # writes sample .docx/.xlsx fixtures for the tests
cargo test
cargo run --release -- path/to/file.docx
```

| File | What it does |
|---|---|
| `src/docx.rs` | Word model: styles, numbering, tables, parser, and the XML rewriter (edit / split / join paragraphs) |
| `src/docview.rs` | Page rendering, pagination, in-place editing with egui |
| `src/xlsx.rs` | Excel reader and per-cell XML rewriter |
| `src/ooxml.rs` | Zip and XML plumbing |
| `src/fonts.rs` | Borrows system fonts so documents use the fonts Word would |
| `build_app.sh` | Builds the two `.app` bundles, "Open With" included |

There are unit tests for parsing, edit round-trips, and headless page layout. Visual fidelity against real Word is *best effort*. If a document renders weirdly, open an issue and attach the file. We enjoy a good argument with an ugly document.

---

*mikocok: because sometimes you just want to fix one word, not attend an onboarding webinar.*
