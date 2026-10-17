mod docview;
mod docx;
mod fonts;
mod ooxml;
mod xlsx;

use eframe::egui::{self, Key, KeyboardShortcut, Modifiers, TextEdit};
use egui_extras::{Column, TableBuilder};
use std::path::{Path, PathBuf};

enum Doc {
    Word(docx::Docx),
    Excel(xlsx::Xlsx),
}

impl Doc {
    fn is_dirty(&self) -> bool {
        match self {
            Doc::Word(d) => d.is_dirty(),
            Doc::Excel(x) => x.is_dirty(),
        }
    }

    fn to_bytes(&self) -> Result<Vec<u8>, String> {
        match self {
            Doc::Word(d) => d.to_bytes(),
            Doc::Excel(x) => x.to_bytes(),
        }
    }
}

/// One binary ships as two apps; the launcher sets MIKOCOK to "Word" or "Excel".
fn app_name() -> &'static str {
    match std::env::var("MIKOCOK").as_deref() {
        Ok("Word") => "mikocokWord",
        Ok("Excel") => "mikocokExcel",
        _ => "mikocok",
    }
}

fn exts() -> &'static [&'static str] {
    match std::env::var("MIKOCOK").as_deref() {
        Ok("Word") => &["docx"],
        Ok("Excel") => &["xlsx", "xlsm"],
        _ => &["docx", "xlsx"],
    }
}

#[derive(Default)]
struct App {
    doc: Option<Doc>,
    path: Option<PathBuf>,
    status: String,
    sheet: usize,
    selected: Option<(u32, u32)>,
    title: String,
    zoom: f32,
}

impl App {
    fn open(&mut self, path: &Path) {
        let result = std::fs::read(path).map_err(|e| e.to_string()).and_then(|bytes| {
            match path.extension().and_then(|e| e.to_str()).map(str::to_lowercase).as_deref() {
                Some("docx") => docx::Docx::open(&bytes).map(Doc::Word),
                Some("xlsx") | Some("xlsm") => xlsx::Xlsx::open(&bytes).map(Doc::Excel),
                _ => Err("only .docx and .xlsx files are supported".into()),
            }
        });
        match result {
            Ok(doc) => {
                self.doc = Some(doc);
                self.path = Some(path.to_path_buf());
                self.sheet = 0;
                self.selected = None;
                self.status = format!("Opened: {}", path.display());
            }
            Err(e) => self.status = format!("Could not open: {e}"),
        }
    }

    fn pick_and_open(&mut self) {
        if let Some(p) = rfd::FileDialog::new().add_filter(app_name(), exts()).pick_file() {
            self.open(&p);
        }
    }

    fn save(&mut self, save_as: bool) {
        let Some(doc) = &self.doc else { return };
        let target = match (&self.path, save_as) {
            (Some(p), false) => p.clone(),
            _ => {
                let ext = if matches!(doc, Doc::Word(_)) { "docx" } else { "xlsx" };
                let mut d = rfd::FileDialog::new().add_filter(ext, &[ext]);
                if let Some(name) = self.path.as_ref().and_then(|p| p.file_name()) {
                    d = d.set_file_name(name.to_string_lossy());
                }
                match d.save_file() {
                    Some(p) => p,
                    None => return,
                }
            }
        };
        let written = doc.to_bytes().and_then(|bytes| {
            // Write beside the target first so a failed save never truncates the original.
            let tmp = target.with_extension("tmp-mikocok");
            std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
            std::fs::rename(&tmp, &target).map_err(|e| e.to_string())
        });
        match written {
            Ok(()) => {
                let keep_sheet = self.sheet;
                self.open(&target); // reload so edits are re-based on the saved file
                self.sheet = keep_sheet.min(match &self.doc {
                    Some(Doc::Excel(x)) => x.sheets.len() - 1,
                    _ => 0,
                });
                self.status = format!("Saved: {}", target.display());
            }
            Err(e) => self.status = format!("Could not save: {e}"),
        }
    }

    fn shortcuts(&mut self, ctx: &egui::Context) {
        let sc = |m, k| KeyboardShortcut::new(m, k);
        let (open, save_as, save) = ctx.input_mut(|i| {
            (
                i.consume_shortcut(&sc(Modifiers::COMMAND, Key::O)),
                i.consume_shortcut(&sc(Modifiers::COMMAND | Modifiers::SHIFT, Key::S)),
                i.consume_shortcut(&sc(Modifiers::COMMAND, Key::S)),
            )
        });
        if open {
            self.pick_and_open();
        }
        if save_as {
            self.save(true);
        } else if save {
            self.save(false);
        }
        let dropped = ctx.input(|i| i.raw.dropped_files.first().and_then(|f| f.path.clone()));
        if let Some(p) = dropped {
            self.open(&p);
        }
    }

