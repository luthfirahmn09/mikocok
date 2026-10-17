use crate::ooxml::{attr, attr_local, parse_rels, Package};
use quick_xml::events::{BytesEnd, BytesStart, BytesText, Event};
use quick_xml::{Reader, Writer};
use std::collections::{BTreeMap, HashMap};
use std::io::Cursor;

type Wr = Writer<Cursor<Vec<u8>>>;
type Edits = BTreeMap<u32, BTreeMap<u32, String>>;

pub struct CellData {
    pub text: String,
    pub formula: Option<String>,
}

pub struct Sheet {
    pub name: String,
    path: String,
    pub cells: BTreeMap<(u32, u32), CellData>,
    pub max_row: u32,
    pub max_col: u32,
    pub edits: HashMap<(u32, u32), String>,
}

pub struct Xlsx {
    pkg: Package,
    pub sheets: Vec<Sheet>,
}

pub fn col_name(mut c: u32) -> String {
    let mut s = Vec::new();
    while c > 0 {
        c -= 1;
        s.push(b'A' + (c % 26) as u8);
        c /= 26;
    }
    s.reverse();
    String::from_utf8(s).unwrap()
}

/// "AB12" -> (row 12, col 28), both 1-based.
fn parse_ref(r: &str) -> Option<(u32, u32)> {
    let split = r.find(|c: char| c.is_ascii_digit())?;
    let (letters, digits) = r.split_at(split);
    if letters.is_empty() {
        return None;
    }
    let col = letters
        .bytes()
        .try_fold(0u32, |a, b| a.checked_mul(26)?.checked_add((b.to_ascii_uppercase() - b'A') as u32 + 1))?;
    Some((digits.parse().ok()?, col))
}

impl Xlsx {
    pub fn open(bytes: &[u8]) -> Result<Self, String> {
        let pkg = Package::open(bytes)?;
        let wb = pkg.get_str("xl/workbook.xml").ok_or("bukan file .xlsx yang valid")?;
        let rels = pkg.get_str("xl/_rels/workbook.xml.rels").unwrap_or_default();
        let targets = parse_rels(&rels)?;
        let shared = pkg.get_str("xl/sharedStrings.xml").map(|x| parse_shared(&x)).transpose()?.unwrap_or_default();

        let mut sheets = Vec::new();
        for (name, rid) in parse_sheet_list(&wb)? {
            let Some(t) = targets.get(&rid) else { continue };
            let path = match t.strip_prefix('/') {
                Some(abs) => abs.to_string(),
                None => format!("xl/{t}"),
            };
            let Some(xml) = pkg.get_str(&path) else { continue }; // chartsheets etc.
            let (cells, max_row, max_col) = parse_sheet(&xml, &shared)?;
            sheets.push(Sheet { name, path, cells, max_row, max_col, edits: HashMap::new() });
        }
        if sheets.is_empty() {
            return Err("tidak ada worksheet".into());
        }
        Ok(Self { pkg, sheets })
    }

