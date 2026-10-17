use quick_xml::events::{BytesStart, Event};
use quick_xml::Reader;
use std::collections::HashMap;
use std::io::{Cursor, Read, Write};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

const MAX_TOTAL: u64 = 1 << 30;

/// In-memory copy of an OOXML zip package. Only entries we touch are changed.
pub struct Package {
    entries: Vec<(String, Vec<u8>)>,
}

impl Package {
    pub fn open(bytes: &[u8]) -> Result<Self, String> {
        let mut zip = ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
        let mut entries = Vec::new();
        let mut total = 0u64;
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).map_err(|e| e.to_string())?;
            if f.is_dir() {
                continue;
            }
            total += f.size();
            if total > MAX_TOTAL {
                return Err("file terlalu besar".into());
            }
            let mut buf = Vec::with_capacity(f.size() as usize);
            f.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            let name = f.name().map_err(|e| e.to_string())?.into_owned();
            entries.push((name, buf));
        }
        Ok(Self { entries })
    }

    pub fn get(&self, name: &str) -> Option<&[u8]> {
        self.entries.iter().find(|(n, _)| n == name).map(|(_, d)| d.as_slice())
    }

    pub fn get_str(&self, name: &str) -> Option<String> {
        self.entries
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, d)| String::from_utf8_lossy(d).into_owned())
    }

    pub fn set(&mut self, name: &str, data: Vec<u8>) {
        match self.entries.iter_mut().find(|(n, _)| n == name) {
            Some(e) => e.1 = data,
            None => self.entries.push((name.to_string(), data)),
        }
    }

    pub fn remove(&mut self, name: &str) {
        self.entries.retain(|(n, _)| n != name);
    }

    pub fn save(&self) -> Result<Vec<u8>, String> {
        let mut w = ZipWriter::new(Cursor::new(Vec::new()));
        let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, data) in &self.entries {
            w.start_file(name.as_str(), opts).map_err(|e| e.to_string())?;
            w.write_all(data).map_err(|e| e.to_string())?;
        }
        Ok(w.finish().map_err(|e| e.to_string())?.into_inner())
    }
}

/// Attribute value by exact (prefixed) key, e.g. `w:val`.
pub fn attr(e: &BytesStart, key: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.as_ref() == key.as_bytes())
        .and_then(|a| a.unescape_value().ok().map(|v| v.into_owned()))
}

/// Attribute value by local name, ignoring any namespace prefix.
pub fn attr_local(e: &BytesStart, local: &str) -> Option<String> {
    e.attributes()
        .flatten()
        .find(|a| a.key.local_name().as_ref() == local.as_bytes())
        .and_then(|a| a.unescape_value().ok().map(|v| v.into_owned()))
}

pub fn parse_rels(xml: &str) -> Result<HashMap<String, String>, String> {
    let mut r = Reader::from_str(xml);
    let mut m = HashMap::new();
    loop {
        match r.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) | Event::Empty(e) if e.local_name().as_ref() == b"Relationship" => {
                if let (Some(id), Some(t)) = (attr(&e, "Id"), attr(&e, "Target")) {
                    m.insert(id, t);
                }
            }
            Event::Eof => return Ok(m),
            _ => {}
        }
    }
}


/// Path of a fixture written by `tools/make_samples.py`.
#[cfg(test)]
pub fn sample(name: &str) -> String {
    std::env::temp_dir().join("mikocok-samples").join(name).to_string_lossy().into_owned()
}
