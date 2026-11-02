// Miyu 补过的一份（施工 V-35，见仓库的 `third_party/README.md`）：字体文件映射进来，不整份拷进内存；缓存只记字体在哪个文件、
// 第几张、多长、什么时候改的，不另存一份字体；扫系统字体库用的整张表找到字体以后就放掉。量出来的宽和原来一样。
use fontdb::{Database, Family, Query, Source, Stretch, Style, Weight};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::UNIX_EPOCH;
use ttf_parser::{Face, GlyphId};

use crate::unicode_width::{Cluster, consume_cluster, is_cjk_wide_char};

static TEXT_MEASURER: Lazy<Mutex<TextMeasurer>> = Lazy::new(|| Mutex::new(TextMeasurer::new()));
const FONT_CACHE_VERSION: &str = "v3-font-location";

pub fn measure_text_width(text: &str, font_size: f32, font_family: &str) -> Option<f32> {
    if text.is_empty() || font_size <= 0.0 {
        return Some(0.0);
    }
    let mut guard = TEXT_MEASURER.lock().ok()?;
    guard.measure(text, font_size, font_family)
}

pub fn average_char_width(font_family: &str, font_size: f32) -> Option<f32> {
    if font_size <= 0.0 {
        return None;
    }
    let sample = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let width = measure_text_width(sample, font_size, font_family)?;
    let count = sample.chars().count().max(1) as f32;
    Some(width / count)
}

struct TextMeasurer {
    cache: HashMap<String, Option<FontFace>>,
}

impl TextMeasurer {
    fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    fn measure(&mut self, text: &str, font_size: f32, font_family: &str) -> Option<f32> {
        let family_key = normalize_family_key(font_family);
        let face = if self.cache.contains_key(&family_key) {
            self.cache
                .get_mut(&family_key)
                .and_then(|face| face.as_mut())
        } else {
            let face = self.load_face(font_family);
            self.cache.insert(family_key.clone(), face);
            self.cache
                .get_mut(&family_key)
                .and_then(|face| face.as_mut())
        }?;
        let normalized = text.replace('\t', "    ");
        face.measure_width(&normalized, font_size)
    }

    fn load_face(&mut self, font_family: &str) -> Option<FontFace> {
        let family_key = normalize_family_key(font_family);
        if let Some(face) = load_located_face(&family_key) {
            return Some(face);
        }
        // 系统字体库只在这里扫、只在找字体的这一会儿留着（原来常驻在 TextMeasurer 里）。
        let mut db = Database::new();
        db.load_system_fonts();
        #[derive(Clone, Copy)]
        enum FamilyToken {
            Generic(fontdb::Family<'static>),
            Name(usize),
        }

        let mut names: Vec<String> = Vec::new();
        let mut order: Vec<FamilyToken> = Vec::new();
        for part in font_family.split(',') {
            let raw = part.trim().trim_matches('"').trim_matches('\'');
            if raw.is_empty() {
                continue;
            }
            let lower = raw.to_ascii_lowercase();
            match lower.as_str() {
                "serif" => order.push(FamilyToken::Generic(Family::Serif)),
                "sans-serif" => order.push(FamilyToken::Generic(Family::SansSerif)),
                "monospace" => order.push(FamilyToken::Generic(Family::Monospace)),
                "cursive" => order.push(FamilyToken::Generic(Family::Cursive)),
                "fantasy" => order.push(FamilyToken::Generic(Family::Fantasy)),
                "system-ui" | "-apple-system" | "ui-sans-serif" => {
                    order.push(FamilyToken::Generic(Family::SansSerif))
                }
                "ui-monospace" => order.push(FamilyToken::Generic(Family::Monospace)),
                _ => {
                    let idx = names.len();
                    names.push(canonical_family_name(&db, raw).unwrap_or_else(|| raw.to_string()));
                    order.push(FamilyToken::Name(idx));
                }
            }
        }
        if order.is_empty() {
            order.push(FamilyToken::Generic(Family::SansSerif));
        }

        let mut families: Vec<Family<'_>> = Vec::with_capacity(order.len());
        for token in order {
            match token {
                FamilyToken::Generic(family) => families.push(family),
                FamilyToken::Name(idx) => families.push(Family::Name(names[idx].as_str())),
            }
        }

        let query = Query {
            families: &families,
            weight: Weight::NORMAL,
            stretch: Stretch::Normal,
            style: Style::Normal,
        };
        let id = db.query(&query)?;
        let (source, index) = db.face_source(id)?;
        let (data, located) = match source {
            Source::Binary(data) => (FontData::Owned(data.as_ref().as_ref().to_vec()), None),
            Source::File(path) | Source::SharedFile(path, _) => {
                (FontData::Mapped(map_file(&path)?), Some(path))
            }
        };
        let face = FontFace::from_data(data, index)?;
        if let Some(path) = located {
            remember_location(&family_key, &path, index);
        }
        Some(face)
    }
}

/// 字体的数据：映射进来的文件（只有用到的那几页进内存，内存紧时随时丢得掉），或者不在文件里、只好拷一份的。
enum FontData {
    Owned(Vec<u8>),
    Mapped(memmap2::Mmap),
}

impl std::ops::Deref for FontData {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self {
            FontData::Owned(bytes) => bytes,
            FontData::Mapped(map) => map,
        }
    }
}