    pub fn is_dirty(&self) -> bool {
        self.sheets.iter().any(|s| !s.edits.is_empty())
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, String> {
        let mut pkg = Package::open(&self.pkg.save()?)?;
        let mut changed = false;
        for s in self.sheets.iter().filter(|s| !s.edits.is_empty()) {
            let xml = pkg.get_str(&s.path).ok_or("sheet hilang")?;
            let mut by_row: Edits = BTreeMap::new();
            for (&(r, c), t) in &s.edits {
                by_row.entry(r).or_default().insert(c, t.clone());
            }
            pkg.set(&s.path, rewrite_sheet(&xml, by_row)?);
            changed = true;
        }
        if changed {
            drop_calc_chain(&mut pkg);
            force_recalc(&mut pkg);
        }
        pkg.save()
    }
}

/// Removes the element containing `needle` (a self-closing tag) from `xml`.
fn remove_tag(xml: &str, needle: &str) -> Option<String> {
    let i = xml.find(needle)?;
    let start = xml[..i].rfind('<')?;
    let end = i + xml[i..].find("/>")? + 2;
    Some(format!("{}{}", &xml[..start], &xml[end..]))
}

/// A stale calcChain makes Excel report corruption once formulas change.
fn drop_calc_chain(pkg: &mut Package) {
    pkg.remove("xl/calcChain.xml");
    if let Some(x) = pkg.get_str("[Content_Types].xml").and_then(|x| remove_tag(&x, "/xl/calcChain.xml")) {
        pkg.set("[Content_Types].xml", x.into_bytes());
    }
    if let Some(x) = pkg.get_str("xl/_rels/workbook.xml.rels").and_then(|x| remove_tag(&x, "calcChain.xml")) {
        pkg.set("xl/_rels/workbook.xml.rels", x.into_bytes());
    }
}

/// Ask Excel to recompute on open, since cached values of formulas are stale.
fn force_recalc(pkg: &mut Package) {
    let Some(mut x) = pkg.get_str("xl/workbook.xml") else { return };
    if let Some(i) = x.find("<calcPr") {
        let tag_end = x[i..].find('>').map_or(x.len(), |e| i + e);
        if !x[i..tag_end].contains("fullCalcOnLoad") {
            x.insert_str(i + 7, " fullCalcOnLoad=\"1\"");
        }
    } else if let Some(i) = x.find("<extLst").or_else(|| x.find("</workbook>")) {
        x.insert_str(i, "<calcPr fullCalcOnLoad=\"1\"/>");
    }
    pkg.set("xl/workbook.xml", x.into_bytes());
}

fn parse_sheet_list(xml: &str) -> Result<Vec<(String, String)>, String> {
    let mut r = Reader::from_str(xml);
    let mut v = Vec::new();
    loop {
        match r.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == b"sheet" => {
                if let (Some(n), Some(id)) = (attr(&e, "name"), attr_local(&e, "id")) {
                    v.push((n, id));
                }
            }
            Event::Eof => return Ok(v),
            _ => {}
        }
    }
}

fn parse_shared(xml: &str) -> Result<Vec<String>, String> {
    let mut r = Reader::from_str(xml);
    let mut out = Vec::new();
    let (mut cur, mut in_t, mut phonetic) = (String::new(), false, 0u32);
    loop {
        match r.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"si" => cur.clear(),
                b"rPh" => phonetic += 1,
                b"t" => in_t = phonetic == 0,
                _ => {}
            },
            Event::Text(t) if in_t => cur.push_str(&t.unescape().map_err(|e| e.to_string())?),
            Event::End(e) => match e.local_name().as_ref() {
                b"t" => in_t = false,
                b"rPh" => phonetic -= 1,
                b"si" => out.push(std::mem::take(&mut cur)),
                _ => {}
            },
            Event::Eof => return Ok(out),
            _ => {}
        }
    }
}

type Parsed = (BTreeMap<(u32, u32), CellData>, u32, u32);

fn parse_sheet(xml: &str, shared: &[String]) -> Result<Parsed, String> {
    #[derive(PartialEq, Clone, Copy)]
    enum Buf { None, V, F, T }
    let mut r = Reader::from_str(xml);
    let mut cells = BTreeMap::new();
    let (mut max_row, mut max_col) = (0u32, 0u32);
    let (mut row, mut col) = (0u32, 0u32);
    let (mut pos, mut ty) = ((0u32, 0u32), String::new());
    let (mut v, mut f, mut is) = (String::new(), String::new(), String::new());
    let (mut buf, mut in_is) = (Buf::None, false);
    loop {
        match r.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"row" => {
                    row = attr(&e, "r").and_then(|s| s.parse().ok()).unwrap_or(row + 1);
                    col = 0;
                }
                b"c" => {
                    col += 1;
                    pos = attr(&e, "r").and_then(|s| parse_ref(&s)).unwrap_or((row, col));
                    col = pos.1;
                    ty = attr(&e, "t").unwrap_or_default();
                    v.clear();
                    f.clear();
                    is.clear();
                }
                b"v" => buf = Buf::V,
                b"f" => buf = Buf::F,
                b"is" => in_is = true,
                b"t" if in_is => buf = Buf::T,
                _ => {}
            },
            Event::Empty(e) if e.local_name().as_ref() == b"row" => {
                row = attr(&e, "r").and_then(|s| s.parse().ok()).unwrap_or(row + 1);
            }
            Event::Empty(e) if e.local_name().as_ref() == b"c" => {
                col += 1;
                if let Some(p) = attr(&e, "r").and_then(|s| parse_ref(&s)) {
                    col = p.1;
                }
            }
            Event::Text(t) => {
                let s = t.unescape().map_err(|e| e.to_string())?;
                match buf {
                    Buf::V => v.push_str(&s),
                    Buf::F => f.push_str(&s),
                    Buf::T => is.push_str(&s),
                    Buf::None => {}
                }
            }
            Event::End(e) => match e.local_name().as_ref() {
                b"v" | b"f" | b"t" => buf = Buf::None,
                b"is" => in_is = false,
                b"c" => {
                    let text = match ty.as_str() {
                        "s" => v.trim().parse::<usize>().ok().and_then(|i| shared.get(i)).cloned().unwrap_or_default(),
                        "inlineStr" => std::mem::take(&mut is),
                        "b" => if v.trim() == "1" { "TRUE" } else { "FALSE" }.to_string(),
                        _ => std::mem::take(&mut v),
                    };
                    let formula = (!f.is_empty()).then(|| std::mem::take(&mut f));
                    if !text.is_empty() || formula.is_some() {
                        max_row = max_row.max(pos.0);
                        max_col = max_col.max(pos.1);
                        cells.insert(pos, CellData { text, formula });
                    }
                }
                _ => {}
            },
            Event::Eof => return Ok((cells, max_row, max_col)),
            _ => {}
        }
    }
}

