//! Word-like page rendering of a `Docx`: paper with real margins, fonts,
//! character/paragraph formatting, lists, tables and images. Text stays
//! editable in place.
use crate::docx::{Block, Docx, Img, Jc, Para, RPr, Rgb, Table};
use crate::fonts;
use eframe::egui::epaint::StrokeKind;
use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{
    self, pos2, text::CCursor, text_selection::CCursorRange, vec2, Align, Color32, ColorImage, FontId, Frame, Id, Key,
    Label, Layout, Margin, Rect, RichText, Sense, Shape, Stroke, TextBuffer, TextEdit, TextureHandle, TextureOptions, Ui,
    UiBuilder, Vec2,
};
use std::collections::HashMap;

const PAGE_GAP: f32 = 24.0;

enum Action {
    Split { id: usize, at: usize },
    Merge { id: usize },
    Nav { id: usize, up: bool },
}

struct Rc<'a> {
    scale: f32,
    textures: &'a mut HashMap<String, Option<TextureHandle>>,
    media: &'a HashMap<String, Vec<u8>>,
    heights: &'a mut HashMap<(usize, usize), f32>,
    actions: Vec<Action>,
    focus: Option<(usize, usize)>,
    repaint: bool,
}

impl Rc<'_> {
    fn record(&mut self, key: (usize, usize), h: f32) {
        let old = self.heights.insert(key, h);
        if old.is_none_or(|o| (o - h).abs() > 0.5) {
            self.repaint = true;
        }
    }
}

/// Lays blocks out page by page: a block that does not fit in what is left of
/// the page moves to the next one (heights come from the previous frame).
struct Pager {
    top0: f32,
    page_h: f32,
    step: f32,
    gap: f32,
    k: usize,
}

impl Pager {
    fn top(&self) -> f32 {
        self.top0 + self.k as f32 * self.step
    }

    fn before(&mut self, ui: &mut Ui, need: f32, force: bool) {
        let y = ui.cursor().min.y - self.top();
        if y < 1.0 {
            return;
        }
        if force || (need > self.page_h - y + 0.5 && need <= self.page_h) {
            ui.add_space((self.page_h - y).max(0.0) + self.gap);
            self.k += 1;
        }
    }

    fn after(&mut self, ui: &mut Ui) {
        while ui.cursor().min.y > self.top() + self.page_h + 0.5 {
            self.k += 1;
        }
        let y = ui.cursor().min.y;
        if y < self.top() {
            ui.add_space(self.top() - y);
        }
    }
}

fn color(c: Rgb) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

pub fn show(ui: &mut Ui, d: &mut Docx, zoom: f32) {
    let focus = d.focus.take();
    let Docx { paras, blocks, page, media, textures, heights, .. } = d;
    let mut actions = Vec::new();
    let mut repaint = false;
    let mut pages = 1;
    let avail = ui.available_width();
    ui.painter().rect_filled(ui.max_rect(), 0.0, Color32::from_gray(214));
    let scale = ((avail - 48.0) / page.w).clamp(0.4, 96.0 / 72.0) * zoom;

    egui::ScrollArea::both().auto_shrink(false).show(ui, |ui| {
        let page_w = page.w * scale;
        let pad = ((avail - page_w) / 2.0).max(16.0);
        ui.add_space(20.0);
        let origin = ui.cursor().min + vec2(pad, 0.0);
        let bg = ui.painter().add(Shape::Noop);

        let content_w = ((page.w - page.l - page.r) * scale).max(40.0);
        let rect = Rect::from_min_size(origin + vec2(page.l * scale, page.t * scale), vec2(content_w, 1.0e6));
        let mut child = ui.new_child(UiBuilder::new().max_rect(rect).layout(Layout::top_down(Align::Min)).id_salt("page"));
        child.spacing_mut().item_spacing = Vec2::ZERO;
        *child.visuals_mut() = egui::Visuals::light();
        child.set_width(content_w);

        let page_px_h = page.h * scale;
        let mut pager = Pager {
            top0: rect.min.y,
            page_h: ((page.h - page.t - page.b) * scale).max(40.0),
            step: page_px_h + PAGE_GAP,
            gap: PAGE_GAP + (page.t + page.b) * scale,
            k: 0,
        };
        let mut rc = Rc { scale, textures, media, heights, actions: Vec::new(), focus, repaint: false };
        blocks_ui(&mut child, blocks, paras, &mut rc, content_w, Some(&mut pager));
        (actions, repaint) = (rc.actions, rc.repaint);
        pages = pager.k + 1;

        let mut shapes = Vec::new();
        for k in 0..=pager.k {
            let r = Rect::from_min_size(pos2(origin.x, origin.y + k as f32 * pager.step), vec2(page_w, page_px_h));
            shapes.push(Shape::rect_filled(r.translate(vec2(2.0, 3.0)), 0.0, Color32::from_black_alpha(45)));
            shapes.push(Shape::rect_filled(r, 0.0, Color32::WHITE));
        }
        ui.painter().set(bg, Shape::Vec(shapes));
        let end = pos2(origin.x + page_w + pad, origin.y + pager.k as f32 * pager.step + page_px_h + PAGE_GAP);
        ui.allocate_rect(Rect::from_min_max(ui.cursor().min, end), Sense::hover());
    });

    d.pages = pages;
    for a in actions {
        match a {
            Action::Split { id, at } => d.focus = d.split_para(id, at).map(|n| (n, 0)).or(d.focus),
            Action::Merge { id } => d.focus = d.merge_up(id).or(d.focus),
            Action::Nav { id, up } => {
                let order = d.order();
                let pos = order.iter().position(|&x| x == id);
                let target = pos.and_then(|p| if up { p.checked_sub(1) } else { Some(p + 1) }).and_then(|k| order.get(k).copied());
                if let Some(t) = target {
                    d.focus = Some((t, if up { d.paras[t].text.chars().count() } else { 0 }));
                }
            }
        }
        repaint = true;
    }
    if repaint {
        ui.ctx().request_repaint();
    }
}