/// 只读映射一个字体文件。
fn map_file(path: &Path) -> Option<memmap2::Mmap> {
    let file = fs::File::open(path).ok()?;
    // 安全：只读映射一个系统的字体文件；字体文件在用着的时候被原地截短才会出问题，和 fontdb 自己映射字体（它的 `memmap`
    // 特性，默认开着）是同一种风险，包管理器换字体是换文件不是原地改。
    unsafe { memmap2::Mmap::map(&file) }.ok()
}

struct FontFace {
    data: FontData,
    index: u32,
    units_per_em: u16,
    ascii_advances: Option<[u16; 128]>,
    glyph_cache: HashMap<char, Option<u16>>,
    advance_cache: HashMap<u16, u16>,
}

impl FontFace {
    fn from_data(data: FontData, index: u32) -> Option<Self> {
        let units_per_em = Face::parse(&data, index).ok()?.units_per_em().max(1);
        Some(Self::new(data, index, units_per_em))
    }

    fn new(data: FontData, index: u32, units_per_em: u16) -> Self {
        let ascii_advances = Face::parse(&data, index).ok().map(|parsed| {
            let mut advances = [0u16; 128];
            for byte in 0u8..=127 {
                let ch = byte as char;
                if let Some(glyph_id) = parsed.glyph_index(ch) {
                    advances[byte as usize] = parsed.glyph_hor_advance(glyph_id).unwrap_or(0);
                }
            }
            advances
        });
        Self {
            data,
            index,
            units_per_em,
            ascii_advances,
            glyph_cache: HashMap::new(),
            advance_cache: HashMap::new(),
        }
    }

    fn measure_width(&mut self, text: &str, font_size: f32) -> Option<f32> {
        let scale = font_size / self.units_per_em as f32;
        let fallback = font_size * 0.56;

        if text.is_ascii()
            && let Some(advances) = &self.ascii_advances
        {
            let mut width = 0.0f32;
            for byte in text.as_bytes() {
                if *byte == b'\n' {
                    continue;
                }
                let advance = advances[*byte as usize];
                if advance == 0 {
                    width += fallback;
                } else {
                    width += advance as f32 * scale;
                }
            }
            return Some(width.max(0.0));
        }

        let face = Face::parse(&self.data, self.index).ok()?;
        let scale = font_size / self.units_per_em as f32;
        let mut width = 0.0f32;
        let chars: Vec<char> = text.chars().collect();
        let mut idx = 0usize;

        while idx < chars.len() {
            let ch = chars[idx];
            if ch == '\n' {
                idx += 1;
                continue;
            }

            // Mirror fallback_text_width grapheme-cluster handling so both
            // measurement paths agree on widths for CJK ideographs, kana,
            // hangul, fullwidth chars, and emoji sequences. The OS will
            // render these via system font fallback (e.g. PingFang on iOS)
            // at roughly 1em even when the loaded face has no glyph for
            // them — using 0.56em here under-measures CJK by ~44%.
            if let Some((kind, new_idx)) = consume_cluster(&chars, idx) {
                width += match kind {
                    Cluster::Wide => font_size,
                    Cluster::ZeroWidth => 0.0,
                };
                idx = new_idx;
                continue;
            }

            let glyph = if let Some(cached) = self.glyph_cache.get(&ch) {
                *cached
            } else {
                let glyph = face.glyph_index(ch).map(|id| id.0);
                self.glyph_cache.insert(ch, glyph);
                glyph
            };

            if let Some(glyph_id) = glyph {
                let advance = if let Some(value) = self.advance_cache.get(&glyph_id) {
                    *value
                } else {
                    let value = face.glyph_hor_advance(GlyphId(glyph_id)).unwrap_or(0);
                    self.advance_cache.insert(glyph_id, value);
                    value
                };
                width += advance as f32 * scale;
            } else if is_cjk_wide_char(ch) {
                width += font_size;
            } else {
                width += fallback;
            }
            idx += 1;
        }

        Some(width.max(0.0))
    }
}

fn canonical_family_name(db: &Database, raw: &str) -> Option<String> {
    db.faces()
        .flat_map(|face| face.families.iter().map(|(family, _)| family))
        .find(|family| family.eq_ignore_ascii_case(raw))
        .cloned()
}

fn normalize_family_key(font_family: &str) -> String {
    let trimmed = font_family.trim();
    let family = if trimmed.is_empty() {
        "sans-serif"
    } else {
        trimmed
    };
    format!("{FONT_CACHE_VERSION}:{family}")
}