fn is_number(t: &str) -> bool {
    let leading_zero = t.len() > 1 && t.starts_with('0') && t.as_bytes()[1].is_ascii_digit();
    t == t.trim()
        && !leading_zero
        && t.len() <= 15
        && t.parse::<f64>().map_or(false, |n| n.is_finite())
}

struct Ctx {
    pfx: String,
}

impl Ctx {
    fn n(&self, local: &str) -> String {
        format!("{}{}", self.pfx, local)
    }
}

fn io(r: Result<(), std::io::Error>) -> Result<(), String> {
    r.map_err(|e| e.to_string())
}

fn write_text_el(w: &mut Wr, name: String, attrs: &[(&str, &str)], text: &str) -> Result<(), String> {
    let mut s = BytesStart::new(name.clone());
    for a in attrs {
        s.push_attribute(*a);
    }
    io(w.write_event(Event::Start(s)))?;
    io(w.write_event(Event::Text(BytesText::new(text))))?;
    io(w.write_event(Event::End(BytesEnd::new(name))))
}

fn write_cell(w: &mut Wr, cx: &Ctx, row: u32, col: u32, style: Option<&str>, text: &str) -> Result<(), String> {
    let cref = format!("{}{}", col_name(col), row);
    let mut c = BytesStart::new(cx.n("c"));
    c.push_attribute(("r", cref.as_str()));
    if let Some(s) = style {
        c.push_attribute(("s", s));
    }
    if text.is_empty() {
        return io(w.write_event(Event::Empty(c)));
    }
    let formula = text.strip_prefix('=').filter(|f| !f.trim().is_empty());
    let numeric = formula.is_none() && is_number(text);
    if formula.is_none() && !numeric {
        c.push_attribute(("t", "inlineStr"));
    }
    io(w.write_event(Event::Start(c)))?;
    if let Some(f) = formula {
        write_text_el(w, cx.n("f"), &[], f.trim())?;
    } else if numeric {
        write_text_el(w, cx.n("v"), &[], text)?;
    } else {
        io(w.write_event(Event::Start(BytesStart::new(cx.n("is")))))?;
        write_text_el(w, cx.n("t"), &[("xml:space", "preserve")], text)?;
        io(w.write_event(Event::End(BytesEnd::new(cx.n("is")))))?;
    }
    io(w.write_event(Event::End(BytesEnd::new(cx.n("c")))))
}

fn write_row(w: &mut Wr, cx: &Ctx, row: u32, cells: &BTreeMap<u32, String>) -> Result<(), String> {
    let mut r = BytesStart::new(cx.n("row"));
    r.push_attribute(("r", row.to_string().as_str()));
    io(w.write_event(Event::Start(r)))?;
    for (&c, t) in cells {
        write_cell(w, cx, row, c, None, t)?;
    }
    io(w.write_event(Event::End(BytesEnd::new(cx.n("row")))))
}

