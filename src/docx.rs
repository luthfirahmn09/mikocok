use crate::ooxml::{attr, parse_rels, Package};
use eframe::egui::TextureHandle;
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::collections::{HashMap, HashSet};
use std::io::{Cursor, Write};

const DOC: &str = "word/document.xml";

pub type Rgb = [u8; 3];

#[derive(Clone, Copy, PartialEq)]
pub enum Jc {
    Left,
    Center,
    Right,
    Both,
}

/// Run properties; `None` means "inherit".
#[derive(Clone, Default)]
pub struct RPr {
    pub font: Option<String>,
    pub size: Option<f32>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underline: Option<bool>,
    pub strike: Option<bool>,
    pub color: Option<Rgb>,
    pub fill: Option<Rgb>,
    pub vert: Option<i8>,
}

/// Paragraph properties (all lengths in points).
#[derive(Clone, Default)]
pub struct PPr {
    pub jc: Option<Jc>,
    pub before: Option<f32>,
    pub after: Option<f32>,
    /// (value, true = multiple of single spacing / false = points)
    pub line: Option<(f32, bool)>,
    pub left: Option<f32>,
    pub right: Option<f32>,
    pub first: Option<f32>,
    pub num_id: Option<u32>,
    pub ilvl: Option<u32>,
    pub fill: Option<Rgb>,
    pub page_break: Option<bool>,
}

macro_rules! merge {
    ($s:ident, $o:ident, $($f:ident),*) => { $( if $o.$f.is_some() { $s.$f = $o.$f.clone(); } )* };
}

impl RPr {
    pub fn merge(&mut self, o: &RPr) {
        merge!(self, o, font, size, bold, italic, underline, strike, color, fill, vert);
    }
}

impl PPr {
    fn merge(&mut self, o: &PPr) {
        merge!(self, o, jc, before, after, line, left, right, first, num_id, ilvl, fill, page_break);
    }
}

fn toggle(e: &BytesStart) -> bool {
    !matches!(attr(e, "w:val").as_deref(), Some("0" | "false" | "off"))
}

fn hex(s: &str) -> Option<Rgb> {
    (s.len() == 6 && s.is_ascii()).then_some(())?;
    let p = |i| u8::from_str_radix(&s[i..i + 2], 16).ok();
    Some([p(0)?, p(2)?, p(4)?])
}

fn twips(e: &BytesStart, key: &str) -> Option<f32> {
    attr(e, key)?.parse::<f32>().ok().map(|v| v / 20.0)
}

fn highlight(name: &str) -> Option<Rgb> {
    Some(match name {
        "yellow" => [255, 255, 0],
        "green" => [0, 255, 0],
        "cyan" => [0, 255, 255],
        "magenta" => [255, 0, 255],
        "blue" => [0, 0, 255],
        "red" => [255, 0, 0],
        "darkBlue" => [0, 0, 128],
        "darkCyan" => [0, 128, 128],
        "darkGreen" => [0, 128, 0],
        "darkMagenta" => [128, 0, 128],
        "darkRed" => [128, 0, 0],
        "darkYellow" => [128, 128, 0],
        "darkGray" => [128, 128, 128],
        "lightGray" => [192, 192, 192],
        "black" => [0, 0, 0],
        _ => return None,
    })
}

fn parse_rpr_child(e: &BytesStart, r: &mut RPr, rstyle: &mut Option<String>) {
    match e.name().as_ref() {
        b"w:rFonts" => r.font = attr(e, "w:ascii").or_else(|| attr(e, "w:hAnsi")).or(r.font.take()),
        b"w:sz" => r.size = attr(e, "w:val").and_then(|v| v.parse::<f32>().ok()).map(|v| v / 2.0),
        b"w:b" => r.bold = Some(toggle(e)),
        b"w:i" => r.italic = Some(toggle(e)),
        b"w:strike" | b"w:dstrike" => r.strike = Some(toggle(e)),
        b"w:u" => r.underline = Some(attr(e, "w:val").as_deref() != Some("none")),
        b"w:color" => r.color = attr(e, "w:val").and_then(|v| hex(&v)).or(r.color),
        b"w:highlight" => r.fill = attr(e, "w:val").and_then(|v| highlight(&v)),
        b"w:shd" => r.fill = attr(e, "w:fill").and_then(|v| hex(&v)).or(r.fill),
        b"w:vertAlign" => {
            r.vert = match attr(e, "w:val").as_deref() {
                Some("superscript") => Some(1),
                Some("subscript") => Some(-1),
                _ => Some(0),
            }
        }
        b"w:rStyle" => *rstyle = attr(e, "w:val"),
        _ => {}
    }
}

fn parse_ppr_child(e: &BytesStart, p: &mut PPr, pstyle: &mut Option<String>) {
    match e.name().as_ref() {
        b"w:pStyle" => *pstyle = attr(e, "w:val"),
        b"w:jc" => {
            p.jc = match attr(e, "w:val").as_deref() {
                Some("center") => Some(Jc::Center),
                Some("right" | "end") => Some(Jc::Right),
                Some("both" | "distribute") => Some(Jc::Both),
                _ => Some(Jc::Left),
            }
        }
        b"w:spacing" => {
            p.before = twips(e, "w:before").or(p.before);
            p.after = twips(e, "w:after").or(p.after);
            if let Some(v) = attr(e, "w:line").and_then(|v| v.parse::<f32>().ok()) {
                let auto = matches!(attr(e, "w:lineRule").as_deref(), None | Some("auto"));
                p.line = Some(if auto { (v / 240.0, true) } else { (v / 20.0, false) });
            }
        }
        b"w:ind" => {
            p.left = twips(e, "w:left").or(twips(e, "w:start")).or(p.left);
            p.right = twips(e, "w:right").or(twips(e, "w:end")).or(p.right);
            if let Some(h) = twips(e, "w:hanging") {
                p.first = Some(-h);
            } else if let Some(f) = twips(e, "w:firstLine") {
                p.first = Some(f);
            }
        }
        b"w:numId" => p.num_id = attr(e, "w:val").and_then(|v| v.parse().ok()),
        b"w:ilvl" => p.ilvl = attr(e, "w:val").and_then(|v| v.parse().ok()),
        b"w:shd" => p.fill = attr(e, "w:fill").and_then(|v| hex(&v)),
        b"w:pageBreakBefore" => p.page_break = Some(toggle(e)),
        _ => {}
    }
}

// ---------------------------------------------------------------- styles

#[derive(Default)]
struct Style {
    based: Option<String>,
    ppr: PPr,
    rpr: RPr,
    default: bool,
}

#[derive(Default)]
struct Styles {
    def_ppr: PPr,
    def_rpr: RPr,
    default_para: String,
    resolved: HashMap<String, (PPr, RPr)>,
}

impl Styles {
    fn para(&self, id: &Option<String>) -> (PPr, RPr) {
        let key = id.as_deref().unwrap_or(&self.default_para);
        self.resolved.get(key).cloned().unwrap_or_default()
    }