/// 记字体位置写在哪：调用方定过的照它（`set_font_cache_dir`）；没定过的照原版的 `$XDG_CACHE_HOME/mmdr/font-cache`。
static FONT_CACHE_DIR: Mutex<FontCacheDir> = Mutex::new(FontCacheDir::Default);

enum FontCacheDir {
    Default,
    At(PathBuf),
    Off,
}

/// 定记字体位置的目录（Miyu 加的）：`Some` 照这个目录，`None` 不记（每个进程照旧扫一次系统字体库）。整个进程共用一个设置。
pub fn set_font_cache_dir(dir: Option<PathBuf>) {
    if let Ok(mut set) = FONT_CACHE_DIR.lock() {
        *set = dir.map_or(FontCacheDir::Off, FontCacheDir::At);
    }
}

/// 这一串字体名记过位置、那个文件也没变（Miyu 加的）：有的说明这台机器上读得到它，不用扫系统字体库就知道。
pub fn has_remembered_font(font_family: &str) -> bool {
    located(&normalize_family_key(font_family)).is_some()
}

fn cache_dir() -> Option<PathBuf> {
    match &*FONT_CACHE_DIR.lock().ok()? {
        FontCacheDir::At(dir) => Some(dir.clone()),
        FontCacheDir::Off => None,
        FontCacheDir::Default => {
            let base = std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from).or_else(|| {
                std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".cache"))
            })?;
            Some(base.join("mmdr").join("font-cache"))
        }
    }
}

/// 这一串字体名记下的位置写在哪：`<名字的哈希>.location`。
fn location_path(family_key: &str) -> Option<PathBuf> {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    family_key.hash(&mut hasher);
    let hash = hasher.finish();
    Some(cache_dir()?.join(format!("{hash:x}.location")))
}

/// 文件的长度和修改时刻（纳秒）：对不上就当记的不算数，重新扫系统字体库。
fn stamp(path: &Path) -> Option<(u64, u128)> {
    let meta = fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_nanos();
    Some((meta.len(), modified))
}

/// 记下这一串字体名找到的是哪个文件的第几张：一行第几张、一行长度、一行修改时刻，剩下的是路径。目录里别的东西不碰（默认的目录
/// 是几个程序共用的，原版存的整份字体分不清是谁的）。写不进的不要紧，下次照旧扫。
fn remember_location(family_key: &str, path: &Path, index: u32) {
    let (Some(target), Some((len, modified)), Some(path_text)) =
        (location_path(family_key), stamp(path), path.to_str())
    else {
        return;
    };
    if let Some(dir) = target.parent() {
        let _ = fs::create_dir_all(dir);
    }
    let _ = fs::write(&target, format!("{index}\n{len}\n{modified}\n{path_text}"));
}

/// 记下的位置：文件和第几张；没记过、读不懂、文件变了（长度、修改时刻对不上）的交回空。
fn located(family_key: &str) -> Option<(PathBuf, u32)> {
    let text = fs::read_to_string(location_path(family_key)?).ok()?;
    let mut lines = text.splitn(4, '\n');
    let index: u32 = lines.next()?.parse().ok()?;
    let len: u64 = lines.next()?.parse().ok()?;
    let modified: u128 = lines.next()?.parse().ok()?;
    let path = PathBuf::from(lines.next()?);
    if stamp(&path)? != (len, modified) {
        return None;
    }
    Some((path, index))
}

/// 照记下的位置直接映射那个字体文件，不扫系统字体库；没有能用的记录、映射不了的交回空。
fn load_located_face(family_key: &str) -> Option<FontFace> {
    let (path, index) = located(family_key)?;
    FontFace::from_data(FontData::Mapped(map_file(&path)?), index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use fontdb::{FaceInfo, ID, Language, Source};
    use std::sync::Arc;

    fn push_family(db: &mut Database, family: &str) {
        db.push_face_info(FaceInfo {
            id: ID::dummy(),
            source: Source::Binary(Arc::new(Vec::<u8>::new())),
            index: 0,
            families: vec![(family.to_string(), Language::English_UnitedStates)],
            post_script_name: family.replace(' ', ""),
            style: Style::Normal,
            weight: Weight::NORMAL,
            stretch: Stretch::Normal,
            monospaced: false,
        });
    }

    #[test]
    fn measure_empty_text_is_zero() {
        assert_eq!(measure_text_width("", 16.0, "sans-serif"), Some(0.0));
    }

    #[test]
    fn canonical_family_name_matches_case_insensitively() {
        let mut db = Database::new();
        push_family(&mut db, "Trebuchet MS");

        assert_eq!(
            canonical_family_name(&db, "trebuchet ms").as_deref(),
            Some("Trebuchet MS")
        );
    }

    #[test]
    fn normalize_family_key_uses_versioned_cache_namespace() {
        assert_eq!(
            normalize_family_key("trebuchet ms"),
            "v3-font-location:trebuchet ms"
        );
    }
}
