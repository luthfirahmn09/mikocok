use eframe::egui::{self, FontData, FontFamily};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, OnceLock};

/// Families (with Bold/Italic/Bold Italic files) borrowed from the system so
/// documents render with the fonts Word would use. Nothing is bundled.
const FAMILIES: &[&str] = &["Arial", "Times New Roman", "Courier New", "Georgia", "Verdana", "Tahoma", "Calibri", "Cambria"];
const VARIANTS: [(&str, bool, bool); 4] = [("", false, false), (" Bold", true, false), (" Italic", false, true), (" Bold Italic", true, true)];

static LOADED: OnceLock<HashSet<String>> = OnceLock::new();

fn key(canon: &str, bold: bool, italic: bool) -> String {
    format!("{canon}|{}{}", bold as u8, italic as u8)
}

fn dirs() -> Vec<PathBuf> {
    let mut d: Vec<PathBuf> = [
        "/System/Library/Fonts/Supplemental",
        "/Library/Fonts",
        "/Applications/Microsoft Word.app/Contents/Resources/DFonts",
    ]
    .iter()
    .map(PathBuf::from)
    .collect();
    if let Some(h) = std::env::var_os("HOME") {
        d.push(PathBuf::from(h).join("Library/Fonts"));
    }
    d
}

pub fn install(ctx: &egui::Context) {
    let mut defs = egui::FontDefinitions::default();
    let fallback = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let dirs = dirs();
    let find = |name: &str| dirs.iter().map(|d| d.join(format!("{name}.ttf"))).find_map(|p| std::fs::read(p).ok());
    let mut loaded = HashSet::new();

    for fam in FAMILIES {
        let canon = fam.to_lowercase();
        let Some(regular) = find(fam) else { continue };
        let regular_name = key(&canon, false, false);
        defs.font_data.insert(regular_name.clone(), Arc::new(FontData::from_owned(regular)));
        for (suffix, b, i) in VARIANTS {
            let k = key(&canon, b, i);
            let data_name = match (b || i).then(|| find(&format!("{fam}{suffix}"))).flatten() {
                Some(bytes) => {
                    defs.font_data.insert(k.clone(), Arc::new(FontData::from_owned(bytes)));
                    k.clone()
                }
                None => regular_name.clone(),
            };
            let mut list = vec![data_name];
            list.extend(fallback.iter().cloned());
            defs.families.insert(FontFamily::Name(k.clone().into()), list);
            loaded.insert(k);
        }
    }
    let _ = LOADED.set(loaded);
    ctx.set_fonts(defs);
}

/// Maps a font name used in a document to the closest family we have.
pub fn family(name: Option<&str>, bold: bool, italic: bool) -> FontFamily {
    let n = name.unwrap_or("").to_lowercase();
    let has = |c: &str| LOADED.get().is_some_and(|l| l.contains(&key(c, false, false)));
    let canon = if n.contains("courier") || n.contains("consolas") || n.contains("mono") || n.contains("menlo") {
        "courier new"
    } else if n.contains("georgia") {
        "georgia"
    } else if n.contains("times") || n.contains("cambria") || n.contains("garamond") || n.contains("palatino") || n.contains("serif") || n.contains("book antiqua") {
        "times new roman"
    } else if n.contains("verdana") {
        "verdana"
    } else if n.contains("tahoma") {
        "tahoma"
    } else if n.contains("arial") || n.contains("helvetica") {
        "arial"
    } else if has("calibri") {
        "calibri"
    } else {
        "arial"
    };
    let k = key(canon, bold, italic);
    if LOADED.get().is_some_and(|l| l.contains(&k)) {
        FontFamily::Name(k.into())
    } else {
        FontFamily::Proportional
    }
}