fn blocks_ui(ui: &mut Ui, blocks: &[Block], paras: &mut [Para], rc: &mut Rc, width: f32, mut pager: Option<&mut Pager>) {
    for (bi, b) in blocks.iter().enumerate() {
        match b {
            Block::Para(i) => {
                let id = *i;
                if let Some(pg) = pager.as_deref_mut() {
                    let height = |n: usize| rc.heights.get(&(n, usize::MAX)).copied().unwrap_or(0.0);
                    let mut need = height(id);
                    if paras[id].is_heading() {
                        if let Some(Block::Para(n)) = blocks.get(bi + 1) {
                            need += height(*n); // keep a heading with the paragraph after it
                        }
                    }
                    pg.before(ui, need, paras[id].page_break);
                }
                let y0 = ui.cursor().min.y;
                para_ui(ui, id, &mut paras[id], rc, width);
                rc.record((id, usize::MAX), ui.cursor().min.y - y0);
                if let Some(pg) = pager.as_deref_mut() {
                    if paras[id].break_after {
                        pg.before(ui, 0.0, true);
                    }
                    pg.after(ui);
                }
            }
            Block::Table(t) => table_ui(ui, t, paras, rc, width, pager.as_deref_mut()),
        }
    }
}

fn text_format(r: &RPr, scale: f32, line_mult: Option<(f32, bool)>) -> TextFormat {
    let size = r.size.unwrap_or(11.0);
    let vert = r.vert.unwrap_or(0);
    let px = size * scale * if vert != 0 { 0.65 } else { 1.0 };
    let col = r.color.map(color).unwrap_or(Color32::BLACK);
    let line_height = Some(match line_mult {
        Some((v, true)) => size * scale * 1.17 * v,
        Some((v, false)) => (v * scale).max(size * scale),
        None => size * scale * 1.17,
    });
    TextFormat {
        font_id: FontId::new(px, fonts::family(r.font.as_deref(), r.bold == Some(true), r.italic == Some(true))),
        color: col,
        background: r.fill.map(color).unwrap_or(Color32::TRANSPARENT),
        underline: if r.underline == Some(true) { Stroke::new(1.0, col) } else { Stroke::NONE },
        strikethrough: if r.strike == Some(true) { Stroke::new(1.0, col) } else { Stroke::NONE },
        valign: if vert > 0 { Align::TOP } else { Align::BOTTOM },
        line_height,
        ..Default::default()
    }
}