/// Copy of a `<row>` start tag without `spans`, which edits may invalidate.
fn row_without_spans(e: &BytesStart) -> BytesStart<'static> {
    let mut n = BytesStart::new(String::from_utf8_lossy(e.name().as_ref()).into_owned());
    for a in e.attributes().flatten() {
        if a.key.as_ref() != b"spans" {
            let val = a.unescape_value().map(|v| v.into_owned()).unwrap_or_default();
            n.push_attribute((String::from_utf8_lossy(a.key.as_ref()).as_ref(), val.as_str()));
        }
    }
    n
}

/// Streams the sheet XML, replacing/inserting only the edited cells.
fn rewrite_sheet(xml: &str, mut pending: Edits) -> Result<Vec<u8>, String> {
    let mut r = Reader::from_str(xml);
    let mut w: Wr = Writer::new(Cursor::new(Vec::new()));
    let mut cx = Ctx { pfx: String::new() };
    let mut cur_row: Option<(u32, BTreeMap<u32, String>)> = None;
    let mut skip_cell = false;

    fn flush_rows(w: &mut Wr, cx: &Ctx, pending: &mut Edits, before: Option<u32>) -> Result<(), String> {
        while let Some((&row, _)) = pending.iter().next() {
            if before.map_or(false, |b| row >= b) {
                break;
            }
            let cells = pending.remove(&row).unwrap();
            write_row(w, cx, row, &cells)?;
        }
        Ok(())
    }
    fn flush_cells(w: &mut Wr, cx: &Ctx, cur: &mut Option<(u32, BTreeMap<u32, String>)>, before: Option<u32>) -> Result<(), String> {
        if let Some((row, cells)) = cur {
            while let Some((&c, _)) = cells.iter().next() {
                if before.map_or(false, |b| c >= b) {
                    break;
                }
                let t = cells.remove(&c).unwrap();
                write_cell(w, cx, *row, c, None, &t)?;
            }
        }
        Ok(())
    }

    loop {
        let ev = r.read_event().map_err(|e| e.to_string())?;
        match &ev {
            Event::Eof => break,
            _ if skip_cell => {
                if let Event::End(e) = &ev {
                    if e.local_name().as_ref() == b"c" {
                        skip_cell = false;
                    }
                }
                continue;
            }
            Event::Start(e) => match e.local_name().as_ref() {
                b"sheetData" => {
                    let full = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                    cx.pfx = full.strip_suffix("sheetData").unwrap_or("").to_string();
                }
                b"row" => {
                    let n = attr(e, "r").and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
                    flush_rows(&mut w, &cx, &mut pending, Some(n))?;
                    if let Some(cells) = pending.remove(&n) {
                        io(w.write_event(Event::Start(row_without_spans(e))))?;
                        cur_row = Some((n, cells));
                        continue;
                    }
                }
                b"c" => {
                    if let Some(row) = cur_row.as_ref().map(|(r, _)| *r) {
                        if let Some(c) = attr(e, "r").and_then(|s| parse_ref(&s)).map(|p| p.1) {
                            flush_cells(&mut w, &cx, &mut cur_row, Some(c))?;
                            if let Some(t) = cur_row.as_mut().unwrap().1.remove(&c) {
                                write_cell(&mut w, &cx, row, c, attr(e, "s").as_deref(), &t)?;
                                skip_cell = true;
                                continue;
                            }
                        }
                    }
                }
                _ => {}
            },
            Event::Empty(e) => match e.local_name().as_ref() {
                b"sheetData" => {
                    let full = String::from_utf8_lossy(e.name().as_ref()).into_owned();
                    cx.pfx = full.strip_suffix("sheetData").unwrap_or("").to_string();
                    io(w.write_event(Event::Start(e.clone())))?;
                    flush_rows(&mut w, &cx, &mut pending, None)?;
                    io(w.write_event(Event::End(BytesEnd::new(full))))?;
                    continue;
                }
                b"row" => {
                    let n = attr(e, "r").and_then(|s| s.parse::<u32>().ok()).unwrap_or(0);
                    flush_rows(&mut w, &cx, &mut pending, Some(n))?;
                    if let Some(cells) = pending.remove(&n) {
                        io(w.write_event(Event::Start(row_without_spans(e))))?;
                        for (c, t) in &cells {
                            write_cell(&mut w, &cx, n, *c, None, t)?;
                        }
                        io(w.write_event(Event::End(BytesEnd::new(String::from_utf8_lossy(e.name().as_ref()).into_owned()))))?;
                        continue;
                    }
                }
                b"c" => {
                    if let Some((row, _)) = &cur_row {
                        let row = *row;
                        if let Some(c) = attr(e, "r").and_then(|s| parse_ref(&s)).map(|p| p.1) {
                            flush_cells(&mut w, &cx, &mut cur_row, Some(c))?;
                            if let Some(t) = cur_row.as_mut().unwrap().1.remove(&c) {
                                write_cell(&mut w, &cx, row, c, attr(e, "s").as_deref(), &t)?;
                                continue;
                            }
                        }
                    }
                }
                _ => {}
            },
            Event::End(e) => match e.local_name().as_ref() {
                b"row" => {
                    flush_cells(&mut w, &cx, &mut cur_row, None)?;
                    cur_row = None;
                }
                b"sheetData" => flush_rows(&mut w, &cx, &mut pending, None)?,
                _ => {}
            },
            _ => {}
        }
        io(w.write_event(ev))?;
    }
    Ok(w.into_inner().into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edit_roundtrip() {
        let bytes = std::fs::read(&crate::ooxml::sample("t.xlsx")).unwrap();
        let mut x = Xlsx::open(&bytes).unwrap();
        let s = &x.sheets[0];
        assert_eq!(s.cells[&(1, 1)].text, "Nama");
        assert_eq!(s.cells[&(2, 1)].formula.as_deref(), Some("B1*2"));
        assert_eq!(s.cells[&(2, 2)].text, "a & b");
        assert_eq!(s.cells[&(4, 1)].text, "TRUE");

        let e = &mut x.sheets[0].edits;
        e.insert((1, 1), "Ganti <x>".into()); // shared string -> inline
        e.insert((2, 1), "=B1*3".into()); // formula cell keeps style s=3
        e.insert((2, 3), "007".into()); // new cell at end of existing row, text (leading zero)
        e.insert((3, 2), "42".into()); // new row between rows, number
        e.insert((6, 1), "akhir".into()); // new row after last
        assert!(x.is_dirty());

        let saved = x.to_bytes().unwrap();
        let y = Xlsx::open(&saved).unwrap();
        let s = &y.sheets[0];
        assert_eq!(s.cells[&(1, 1)].text, "Ganti <x>");
        assert_eq!(s.cells[&(1, 2)].text, "5");
        assert_eq!(s.cells[&(2, 1)].formula.as_deref(), Some("B1*3"));
        assert_eq!(s.cells[&(2, 2)].text, "a & b");
        assert_eq!(s.cells[&(2, 3)].text, "007");
        assert_eq!(s.cells[&(3, 2)].text, "42");
        assert_eq!(s.cells[&(4, 1)].text, "TRUE");
        assert_eq!(s.cells[&(6, 1)].text, "akhir");

        let pkg = Package::open(&saved).unwrap();
        let xml = pkg.get_str("xl/worksheets/sheet1.xml").unwrap();
        assert!(xml.contains(r#"r="A2" s="3""#), "style kept: {xml}");
        assert!(xml.contains(r#"<c r="B3"><v>42</v></c>"#) || xml.contains(r#"r="B3""#));
        assert!(pkg.get("xl/calcChain.xml").is_none());
        assert!(!pkg.get_str("[Content_Types].xml").unwrap().contains("calcChain"));
        assert!(!pkg.get_str("xl/_rels/workbook.xml.rels").unwrap().contains("calcChain"));
        assert!(pkg.get_str("xl/workbook.xml").unwrap().contains("fullCalcOnLoad"));
        // order inside the sheet must stay ascending: row 3 between 2 and 4
        let (r2, r3, r4) = (xml.find(r#"<row r="2""#).unwrap(), xml.find(r#"<row r="3""#).unwrap(), xml.find(r#"<row r="4""#).unwrap());
        assert!(r2 < r3 && r3 < r4);
    }
}