    fn run(&mut self, root: &mut egui::Ui) {
        let ctx = root.ctx().clone();
        let ctx = &ctx;
        self.shortcuts(ctx);

        let dirty = self.doc.as_ref().is_some_and(Doc::is_dirty);
        let name = self.path.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned());
        let title = match name {
            Some(n) => format!("{}{n} — {}", if dirty { "• " } else { "" }, app_name()),
            None => app_name().into(),
        };
        if title != self.title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.title = title;
        }

        egui::Panel::top("bar").show(root, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open… ⌘O").clicked() {
                    self.pick_and_open();
                }
                if matches!(self.doc, Some(Doc::Word(_))) {
                    ui.separator();
                    if ui.button("−").clicked() {
                        self.zoom = (self.zoom - 0.1).max(0.5);
                    }
                    if ui.button(format!("{:.0}%", self.zoom * 100.0)).clicked() {
                        self.zoom = 1.0;
                    }
                    if ui.button("+").clicked() {
                        self.zoom = (self.zoom + 0.1).min(2.5);
                    }
                    ui.separator();
                }
                ui.add_enabled_ui(self.doc.is_some(), |ui| {
                    if ui.button("Save ⌘S").clicked() {
                        self.save(false);
                    }
                    if ui.button("Save As… ⇧⌘S").clicked() {
                        self.save(true);
                    }
                });
            });
        });

        egui::Panel::bottom("status").show(root, |ui| {
            ui.horizontal(|ui| {
                if let Some(Doc::Excel(x)) = &self.doc {
                    for (i, s) in x.sheets.iter().enumerate() {
                        if ui.selectable_label(self.sheet == i, &s.name).clicked() {
                            self.sheet = i;
                            self.selected = None;
                        }
                    }
                    ui.separator();
                }
                if let Some(Doc::Word(d)) = &self.doc {
                    ui.label(if d.pages == 1 { "1 page".to_string() } else { format!("{} pages", d.pages) });
                    ui.separator();
                }
                ui.weak(&self.status);
            });
        });

        egui::CentralPanel::default().show(root, |ui| match &mut self.doc {
            None => {
                ui.centered_and_justified(|ui| ui.heading(format!("Open or drop a .{} file here", exts().join(" / ."))));
            }
            Some(Doc::Word(d)) => docview::show(ui, d, self.zoom),
            Some(Doc::Excel(x)) => {
                let i = self.sheet.min(x.sheets.len() - 1);
                excel_ui(ui, &mut x.sheets[i], &mut self.selected);
            }
        });
    }
}

fn excel_ui(ui: &mut egui::Ui, s: &mut xlsx::Sheet, selected: &mut Option<(u32, u32)>) {
    let formula = selected
        .and_then(|k| s.cells.get(&k).map(|c| (k, c)))
        .and_then(|(k, c)| c.formula.as_ref().map(|f| (k, f)));
    ui.horizontal(|ui| {
        match selected {
            Some((r, c)) => ui.strong(format!("{}{}", xlsx::col_name(*c), r)),
            None => ui.strong("—"),
        };
        ui.weak("fx");
        match formula {
            Some((_, f)) => ui.label(format!("={f}")),
            None => ui.label(""),
        };
    });
    ui.separator();

    let ncols = (s.max_col + 5).max(12);
    let nrows = (s.max_row + 30).max(100);
    egui::ScrollArea::horizontal().auto_shrink(false).show(ui, |ui| {
        let mut table = TableBuilder::new(ui).striped(true).column(Column::exact(46.0));
        for _ in 0..ncols {
            table = table.column(Column::exact(110.0));
        }
        table
            .header(22.0, |mut h| {
                h.col(|_| {});
                for c in 1..=ncols {
                    h.col(|ui| {
                        ui.strong(xlsx::col_name(c));
                    });
                }
            })
            .body(|body| {
                body.rows(22.0, nrows as usize, |mut row| {
                    let r = row.index() as u32 + 1;
                    row.col(|ui| {
                        ui.weak(r.to_string());
                    });
                    for c in 1..=ncols {
                        row.col(|ui| {
                            let key = (r, c);
                            let orig = s.cells.get(&key);
                            let mut text = match s.edits.get(&key) {
                                Some(t) => t.clone(),
                                None => orig.map(|c| c.text.clone()).unwrap_or_default(),
                            };
                            let resp = ui.add(TextEdit::singleline(&mut text).frame(egui::Frame::NONE).desired_width(f32::INFINITY));
                            if resp.changed() {
                                let unchanged = match orig {
                                    Some(o) => o.formula.is_none() && o.text == text,
                                    None => text.is_empty(),
                                };
                                if unchanged {
                                    s.edits.remove(&key);
                                } else {
                                    s.edits.insert(key, text);
                                }
                            }
                            if resp.has_focus() {
                                *selected = Some(key);
                            }
                        });
                    }
                });
            });
    });
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.run(ui);
    }
}

fn main() -> eframe::Result {
    let mut app = App { zoom: 1.0, ..Default::default() };
    if let Some(p) = std::env::args().nth(1) {
        app.open(Path::new(&p));
    }
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([1000.0, 700.0]).with_title(app_name()),
        ..Default::default()
    };
    eframe::run_native(app_name(), options, Box::new(|cc| {
        fonts::install(&cc.egui_ctx);
        Ok(Box::new(app))
    }))
}