fn para_ui(ui: &mut Ui, idx: usize, p: &mut Para, rc: &mut Rc, width: f32) {
    let s = rc.scale;
    let ppr = p.ppr.clone();
    ui.add_space(ppr.before.unwrap_or(0.0) * s);
    for img in &p.imgs {
        image_ui(ui, img, ppr.jc, width, rc);
    }

    let left = ppr.left.unwrap_or(0.0) * s;
    let right = ppr.right.unwrap_or(0.0) * s;
    let first = ppr.first.unwrap_or(0.0) * s;
    let text_left = if p.label.is_some() { left } else { left + first.min(0.0) }.max(0.0);
    let lead = if p.label.is_some() { 0.0 } else { first.max(0.0) };
    let tw = (width - text_left - right).max(30.0);

    // Formatting per slot, as (chars, format); the tail reuses the last one.
    let runs: Vec<(usize, TextFormat)> =
        p.slots.iter().map(|sl| (sl.text.chars().count(), text_format(&sl.fmt, s, ppr.line))).collect();
    let tail = runs.last().map(|r| r.1.clone()).unwrap_or_else(|| text_format(&p.base, s, ppr.line));
    let min_h = tail.line_height.unwrap_or(0.0);
    let (halign, justify) = match ppr.jc {
        Some(Jc::Center) => (Align::Center, false),
        Some(Jc::Right) => (Align::RIGHT, false),
        Some(Jc::Both) => (Align::LEFT, true),
        _ => (Align::LEFT, false),
    };

    let mut layouter = |ui: &Ui, text: &dyn TextBuffer, wrap: f32| {
        let text = text.as_str();
        let mut job = LayoutJob::default();
        job.wrap.max_width = wrap;
        job.halign = halign;
        job.justify = justify;
        job.first_row_min_height = min_h;
        let (mut start, mut lead) = (0usize, lead);
        for (n, tf) in &runs {
            let end = text[start..].char_indices().nth(*n).map_or(text.len(), |(b, _)| start + b);
            if end > start {
                job.append(&text[start..end], lead, tf.clone());
                lead = 0.0;
                start = end;
            }
        }
        if start < text.len() {
            job.append(&text[start..], lead, tail.clone());
        }
        ui.ctx().fonts_mut(|f| f.layout_job(job))
    };

    let label_tf = p.slots.first().map(|sl| text_format(&sl.fmt, s, None)).unwrap_or_else(|| text_format(&p.base, s, None));
    let mut changed = false;
    let id = Id::new(("para", idx));
    let ctx = ui.ctx().clone();
    if rc.focus.is_some_and(|f| f.0 == idx) {
        let at = rc.focus.take().map_or(0, |f| f.1);
        let mut st = TextEdit::load_state(&ctx, id).unwrap_or_default();
        st.cursor.set_char_range(Some(CCursorRange::one(CCursor::new(at))));
        st.store(&ctx, id);
        ctx.memory_mut(|m| m.request_focus(id));
    }
    let before_cursor = TextEdit::load_state(&ctx, id).and_then(|st| st.cursor.char_range());
    let mut action = None;
    ui.push_id(idx, |ui| {
        ui.horizontal_top(|ui| {
            if let Some(lbl) = &p.label {
                let lx = (left + first).max(0.0);
                ui.add_space(lx);
                let lw = (left - lx).max(12.0 * s);
                ui.allocate_ui_with_layout(vec2(lw, 1.0), Layout::left_to_right(Align::TOP), |ui| {
                    ui.add(Label::new(RichText::new(lbl).font(label_tf.font_id.clone()).color(label_tf.color)).extend());
                });
            } else {
                ui.add_space(text_left);
            }
            let out = TextEdit::multiline(&mut p.text)
                .id(id)
                .layouter(&mut layouter)
                .desired_width(tw)
                .desired_rows(1)
                .frame(Frame::NONE)
                .margin(Margin::ZERO)
                .horizontal_align(halign)
                .return_key(None)
                .show(ui);
            changed = out.response.changed();
            if let (true, Some(r)) = (out.response.has_focus(), before_cursor) {
                let (at, alone) = (usize::from(r.primary.index), r.primary == r.secondary);
                let (enter, back, up, down) = ctx.input(|i| {
                    let plain = !i.modifiers.shift && !i.modifiers.command && !i.modifiers.alt;
                    (i.key_pressed(Key::Enter) && plain, i.key_pressed(Key::Backspace), i.key_pressed(Key::ArrowUp), i.key_pressed(Key::ArrowDown))
                });
                let row = out.galley.layout_from_cursor(r.primary).row;
                action = if enter && alone {
                    Some(Action::Split { id: idx, at })
                } else if back && alone && at == 0 {
                    Some(Action::Merge { id: idx })
                } else if up && alone && row == 0 {
                    Some(Action::Nav { id: idx, up: true })
                } else if down && alone && row + 1 >= out.galley.rows.len() {
                    Some(Action::Nav { id: idx, up: false })
                } else {
                    None
                };
            }
        });
    });
    if changed {
        p.sync();
    }
    rc.actions.extend(action);
    ui.add_space(ppr.after.unwrap_or(0.0) * s);
}

fn load_texture(ctx: &egui::Context, rid: &str, media: &HashMap<String, Vec<u8>>) -> Option<TextureHandle> {
    let img = image::load_from_memory(media.get(rid)?).ok()?.to_rgba8();
    let size = [img.width() as usize, img.height() as usize];
    Some(ctx.load_texture(rid, ColorImage::from_rgba_unmultiplied(size, img.as_raw()), TextureOptions::LINEAR))
}