    fn rpr(&self, id: &Option<String>) -> RPr {
        id.as_ref().and_then(|k| self.resolved.get(k)).map(|s| s.1.clone()).unwrap_or_default()
    }
}

fn parse_styles(xml: &str) -> Result<Styles, String> {
    let mut r = Reader::from_str(xml);
    let mut st = Styles::default();
    let mut raw: HashMap<String, Style> = HashMap::new();
    let mut cur: Option<(String, Style)> = None;
    let (mut in_ppr, mut in_rpr, mut rpr_in_ppr, mut tbl_style, mut defaults) = (false, false, false, 0u32, false);
    let mut dummy = None;
    loop {
        let ev = r.read_event().map_err(|e| e.to_string())?;
        let (e, empty) = match &ev {
            Event::Start(e) => (e, false),
            Event::Empty(e) => (e, true),
            Event::End(e) => {
                match e.name().as_ref() {
                    b"w:pPr" => in_ppr = false,
                    b"w:rPr" => in_rpr = false,
                    b"w:tblStylePr" => tbl_style -= 1,
                    b"w:docDefaults" => defaults = false,
                    b"w:style" => {
                        if let Some((id, s)) = cur.take() {
                            if s.default {
                                st.default_para = id.clone();
                            }
                            raw.insert(id, s);
                        }
                    }
                    _ => {}
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        match e.name().as_ref() {
            b"w:docDefaults" => defaults = true,
            b"w:tblStylePr" if !empty => tbl_style += 1,
            b"w:style" if !empty => {
                let ty = attr(e, "w:type").unwrap_or_default();
                cur = Some((
                    attr(e, "w:styleId").unwrap_or_default(),
                    Style { default: ty == "paragraph" && attr(e, "w:default").as_deref() == Some("1"), ..Default::default() },
                ));
            }
            b"w:basedOn" => {
                if let Some((_, s)) = &mut cur {
                    s.based = attr(e, "w:val");
                }
            }
            b"w:pPr" if !empty => in_ppr = true,
            b"w:rPr" if !empty => {
                in_rpr = true;
                rpr_in_ppr = in_ppr;
            }
            _ if tbl_style > 0 => {}
            _ if in_rpr && !rpr_in_ppr => match (&mut cur, defaults) {
                (Some((_, s)), _) => parse_rpr_child(e, &mut s.rpr, &mut dummy),
                (None, true) => parse_rpr_child(e, &mut st.def_rpr, &mut dummy),
                _ => {}
            },
            _ if in_ppr && !in_rpr => match (&mut cur, defaults) {
                (Some((_, s)), _) => parse_ppr_child(e, &mut s.ppr, &mut dummy),
                (None, true) => parse_ppr_child(e, &mut st.def_ppr, &mut dummy),
                _ => {}
            },
            _ => {}
        }
    }
    // flatten basedOn chains: base first, then the style itself
    for id in raw.keys() {
        let mut chain = Vec::new();
        let mut at = Some(id.as_str());
        while let Some(k) = at.filter(|_| chain.len() < 24) {
            let Some(s) = raw.get(k) else { break };
            chain.push(s);
            at = s.based.as_deref();
        }
        let (mut p, mut rp) = (PPr::default(), RPr::default());
        for s in chain.iter().rev() {
            p.merge(&s.ppr);
            rp.merge(&s.rpr);
        }
        st.resolved.insert(id.clone(), (p, rp));
    }
    Ok(st)
}

// ------------------------------------------------------------- numbering

struct Lvl {
    fmt: String,
    text: String,
    start: u32,
    left: Option<f32>,
    first: Option<f32>,
}

#[derive(Default)]
struct Numbering {
    abstracts: HashMap<u32, Vec<Lvl>>,
    nums: HashMap<u32, u32>,
}

fn parse_numbering(xml: &str) -> Result<Numbering, String> {
    let mut r = Reader::from_str(xml);
    let mut n = Numbering::default();
    let (mut abs_id, mut num_id, mut lvl): (Option<u32>, Option<u32>, Option<(usize, Lvl)>) = (None, None, None);
    let mut dummy = None;
    loop {
        match r.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) | Event::Empty(e) => match e.name().as_ref() {
                b"w:abstractNum" => {
                    abs_id = attr(&e, "w:abstractNumId").and_then(|v| v.parse().ok());
                    if let Some(a) = abs_id {
                        n.abstracts.entry(a).or_default();
                    }
                }
                b"w:num" => num_id = attr(&e, "w:numId").and_then(|v| v.parse().ok()),
                b"w:abstractNumId" => {
                    if let (Some(nid), Some(a)) = (num_id, attr(&e, "w:val").and_then(|v| v.parse().ok())) {
                        n.nums.insert(nid, a);
                    }
                }
                b"w:lvl" => {
                    let i = attr(&e, "w:ilvl").and_then(|v| v.parse().ok()).unwrap_or(0);
                    lvl = Some((i, Lvl { fmt: "decimal".into(), text: String::new(), start: 1, left: None, first: None }));
                }
                b"w:start" => {
                    if let Some((_, l)) = &mut lvl {
                        l.start = attr(&e, "w:val").and_then(|v| v.parse().ok()).unwrap_or(1);
                    }
                }
                b"w:numFmt" => {
                    if let Some((_, l)) = &mut lvl {
                        l.fmt = attr(&e, "w:val").unwrap_or_default();
                    }
                }
                b"w:lvlText" => {
                    if let Some((_, l)) = &mut lvl {
                        l.text = attr(&e, "w:val").unwrap_or_default();
                    }
                }
                b"w:ind" => {
                    if let Some((_, l)) = &mut lvl {
                        let mut p = PPr::default();
                        parse_ppr_child(&e, &mut p, &mut dummy);
                        (l.left, l.first) = (p.left, p.first);
                    }
                }
                _ => {}
            },
            Event::End(e) => match e.name().as_ref() {
                b"w:lvl" => {
                    if let (Some((i, l)), Some(a)) = (lvl.take(), abs_id) {
                        let v = n.abstracts.entry(a).or_default();
                        if v.len() <= i {
                            v.resize_with(i + 1, || Lvl { fmt: "decimal".into(), text: String::new(), start: 1, left: None, first: None });
                        }
                        v[i] = l;
                    }
                }
                b"w:num" => num_id = None,
                _ => {}
            },
            Event::Eof => return Ok(n),
            _ => {}
        }
    }
}

fn format_number(n: u32, fmt: &str) -> String {
    let letters = |upper: bool| {
        let base = if upper { b'A' } else { b'a' };
        let (mut n, mut s) = (n.max(1) - 1, Vec::new());
        loop {
            s.push(base + (n % 26) as u8);
            if n < 26 {
                break;
            }
            n = n / 26 - 1;
        }
        s.reverse();
        String::from_utf8(s).unwrap_or_default()
    };
    let roman = |upper: bool| {
        let (mut n, mut out) = (n, String::new());
        for (v, r) in [(1000, "m"), (900, "cm"), (500, "d"), (400, "cd"), (100, "c"), (90, "xc"), (50, "l"), (40, "xl"), (10, "x"), (9, "ix"), (5, "v"), (4, "iv"), (1, "i")] {
            while n >= v {
                out.push_str(r);
                n -= v;
            }
        }
        if upper { out.to_uppercase() } else { out }
    };
    match fmt {
        "lowerLetter" => letters(false),
        "upperLetter" => letters(true),
        "lowerRoman" => roman(false),
        "upperRoman" => roman(true),
        _ => n.to_string(),
    }
}

// ----------------------------------------------------------------- model

#[derive(Clone, Copy, PartialEq)]
pub enum Kind {
    Text,
    Tab,
    Br,
}

/// One editable unit of a paragraph: a `w:t`, a tab or a line break.
#[derive(Clone)]
pub struct Slot {
    pub kind: Kind,
    pub text: String,
    pub fmt: RPr,
    /// Raw `<w:rPr>` of the run, reused when this slot is written into a new run.
    pub rpr_xml: Option<String>,
}

pub struct Img {
    pub rid: String,
    pub w: f32,
    pub h: f32,
}

pub struct Para {
    pub slots: Vec<Slot>,
    /// Edit buffer: concatenation of the slot texts.
    pub text: String,
    orig: Vec<String>,
    pub ppr: PPr,
    pub base: RPr,
    pub label: Option<String>,
    pub imgs: Vec<Img>,
    pub page_break: bool,
    pub break_after: bool,
    pub style: Option<String>,
    /// Raw `<w:pPr>` copied into paragraphs created by Enter.
    ppr_xml: String,
    /// Slots before an explicit page break (`w:br type=page`), if any.
    br_at: Option<usize>,
    /// Removed by joining it into the previous paragraph.
    pub deleted: bool,
}

fn remove_between(mut x: String, open: &str, close: &str) -> String {
    while let Some(a) = x.find(open) {
        let Some(b) = x[a..].find(close) else { break };
        x.replace_range(a..a + b + close.len(), "");
    }
    x
}

/// "3." -> "4.", "b)" stays; bullets are unchanged.
fn next_label(l: &Option<String>) -> Option<String> {
    let l = l.as_ref()?;
    let Some(a) = l.find(|c: char| c.is_ascii_digit()) else { return Some(l.clone()) };
    let b = l[a..].find(|c: char| !c.is_ascii_digit()).map_or(l.len(), |i| a + i);
    let n: u32 = l[a..b].parse().ok()?;
    Some(format!("{}{}{}", &l[..a], n + 1, &l[b..]))
}

impl Para {
    fn new() -> Self {
        Para {
            slots: vec![],
            text: String::new(),
            orig: vec![],
            ppr: PPr::default(),
            base: RPr::default(),
            label: None,
            imgs: vec![],
            page_break: false,
            break_after: false,
            style: None,
            ppr_xml: String::new(),
            br_at: None,
            deleted: false,
        }
    }

    fn dirty(&self) -> bool {
        self.slots.len() != self.orig.len() || self.slots.iter().zip(&self.orig).any(|(s, o)| s.text != *o)
    }

    pub fn is_heading(&self) -> bool {
        self.style.as_deref().is_some_and(|s| {
            let s = s.to_lowercase();
            s.starts_with("heading") || s == "title" || s == "subtitle"
        })
    }

    /// Enter at char offset `at`: everything after it moves to a new paragraph.
    /// The moved slots stay behind as empty placeholders so slot indices keep
    /// matching the original XML elements.
    fn split_at(&mut self, at: usize, normal: &(PPr, RPr)) -> Para {
        let heading_end = self.is_heading() && at >= self.text.chars().count();
        let mut moved: Vec<Slot> = Vec::new();
        let mut pos = 0;
        for slot in &mut self.slots {
            let len = slot.text.chars().count();
            let start = pos;
            pos += len;
            if start + len <= at {
                continue;
            }
            if start >= at {
                let mut m = slot.clone();
                m.text = std::mem::take(&mut slot.text);
                moved.push(m);
            } else {
                let cut = slot.text.char_indices().nth(at - start).map_or(slot.text.len(), |(b, _)| b);
                let mut m = slot.clone();
                m.text = slot.text.split_off(cut);
                moved.push(m);
            }
        }
        self.text = self.slots.iter().map(|s| s.text.as_str()).collect();
        let mut n = Para::new();
        n.text = moved.iter().map(|s| s.text.as_str()).collect();
        n.slots = moved;
        if heading_end {
            n.ppr = normal.0.clone();
            n.base = normal.1.clone();
        } else {
            n.ppr = self.ppr.clone();
            n.base = self.base.clone();
            n.style = self.style.clone();
            n.label = next_label(&self.label);
            n.ppr_xml = remove_between(
                remove_between(self.ppr_xml.clone(), "<w:sectPr", "</w:sectPr>"),
                "<w:pageBreakBefore",
                "/>",
            );
        }
        n.ppr.page_break = None;
        n
    }

    /// Pushes a change made in `text` back into the slots, so every run keeps
    /// its own formatting. Works on the common prefix/suffix of old and new.
    pub fn sync(&mut self) {
        let old: String = self.slots.iter().map(|s| s.text.as_str()).collect();
        if old == self.text {
            return;
        }
        let (o, n): (Vec<char>, Vec<char>) = (old.chars().collect(), self.text.chars().collect());
        let mut p = 0;
        while p < o.len() && p < n.len() && o[p] == n[p] {
            p += 1;
        }
        let mut s = 0;
        while s < o.len() - p && s < n.len() - p && o[o.len() - 1 - s] == n[n.len() - 1 - s] {
            s += 1;
        }
        let (del_a, del_b) = (p, o.len() - s);
        let ins: String = n[p..n.len() - s].iter().collect();

        let mut pos = 0;
        let mut replaced: Option<(usize, usize)> = None; // (slot, char offset) of the first deleted text
        for (i, slot) in self.slots.iter_mut().enumerate() {
            let len = slot.text.chars().count();
            let (a, b) = (pos, pos + len);
            pos = b;
            let (ds, de) = (del_a.max(a), del_b.min(b));
            if ds >= de {
                continue;
            }
            if slot.kind == Kind::Text && replaced.is_none() {
                replaced = Some((i, ds - a));
            }
            if slot.kind == Kind::Text {
                slot.text = slot.text.chars().enumerate().filter(|(i, _)| !(ds - a..de - a).contains(i)).map(|(_, c)| c).collect();
            } else {
                slot.text.clear();
            }
        }
        if ins.is_empty() {
            return;
        }
        // Replacing text keeps the formatting of the run it replaced.
        let mut pos = 0;
        let mut target = replaced;
        for (i, slot) in self.slots.iter().enumerate() {
            let len = slot.text.chars().count();
            if target.is_none() && slot.kind == Kind::Text && ((pos < p && p <= pos + len) || (pos >= p && target.is_none())) {
                if pos < p {
                    target = Some((i, p - pos));
                    break;
                }
                target = Some((i, 0));
            }
            pos += len;
        }
        let (i, at) = target.unwrap_or_else(|| {
            let fmt = self.slots.last().map(|s| s.fmt.clone()).unwrap_or_else(|| self.base.clone());
            let rpr_xml = self.slots.last().and_then(|s| s.rpr_xml.clone());
            self.slots.push(Slot { kind: Kind::Text, text: String::new(), fmt, rpr_xml });
            (self.slots.len() - 1, 0)
        });
        let t = &mut self.slots[i].text;
        let byte = t.char_indices().nth(at).map_or(t.len(), |(b, _)| b);
        t.insert_str(byte, &ins);
    }
}

#[derive(Default)]
pub struct Table {
    pub grid: Vec<f32>,
    pub rows: Vec<Vec<Cell>>,
    pub borders: bool,
}

pub struct Cell {
    pub width: f32,
    pub span: usize,
    pub vmerge: bool,
    pub fill: Option<Rgb>,
    pub blocks: Vec<Block>,
}

pub enum Block {
    Para(usize),
    Table(Table),
}

#[derive(Clone, Copy)]
pub struct Page {
    pub w: f32,
    pub h: f32,
    pub l: f32,
    pub r: f32,
    pub t: f32,
    pub b: f32,
}

impl Default for Page {
    fn default() -> Self {
        Page { w: 595.3, h: 841.9, l: 72.0, r: 72.0, t: 72.0, b: 72.0 }
    }
}

pub struct Docx {
    pkg: Package,
    pub paras: Vec<Para>,
    pub blocks: Vec<Block>,
    pub page: Page,
    pub media: HashMap<String, Vec<u8>>,
    pub textures: HashMap<String, Option<TextureHandle>>,
    orig_count: usize,
    normal: (PPr, RPr),
    /// Paragraph (id, char offset) that should take keyboard focus next frame.
    pub focus: Option<(usize, usize)>,
    /// Last measured heights, used to place automatic page breaks.
    pub heights: HashMap<(usize, usize), f32>,
    /// Number of pages laid out in the last frame.
    pub pages: usize,
}

fn insert_after(blocks: &mut Vec<Block>, id: usize, new: usize) -> bool {
    for i in 0..blocks.len() {
        if matches!(&blocks[i], Block::Para(x) if *x == id) {
            blocks.insert(i + 1, Block::Para(new));
            return true;
        }
        if let Block::Table(t) = &mut blocks[i] {
            for cell in t.rows.iter_mut().flatten() {
                if insert_after(&mut cell.blocks, id, new) {
                    return true;
                }
            }
        }
    }
    false
}

/// `Some(Some(prev))`: removed `id` and `prev` is the paragraph before it.
/// `Some(None)`: found, but it is first in its container or follows a table.
fn remove_joining(blocks: &mut Vec<Block>, id: usize) -> Option<Option<usize>> {
    for i in 0..blocks.len() {
        if matches!(&blocks[i], Block::Para(x) if *x == id) {
            return Some(match i.checked_sub(1).map(|j| &blocks[j]) {
                Some(Block::Para(q)) => {
                    let q = *q;
                    blocks.remove(i);
                    Some(q)
                }
                _ => None,
            });
        }
        if let Block::Table(t) = &mut blocks[i] {
            for cell in t.rows.iter_mut().flatten() {
                if let Some(r) = remove_joining(&mut cell.blocks, id) {
                    return Some(r);
                }
            }
        }
    }
    None
}

fn order_of(blocks: &[Block], out: &mut Vec<usize>) {
    for b in blocks {
        match b {
            Block::Para(i) => out.push(*i),
            Block::Table(t) => t.rows.iter().flatten().for_each(|c| order_of(&c.blocks, out)),
        }
    }
}

struct SlotOut {
    kind: Kind,
    text: String,
    rpr: Option<String>,
}

struct NewOut {
    ppr: String,
    slots: Vec<SlotOut>,
}

fn slots_out(p: &Para) -> Vec<SlotOut> {
    p.slots.iter().map(|s| SlotOut { kind: s.kind, text: s.text.clone(), rpr: s.rpr_xml.clone() }).collect()
}

impl Docx {
    /// Paragraph ids in reading order (tables included).
    pub fn order(&self) -> Vec<usize> {
        let mut v = Vec::new();
        order_of(&self.blocks, &mut v);
        v
    }

    /// Enter: splits paragraph `id` at char offset `at`; returns the new id.
    pub fn split_para(&mut self, id: usize, at: usize) -> Option<usize> {
        if !self.order().contains(&id) {
            return None;
        }
        let n = self.paras[id].split_at(at, &self.normal);
        let nid = self.paras.len();
        self.paras.push(n);
        insert_after(&mut self.blocks, id, nid).then_some(nid)
    }

    /// Backspace at the start: appends paragraph `id` to the one before it.
    /// Returns (that paragraph, char offset of the join).
    pub fn merge_up(&mut self, id: usize) -> Option<(usize, usize)> {
        if !self.paras[id].imgs.is_empty() {
            return None;
        }
        let q = remove_joining(&mut self.blocks, id).flatten()?;
        let at = self.paras[q].text.chars().count();
        let moved = std::mem::take(&mut self.paras[id].slots);
        self.paras[id].text.clear();
        self.paras[id].deleted = true;
        let qp = &mut self.paras[q];
        qp.slots.extend(moved);
        qp.text = qp.slots.iter().map(|s| s.text.as_str()).collect();
        Some((q, at))
    }
}

impl Docx {
    pub fn open(bytes: &[u8]) -> Result<Self, String> {
        let pkg = Package::open(bytes)?;
        let xml = pkg.get_str(DOC).ok_or("bukan file .docx yang valid")?;
        let styles = pkg.get_str("word/styles.xml").map(|x| parse_styles(&x)).transpose()?.unwrap_or_default();
        let numbering = pkg.get_str("word/numbering.xml").map(|x| parse_numbering(&x)).transpose()?.unwrap_or_default();
        let (paras, blocks, page) = parse_doc(&xml, &styles, &numbering)?;

        let rels = pkg.get_str("word/_rels/document.xml.rels").map(|x| parse_rels(&x)).transpose()?.unwrap_or_default();
        let mut media = HashMap::new();
        for img in paras.iter().flat_map(|p| &p.imgs) {
            if let Some(t) = rels.get(&img.rid) {
                let path = match t.strip_prefix('/') {
                    Some(abs) => abs.to_string(),
                    None => format!("word/{t}"),
                };
                if let Some(b) = pkg.get(&path) {
                    media.insert(img.rid.clone(), b.to_vec());
                }
            }
        }
        let orig_count = paras.len();
        let (np, nr) = styles.para(&None);
        let mut normal = (styles.def_ppr.clone(), styles.def_rpr.clone());
        normal.0.merge(&np);
        normal.1.merge(&nr);
        Ok(Self { pkg, paras, blocks, page, media, textures: HashMap::new(), orig_count, normal, focus: None, heights: HashMap::new(), pages: 1 })
    }

    pub fn is_dirty(&self) -> bool {
        self.paras[..self.orig_count].iter().any(|p| p.deleted || p.dirty())
            || self.paras[self.orig_count..].iter().any(|p| !p.deleted)
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let deleted: HashSet<usize> = (0..self.orig_count).filter(|&i| self.paras[i].deleted).collect();
        let edits: HashMap<usize, Vec<SlotOut>> = (0..self.orig_count)
            .filter(|i| !deleted.contains(i) && self.paras[*i].dirty())
            .map(|i| (i, slots_out(&self.paras[i])))
            .collect();
        // New paragraphs go right after the last original one that precedes them.
        let mut inserts: HashMap<usize, Vec<NewOut>> = HashMap::new();
        let mut prev = None;
        for id in self.order() {
            if id < self.orig_count {
                prev = Some(id);
            } else if let Some(at) = prev {
                let p = &self.paras[id];
                inserts.entry(at).or_default().push(NewOut { ppr: p.ppr_xml.clone(), slots: slots_out(p) });
            }
        }
        let mut pkg = Package::open(&self.pkg.save()?)?;
        if !(edits.is_empty() && inserts.is_empty() && deleted.is_empty()) {
            let xml = pkg.get_str(DOC).ok_or("document.xml hilang")?;
            pkg.set(DOC, rewrite(&xml, &edits, &inserts, &deleted)?);
        }
        pkg.save()
    }
}

// --------------------------------------------------------------- parsing

struct PFrame {
    ppr_xml: String,
    idx: usize,
    direct: PPr,
    pstyle: Option<String>,
    base: RPr,
}

enum Cont {
    Body(Vec<Block>),
    Table(Table),
    Row(Vec<Cell>),
    Cell(Cell),
}

struct Parser<'a> {
    xml: &'a str,
    ppr_start: usize,
    rpr_start: usize,
    run_rpr_xml: Option<String>,
    st: &'a Styles,
    nu: &'a Numbering,
    paras: Vec<Para>,
    pstack: Vec<PFrame>,
    conts: Vec<Cont>,
    in_ppr: bool,
    in_rpr: bool,
    rpr_in_ppr: bool,
    in_run: u32,
    run_rpr: RPr,
    run_style: Option<String>,
    in_tcpr: bool,
    t_open: bool,
    fallback: u32,
    extent: (f32, f32),
    counters: HashMap<u32, [u32; 9]>,
    page: Page,
}

fn parse_doc(xml: &str, st: &Styles, nu: &Numbering) -> Result<(Vec<Para>, Vec<Block>, Page), String> {
    let mut p = Parser {
        xml,
        ppr_start: 0,
        rpr_start: 0,
        run_rpr_xml: None,
        st,
        nu,
        paras: vec![],
        pstack: vec![],
        conts: vec![Cont::Body(vec![])],
        in_ppr: false,
        in_rpr: false,
        rpr_in_ppr: false,
        in_run: 0,
        run_rpr: RPr::default(),
        run_style: None,
        in_tcpr: false,
        t_open: false,
        fallback: 0,
        extent: (0.0, 0.0),
        counters: HashMap::new(),
        page: Page::default(),
    };
    let mut r = Reader::from_str(xml);
    loop {
        let before = r.buffer_position() as usize;
        let ev = r.read_event().map_err(|e| e.to_string())?;
        let after = r.buffer_position() as usize;
        match ev {
            Event::Start(e) => p.open(&e, false, before),
            Event::Empty(e) => p.open(&e, true, before),
            Event::End(e) => p.close(e.name().as_ref(), after),
            Event::Text(t) => {
                if let Some(f) = p.pstack.last() {
                    if p.in_t() {
                        if let Some(s) = p.paras[f.idx].slots.last_mut() {
                            s.text.push_str(&t.unescape().map_err(|e| e.to_string())?);
                        }
                    }
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }
    let blocks = match p.conts.into_iter().next() {
        Some(Cont::Body(b)) => b,
        _ => vec![],
    };
    for para in &mut p.paras {
        para.text = para.slots.iter().map(|s| s.text.as_str()).collect();
        para.orig = para.slots.iter().map(|s| s.text.clone()).collect();
    }
    Ok((p.paras, blocks, p.page))
}

impl Parser<'_> {
    fn in_t(&self) -> bool {
        self.t_open
    }

    fn add_block(&mut self, b: Block) {
        match self.conts.last_mut() {
            Some(Cont::Body(v)) => v.push(b),
            Some(Cont::Cell(c)) => c.blocks.push(b),
            _ => {}
        }
    }

    fn base_for(&self, pstyle: &Option<String>) -> RPr {
        let mut r = self.st.def_rpr.clone();
        r.merge(&self.st.para(pstyle).1);
        r
    }

    fn slot(&mut self, kind: Kind, text: &str) {
        let Some(f) = self.pstack.last() else { return };
        let mut fmt = f.base.clone();
        if self.in_run > 0 {
            fmt.merge(&self.st.rpr(&self.run_style));
            fmt.merge(&self.run_rpr);
        }
        let rpr_xml = if self.in_run > 0 { self.run_rpr_xml.clone() } else { None };
        self.paras[f.idx].slots.push(Slot { kind, text: text.to_string(), fmt, rpr_xml });
    }

    fn open(&mut self, e: &BytesStart, empty: bool, before: usize) {
        match e.name().as_ref() {
            b"w:p" => {
                self.paras.push(Para::new());
                let base = self.base_for(&None);
                self.pstack.push(PFrame { ppr_xml: String::new(), idx: self.paras.len() - 1, direct: PPr::default(), pstyle: None, base });
                if empty {
                    self.end_p();
                }
            }
            b"w:tbl" => self.conts.push(Cont::Table(Table::default())),
            b"w:tr" => self.conts.push(Cont::Row(vec![])),
            b"w:tc" => self.conts.push(Cont::Cell(Cell { width: 0.0, span: 1, vmerge: false, fill: None, blocks: vec![] })),
            b"w:r" if !empty => {
                self.in_run += 1;
                self.run_rpr = RPr::default();
                self.run_style = None;
                self.run_rpr_xml = None;
            }
            b"w:t" => {
                self.slot(Kind::Text, "");
                self.t_open = !empty;
            }
            b"w:tab" if self.in_run > 0 && !self.in_rpr => self.slot(Kind::Tab, "\t"),
            b"w:br" if self.in_run > 0 => match attr(e, "w:type").as_deref() {
                Some("page") => {
                    if let Some(f) = self.pstack.last() {
                        let para = &mut self.paras[f.idx];
                        para.br_at.get_or_insert(para.slots.len());
                    }
                }
                Some("column") => {}
                _ => self.slot(Kind::Br, "\n"),
            },
            b"w:pPr" if !empty => {
                self.in_ppr = true;
                self.ppr_start = before;
            }
            b"w:rPr" if !empty => {
                self.in_rpr = true;
                self.rpr_start = before;
                self.rpr_in_ppr = self.in_ppr;
            }
            b"w:tcPr" if !empty => self.in_tcpr = true,
            b"mc:Fallback" if !empty => self.fallback += 1,
            _ => self.leaf(e),
        }
    }

    fn close(&mut self, name: &[u8], after: usize) {
        match name {
            b"w:p" => self.end_p(),
            b"w:tbl" => {
                if let Some(Cont::Table(t)) = self.conts.pop() {
                    if self.pstack.is_empty() {
                        self.add_block(Block::Table(t));
                    }
                }
            }
            b"w:tr" => {
                if let (Some(Cont::Row(cells)), Some(Cont::Table(t))) = (self.conts.pop(), self.conts.last_mut()) {
                    t.rows.push(cells);
                }
            }
            b"w:tc" => {
                if let (Some(Cont::Cell(c)), Some(Cont::Row(row))) = (self.conts.pop(), self.conts.last_mut()) {
                    row.push(c);
                }
            }
            b"w:r" => self.in_run = self.in_run.saturating_sub(1),
            b"w:t" => self.t_open = false,
            b"w:pPr" => {
                self.in_ppr = false;
                if let Some(i) = self.pstack.len().checked_sub(1) {
                    let base = self.base_for(&self.pstack[i].pstyle);
                    self.pstack[i].base = base;
                    self.pstack[i].ppr_xml = self.xml[self.ppr_start..after].to_string();
                }
            }
            b"w:rPr" => {
                if self.in_run > 0 && !self.rpr_in_ppr {
                    self.run_rpr_xml = Some(self.xml[self.rpr_start..after].to_string());
                }
                self.in_rpr = false;
            }
            b"w:tcPr" => self.in_tcpr = false,
            b"mc:Fallback" => self.fallback = self.fallback.saturating_sub(1),
            _ => {}
        }
    }

    fn leaf(&mut self, e: &BytesStart) {
        match e.name().as_ref() {
            b"w:pgSz" => {
                self.page.w = twips(e, "w:w").unwrap_or(self.page.w);
                self.page.h = twips(e, "w:h").unwrap_or(self.page.h);
                return;
            }
            b"w:pgMar" => {
                self.page.l = twips(e, "w:left").unwrap_or(self.page.l);
                self.page.r = twips(e, "w:right").unwrap_or(self.page.r);
                self.page.t = twips(e, "w:top").unwrap_or(self.page.t);
                self.page.b = twips(e, "w:bottom").unwrap_or(self.page.b);
                return;
            }
            b"wp:extent" => {
                let emu = |k| attr(e, k).and_then(|v| v.parse::<f32>().ok()).unwrap_or(0.0) / 12700.0;
                self.extent = (emu("cx"), emu("cy"));
                return;
            }
            b"a:blip" => {
                if let (Some(rid), true, Some(f)) = (attr(e, "r:embed"), self.fallback == 0, self.pstack.last()) {
                    let (w, h) = self.extent;
                    self.paras[f.idx].imgs.push(Img { rid, w, h });
                }
                return;
            }
            _ => {}
        }
        if self.in_rpr {
            if !self.rpr_in_ppr && self.in_run > 0 {
                parse_rpr_child(e, &mut self.run_rpr, &mut self.run_style);
            }
            return;
        }
        if self.in_ppr {
            if let Some(f) = self.pstack.last_mut() {
                parse_ppr_child(e, &mut f.direct, &mut f.pstyle);
            }
            return;
        }
        match (e.name().as_ref(), self.conts.last_mut()) {
            (b"w:gridCol", Some(Cont::Table(t))) => t.grid.push(twips(e, "w:w").unwrap_or(0.0)),
            (b"w:tblBorders", Some(Cont::Table(t))) => t.borders = true,
            (b"w:tblStyle", Some(Cont::Table(t))) => {
                if attr(e, "w:val").is_some_and(|v| v.contains("Grid")) {
                    t.borders = true;
                }
            }
            (b"w:tcW", Some(Cont::Cell(c))) => c.width = twips(e, "w:w").unwrap_or(0.0),
            (b"w:gridSpan", Some(Cont::Cell(c))) => c.span = attr(e, "w:val").and_then(|v| v.parse().ok()).unwrap_or(1),
            (b"w:vMerge", Some(Cont::Cell(c))) => c.vmerge = attr(e, "w:val").as_deref() != Some("restart"),
            (b"w:shd", Some(Cont::Cell(c))) if self.in_tcpr => c.fill = attr(e, "w:fill").and_then(|v| hex(&v)),
            _ => {}
        }
    }

    fn end_p(&mut self) {
        let Some(f) = self.pstack.pop() else { return };
        let (sp, _) = self.st.para(&f.pstyle);
        let mut ppr = self.st.def_ppr.clone();
        ppr.merge(&sp);

        let num_id = f.direct.num_id.or(ppr.num_id).filter(|&n| n != 0);
        let ilvl = f.direct.ilvl.or(ppr.ilvl).unwrap_or(0).min(8) as usize;
        let mut label = None;
        if let Some(n) = num_id {
            if let Some(abs) = self.nu.nums.get(&n).copied() {
                if let Some(lv) = self.nu.abstracts.get(&abs).and_then(|l| l.get(ilvl)) {
                    ppr.left = lv.left.or(ppr.left);
                    ppr.first = lv.first.or(ppr.first);
                    let c = self.counters.entry(abs).or_insert([0; 9]);
                    c[ilvl] = if c[ilvl] == 0 { lv.start } else { c[ilvl] + 1 };
                    for d in c.iter_mut().skip(ilvl + 1) {
                        *d = 0;
                    }
                    let counters = *c;
                    label = Some(if lv.fmt == "bullet" {
                        ["•", "◦", "▪"][ilvl.min(2)].to_string()
                    } else if lv.fmt == "none" {
                        String::new()
                    } else {
                        let levels = &self.nu.abstracts[&abs];
                        let mut t = lv.text.clone();
                        for k in 1..=9usize {
                            let fmt = levels.get(k - 1).map_or("decimal", |l| l.fmt.as_str());
                            t = t.replace(&format!("%{k}"), &format_number(counters[k - 1].max(1), fmt));
                        }
                        t
                    });
                }
            }
        }
        ppr.merge(&f.direct);

        let para = &mut self.paras[f.idx];
        para.page_break |= ppr.page_break == Some(true);
        para.ppr = ppr;
        para.base = f.base;
        para.label = label;
        para.style = f.pstyle;
        para.ppr_xml = f.ppr_xml;
        if let Some(at) = para.br_at {
            // A break with nothing after it ends the page; otherwise it starts one.
            if para.slots.iter().all(|s| s.text.is_empty()) || at >= para.slots.len() {
                para.break_after = true;
            } else {
                para.page_break = true;
            }
        }
        if self.pstack.is_empty() {
            self.add_block(Block::Para(f.idx));
        }
    }
}

// -------------------------------------------------------------- rewriting

fn io(r: Result<(), std::io::Error>) -> Result<(), String> {
    r.map_err(|e| e.to_string())
}

type Wr = Writer<Cursor<Vec<u8>>>;

fn raw(w: &mut Wr, s: &str) -> Result<(), String> {
    io(w.get_mut().write_all(s.as_bytes()))
}

fn esc(s: &str) -> std::borrow::Cow<'_, str> {
    quick_xml::escape::escape(s)
}

/// Writes slots as fresh runs, each with the formatting of its original run.
fn write_runs(w: &mut Wr, slots: &[SlotOut]) -> Result<(), String> {
    for s in slots.iter().filter(|s| !s.text.is_empty()) {
        let rpr = s.rpr.as_deref().unwrap_or("");
        match s.kind {
            Kind::Text => raw(w, &format!("<w:r>{rpr}<w:t xml:space=\"preserve\">{}</w:t></w:r>", esc(&s.text)))?,
            Kind::Tab => raw(w, &format!("<w:r>{rpr}<w:tab/></w:r>"))?,
            Kind::Br => raw(w, &format!("<w:r>{rpr}<w:br/></w:r>"))?,
        }
    }
    Ok(())
}

fn write_inserts(w: &mut Wr, inserts: &HashMap<usize, Vec<NewOut>>, idx: usize) -> Result<(), String> {
    for n in inserts.get(&idx).into_iter().flatten() {
        raw(w, "<w:p>")?;
        raw(w, &n.ppr)?;
        write_runs(w, &n.slots)?;
        raw(w, "</w:p>")?;
    }
    Ok(())
}

fn t_start() -> BytesStart<'static> {
    let mut s = BytesStart::new("w:t");
    s.push_attribute(("xml:space", "preserve"));
    s
}