fn image_ui(ui: &mut Ui, img: &Img, jc: Option<Jc>, width: f32, rc: &mut Rc) {
    let (media, s) = (rc.media, rc.scale);
    let tex = rc.textures.entry(img.rid.clone()).or_insert_with(|| load_texture(ui.ctx(), &img.rid, media));
    let Some(tex) = tex else { return };
    let (mut w, mut h) = (img.w * s, img.h * s);
    if w < 1.0 || h < 1.0 {
        let sz = tex.size_vec2();
        (w, h) = (sz.x * 0.75 * s, sz.y * 0.75 * s);
    }
    if w > width {
        h *= width / w;
        w = width;
    }
    let off = match jc {
        Some(Jc::Center) => (width - w) / 2.0,
        Some(Jc::Right) => width - w,
        _ => 0.0,
    };
    ui.horizontal(|ui| {
        ui.add_space(off);
        ui.image((tex.id(), vec2(w, h)));
    });
}

fn table_ui(ui: &mut Ui, t: &Table, paras: &mut [Para], rc: &mut Rc, width: f32, mut pager: Option<&mut Pager>) {
    let tkey = t as *const Table as usize;
    let s = rc.scale;
    let (pad_x, pad_y) = (5.0 * s, 2.0 * s);
    let line = Stroke::new(1.0, Color32::from_gray(70));
    for (ri, row) in t.rows.iter().enumerate() {
        if let Some(pg) = pager.as_deref_mut() {
            let need = rc.heights.get(&(tkey, ri)).copied().unwrap_or(0.0);
            pg.before(ui, need, false);
        }
        let mut col = 0usize;
        let widths: Vec<f32> = row
            .iter()
            .map(|c| {
                let span = c.span.max(1);
                let w = if t.grid.len() >= col + span { t.grid[col..col + span].iter().sum() } else { c.width };
                col += span;
                if w > 0.0 { w } else { 60.0 }
            })
            .collect();
        let total = widths.iter().sum::<f32>() * s;
        let k = if total > width { width / total } else { 1.0 };

        let top = ui.cursor().min;
        let mut x = top.x;
        let mut max_h = 16.0 * s;
        let mut cells = Vec::new();
        for (cell, w) in row.iter().zip(&widths) {
            let w = w * s * k;
            let bg = ui.painter().add(Shape::Noop);
            let r = Rect::from_min_size(pos2(x + pad_x, top.y + pad_y), vec2((w - 2.0 * pad_x).max(10.0), 1.0e5));
            let salt = ("cell", cell as *const _ as usize);
            let mut child = ui.new_child(UiBuilder::new().max_rect(r).layout(Layout::top_down(Align::Min)).id_salt(salt));
            child.spacing_mut().item_spacing = Vec2::ZERO;
            child.set_width(r.width());
            if !cell.vmerge {
                blocks_ui(&mut child, &cell.blocks, paras, rc, r.width(), None);
            }
            max_h = max_h.max(child.min_rect().height() + 2.0 * pad_y);
            cells.push((bg, x, w, cell.fill));
            x += w;
        }
        for (bg, cx, w, fill) in cells {
            let rect = Rect::from_min_size(pos2(cx, top.y), vec2(w, max_h));
            let mut shapes = Vec::new();
            if let Some(f) = fill {
                shapes.push(Shape::rect_filled(rect, 0.0, color(f)));
            }
            if t.borders {
                shapes.push(Shape::rect_stroke(rect, 0.0, line, StrokeKind::Inside));
            }
            ui.painter().set(bg, Shape::Vec(shapes));
        }
        ui.allocate_space(vec2(x - top.x, max_h));
        rc.record((tkey, ri), max_h);
        if let Some(pg) = pager.as_deref_mut() {
            pg.after(ui);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(path: &str, n: usize) -> Docx {
        let mut d = Docx::open(&std::fs::read(path).unwrap()).unwrap();
        let ctx = egui::Context::default();
        fonts::install(&ctx);
        for _ in 0..n {
            let input = egui::RawInput {
                screen_rect: Some(Rect::from_min_size(pos2(0.0, 0.0), vec2(1000.0, 700.0))),
                ..Default::default()
            };
            let _ = ctx.run_ui(input, |ui| show(ui, &mut d, 1.0));
        }
        d
    }

    #[test]
    fn short_doc_is_one_page() {
        let d = frames(&crate::ooxml::sample("rich.docx"), 4);
        assert_eq!(d.pages, 1);
    }

    #[test]
    fn long_doc_paginates_and_is_stable() {
        let d = frames(&crate::ooxml::sample("long.docx"), 6);
        eprintln!("long.docx pages = {}", d.pages);
        assert!(d.pages >= 5, "pages = {}", d.pages);
        let d2 = frames(&crate::ooxml::sample("long.docx"), 9);
        assert_eq!(d.pages, d2.pages, "page count settles");
    }
}