struct Frame {
    idx: usize,
    slot: usize,
}

/// Streams document.xml. For edited paragraphs every `w:t` receives the text
/// of its slot (so each run keeps its formatting) and emptied tabs/breaks are
/// dropped; slots beyond the original ones become new runs at the end.
/// Paragraphs created by Enter are inserted after their source and joined-away
/// paragraphs are removed. Every other element passes through untouched.
fn rewrite(
    xml: &str,
    edits: &HashMap<usize, Vec<SlotOut>>,
    inserts: &HashMap<usize, Vec<NewOut>>,
    deleted: &HashSet<usize>,
) -> Result<Vec<u8>, String> {
    let mut r = Reader::from_str(xml);
    let mut w: Wr = Writer::new(Cursor::new(Vec::new()));
    let mut stack: Vec<Frame> = Vec::new();
    let (mut counter, mut run_depth, mut skipping) = (0usize, 0u32, false);
    loop {
        let ev = r.read_event().map_err(|e| e.to_string())?;
        match &ev {
            Event::Eof => break,
            Event::Start(e) if e.name().as_ref() == b"w:p" => {
                let idx = counter;
                counter += 1;
                if deleted.contains(&idx) {
                    // drop the whole element, still counting nested paragraphs
                    let mut depth = 1;
                    while depth > 0 {
                        match r.read_event().map_err(|e| e.to_string())? {
                            Event::Start(e) if e.name().as_ref() == b"w:p" => {
                                counter += 1;
                                depth += 1;
                            }
                            Event::Empty(e) if e.name().as_ref() == b"w:p" => counter += 1,
                            Event::End(e) if e.name().as_ref() == b"w:p" => depth -= 1,
                            Event::Eof => return Err("XML tidak lengkap".into()),
                            _ => {}
                        }
                    }
                    write_inserts(&mut w, inserts, idx)?;
                    continue;
                }
                stack.push(Frame { idx, slot: 0 });
            }
            Event::Empty(e) if e.name().as_ref() == b"w:p" => {
                let idx = counter;
                counter += 1;
                if deleted.contains(&idx) {
                    continue;
                }
                match edits.get(&idx).filter(|v| v.iter().any(|s| !s.text.is_empty())) {
                    Some(v) => {
                        io(w.write_event(Event::Start(e.clone())))?;
                        write_runs(&mut w, v)?;
                        io(w.write_event(Event::End(BytesEnd::new("w:p"))))?;
                    }
                    None => io(w.write_event(ev.clone()))?,
                }
                write_inserts(&mut w, inserts, idx)?;
                continue;
            }
            Event::Start(e) if e.name().as_ref() == b"w:r" => {
                let _ = e;
                run_depth += 1;
            }
            Event::End(e) if e.name().as_ref() == b"w:r" => run_depth = run_depth.saturating_sub(1),
            Event::Start(e) if e.name().as_ref() == b"w:t" => {
                if let Some(f) = stack.last_mut() {
                    let s = f.slot;
                    f.slot += 1;
                    if let Some(v) = edits.get(&f.idx) {
                        io(w.write_event(Event::Start(t_start())))?;
                        if let Some(sl) = v.get(s) {
                            io(w.write_event(Event::Text(BytesText::new(&sl.text))))?;
                        }
                        skipping = true;
                        continue;
                    }
                }
            }
            Event::Empty(e) if e.name().as_ref() == b"w:t" => {
                if let Some(f) = stack.last_mut() {
                    let s = f.slot;
                    f.slot += 1;
                    if let Some(sl) = edits.get(&f.idx).and_then(|v| v.get(s)).filter(|t| !t.text.is_empty()) {
                        io(w.write_event(Event::Start(t_start())))?;
                        io(w.write_event(Event::Text(BytesText::new(&sl.text))))?;
                        io(w.write_event(Event::End(BytesEnd::new("w:t"))))?;
                        continue;
                    }
                }
            }
            Event::Empty(e) if run_depth > 0 && matches!(e.name().as_ref(), b"w:tab" | b"w:br") => {
                let is_slot = e.name().as_ref() == b"w:tab" || !matches!(attr(e, "w:type").as_deref(), Some("page" | "column"));
                if let (true, Some(f)) = (is_slot, stack.last_mut()) {
                    let s = f.slot;
                    f.slot += 1;
                    if edits.get(&f.idx).is_some_and(|v| v.get(s).is_none_or(|t| t.text.is_empty())) {
                        continue; // user deleted this tab/break
                    }
                }
            }
            Event::Text(_) if skipping => continue,
            Event::End(e) if e.name().as_ref() == b"w:t" => skipping = false,
            Event::End(e) if e.name().as_ref() == b"w:p" => {
                if let Some(f) = stack.pop() {
                    if let Some(v) = edits.get(&f.idx) {
                        write_runs(&mut w, v.get(f.slot..).unwrap_or(&[]))?;
                    }
                    io(w.write_event(ev.clone()))?;
                    write_inserts(&mut w, inserts, f.idx)?;
                    continue;
                }
            }
            _ => {}
        }
        io(w.write_event(ev))?;
    }
    Ok(w.into_inner().into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Vec<u8> {
        std::fs::read(&crate::ooxml::sample("rich.docx")).expect("run the sample generator first")
    }

    #[test]
    fn parses_structure() {
        let d = Docx::open(&sample()).unwrap();
        assert!((d.page.w - 595.3).abs() < 1.0 && (d.page.l - 72.0).abs() < 0.1);
        // title: centered, bold, Times, 26pt
        let t = &d.paras[0];
        assert_eq!(t.text, "Laporan Bulanan");
        assert!(t.ppr.jc == Some(Jc::Center));
        let f = &t.slots[0].fmt;
        assert_eq!((f.bold, f.size, f.font.as_deref()), (Some(true), Some(26.0), Some("Times New Roman")));
        // mixed runs keep their own formatting
        let m = d.paras.iter().find(|p| p.text.starts_with("Halo")).unwrap();
        assert_eq!(m.slots[1].fmt.bold, Some(true));
        assert_eq!(m.slots[3].fmt.italic, Some(true));
        assert_eq!(m.slots[5].fmt.color, Some([0xC0, 0, 0]));
        assert!(m.ppr.jc == Some(Jc::Both));
        // lists
        let labels: Vec<_> = d.paras.iter().filter_map(|p| p.label.clone()).collect();
        assert_eq!(labels, ["•", "•", "1.", "2."]);
        // table + image + tab
        assert!(d.blocks.iter().any(|b| matches!(b, Block::Table(t) if t.rows.len() == 2 && t.rows[0].len() == 3 && t.borders)));
        assert_eq!(d.media.len(), 1);
        assert!(d.paras.iter().any(|p| p.text == "Nama:\tBudi Santoso"));
    }

    #[test]
    fn edit_roundtrip_keeps_formatting() {
        let mut d = Docx::open(&sample()).unwrap();
        assert!(!d.is_dirty());
        // edit inside the bold run, and inside the plain tail of the same paragraph
        let i = d.paras.iter().position(|p| p.text.starts_with("Halo")).unwrap();
        d.paras[i].text = d.paras[i].text.replacen("tebal", "SANGAT tebal & <b>", 1);
        d.paras[i].sync();
        assert_eq!(d.paras[i].slots[1].text, "SANGAT tebal & <b>");
        // paragraph without runs (empty) receives new text
        let e = d.paras.iter().position(|p| p.slots.is_empty() || p.text.is_empty()).unwrap();
        d.paras[e].text = "baru".into();
        d.paras[e].sync();
        // delete the tab
        let j = d.paras.iter().position(|p| p.text.contains('\t')).unwrap();
        d.paras[j].text = d.paras[j].text.replace('\t', "");
        d.paras[j].sync();
        assert!(d.is_dirty());

        let saved = d.to_bytes().unwrap();
        let r = Docx::open(&saved).unwrap();
        assert!(!r.is_dirty());
        assert_eq!(r.paras[i].text, d.paras[i].text);
        assert_eq!(r.paras[i].slots[1].fmt.bold, Some(true), "bold run survived");
        assert_eq!(r.paras[i].slots[5].fmt.color, Some([0xC0, 0, 0]));
        assert_eq!(r.paras[e].text, "baru");
        assert_eq!(r.paras[j].text, "Nama:Budi Santoso");
        assert_eq!(r.media.len(), 1, "image kept");
        assert_eq!(r.paras.len(), d.paras.len());
    }

    #[test]
    fn enter_and_backspace() {
        let mut d = Docx::open(&sample()).unwrap();
        let n0 = d.order().len();
        let texts = |d: &Docx| d.order().iter().map(|&i| d.paras[i].text.clone()).collect::<Vec<_>>();

        // Enter in the middle of the bold word "tebal"
        let i = d.paras.iter().position(|p| p.text.starts_with("Halo")).unwrap();
        let full = d.paras[i].text.clone();
        let at = full.chars().take_while(|_| true).count() - full.split("tebal").nth(1).unwrap().chars().count() - 2;
        let n = d.split_para(i, at).unwrap();
        assert_eq!(d.paras[i].text.clone() + &d.paras[n].text, full);
        assert!(d.paras[n].text.starts_with("al, miring"));
        assert_eq!(d.paras[n].slots[0].fmt.bold, Some(true));
        // Enter at the end of a heading gives a plain paragraph
        let t = 0;
        let end = d.paras[t].text.chars().count();
        let tn = d.split_para(t, end).unwrap();
        assert!(d.paras[tn].style.is_none() && d.paras[tn].text.is_empty());
        // Enter inside a table cell
        let cell = d.paras.iter().position(|p| p.text == "Budi").unwrap();
        let cn = d.split_para(cell, 2).unwrap();
        d.paras[cn].text = "NEW".into();
        d.paras[cn].sync();
        assert_eq!(d.order().len(), n0 + 3);

        let saved = d.to_bytes().unwrap();
        let r = Docx::open(&saved).unwrap();
        assert_eq!(texts(&r), texts(&d));
        let ri = r.paras.iter().position(|p| p.text.starts_with("al, miring")).unwrap();
        assert_eq!(r.paras[ri].slots[0].fmt.bold, Some(true), "bold survived the split");
        assert_eq!(r.paras[ri - 1].slots.iter().rev().find(|s| !s.text.is_empty()).unwrap().fmt.bold, Some(true));
        assert!(r.paras.iter().any(|p| p.text == "di" ) || r.paras.iter().any(|p| p.text == "Bu"));
        assert!(r.paras[1].style.is_none() || r.paras[1].text.is_empty());

        // Backspace joins the second list item into the first (and survives saving)
        let mut d = Docx::open(&sample()).unwrap();
        let b = d.paras.iter().position(|p| p.text == "Poin kedua").unwrap();
        let (q, at) = d.merge_up(b).unwrap();
        assert_eq!((d.paras[q].text.as_str(), at), ("Poin pertamaPoin kedua", 12));
        assert!(d.merge_up(0).is_none(), "first paragraph cannot join upwards");
        let r = Docx::open(&d.to_bytes().unwrap()).unwrap();
        assert_eq!(texts(&r).len(), texts(&Docx::open(&sample()).unwrap()).len() - 1);
        assert!(texts(&r).contains(&"Poin pertamaPoin kedua".to_string()));
        assert!(!r.is_dirty());
        assert_eq!(r.media.len(), 1);
    }
}
