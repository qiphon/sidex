//! Character encoding detection, decoding, and encoding.
//!
//! Mirrors the encoding support in Monaco / VS Code, which auto-detects BOM
//! markers and falls back to heuristics for common encodings.

use serde::{Deserialize, Serialize};

/// Supported character encodings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Encoding {
    /// UTF-8 (default).
    Utf8,
    /// UTF-8 with BOM.
    Utf8Bom,
    /// UTF-16 Little Endian.
    Utf16Le,
    /// UTF-16 Big Endian.
    Utf16Be,
    /// ISO 8859-1 / Latin-1.
    Latin1,
    /// ISO 8859-2 / Latin-2 (Central European).
    Iso8859_2,
    /// ISO 8859-15 / Latin-9 (Western European with Euro sign).
    Iso8859_15,
    /// Windows-1250 (Central European).
    Windows1250,
    /// Windows-1251 (Cyrillic).
    Windows1251,
    /// Windows-1252 (Western European).
    Windows1252,
    /// Shift JIS (Japanese).
    ShiftJis,
    /// EUC-JP (Japanese).
    EucJp,
    /// GBK / GB2312 (Chinese).
    Gbk,
    /// GB18030 (Chinese, superset of GBK).
    Gb18030,
    /// Big5 (Traditional Chinese).
    Big5,
    /// EUC-KR (Korean).
    EucKr,
    /// KOI8-R (Russian Cyrillic).
    Koi8R,
    /// Mac Roman.
    MacRoman,
    /// ASCII (7-bit subset of UTF-8).
    Ascii,
}

impl Encoding {
    /// Human-readable label for display in status bars / pickers.
    pub fn label(self) -> &'static str {
        match self {
            Self::Utf8 => "UTF-8",
            Self::Utf8Bom => "UTF-8 with BOM",
            Self::Utf16Le => "UTF-16 LE",
            Self::Utf16Be => "UTF-16 BE",
            Self::Latin1 => "ISO 8859-1",
            Self::Iso8859_2 => "ISO 8859-2",
            Self::Iso8859_15 => "ISO 8859-15",
            Self::Windows1250 => "Windows 1250",
            Self::Windows1251 => "Windows 1251",
            Self::Windows1252 => "Windows 1252",
            Self::ShiftJis => "Shift JIS",
            Self::EucJp => "EUC-JP",
            Self::Gbk => "GBK",
            Self::Gb18030 => "GB18030",
            Self::Big5 => "Big5",
            Self::EucKr => "EUC-KR",
            Self::Koi8R => "KOI8-R",
            Self::MacRoman => "Mac Roman",
            Self::Ascii => "ASCII",
        }
    }
}

impl std::fmt::Display for Encoding {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// Stateless encoding service providing detection, decoding, encoding, and
/// label lookup.
pub struct EncodingService;

impl EncodingService {
    /// Detect the encoding of raw bytes using BOM detection and heuristics.
    pub fn detect_encoding(bytes: &[u8]) -> Encoding {
        detect_encoding(bytes)
    }

    /// Detect encoding from BOM only (returns `None` if no BOM found).
    pub fn detect_bom(bytes: &[u8]) -> Option<Encoding> {
        if bytes.starts_with(UTF8_BOM) {
            Some(Encoding::Utf8Bom)
        } else if bytes.starts_with(UTF16_BE_BOM) {
            Some(Encoding::Utf16Be)
        } else if bytes.starts_with(UTF16_LE_BOM) {
            Some(Encoding::Utf16Le)
        } else {
            None
        }
    }

    /// Decode bytes to a `String` using the specified encoding.
    ///
    /// # Errors
    ///
    /// Returns an error if the bytes are not valid for the encoding.
    pub fn decode(bytes: &[u8], encoding: Encoding) -> Result<String, EncodingError> {
        decode(bytes, encoding)
    }

    /// Encode a string to bytes using the specified encoding.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported encodings or out-of-range chars.
    pub fn encode(text: &str, encoding: Encoding) -> Result<Vec<u8>, EncodingError> {
        encode(text, encoding)
    }

    /// Returns the human-readable label for an encoding.
    pub fn encoding_label(encoding: Encoding) -> &'static str {
        encoding.label()
    }

    /// Looks up an encoding by its label (case-insensitive).
    pub fn encoding_from_label(label: &str) -> Option<Encoding> {
        encoding_from_label(label)
    }

    /// Returns all supported encodings with their labels.
    pub fn all_encodings() -> Vec<(Encoding, &'static str)> {
        ALL_ENCODINGS.to_vec()
    }
}

/// All supported encodings and their display labels.
pub const ALL_ENCODINGS: &[(Encoding, &str)] = &[
    (Encoding::Utf8, "UTF-8"),
    (Encoding::Utf8Bom, "UTF-8 with BOM"),
    (Encoding::Utf16Le, "UTF-16 LE"),
    (Encoding::Utf16Be, "UTF-16 BE"),
    (Encoding::Ascii, "ASCII"),
    (Encoding::Latin1, "ISO 8859-1"),
    (Encoding::Iso8859_2, "ISO 8859-2"),
    (Encoding::Iso8859_15, "ISO 8859-15"),
    (Encoding::Windows1250, "Windows 1250"),
    (Encoding::Windows1251, "Windows 1251"),
    (Encoding::Windows1252, "Windows 1252"),
    (Encoding::ShiftJis, "Shift JIS"),
    (Encoding::EucJp, "EUC-JP"),
    (Encoding::Gbk, "GBK"),
    (Encoding::Gb18030, "GB18030"),
    (Encoding::Big5, "Big5"),
    (Encoding::EucKr, "EUC-KR"),
    (Encoding::Koi8R, "KOI8-R"),
    (Encoding::MacRoman, "Mac Roman"),
];

/// Looks up an encoding by its label (case-insensitive, ignoring spaces
/// and hyphens).
pub fn encoding_from_label(label: &str) -> Option<Encoding> {
    let normalized: String = label
        .to_ascii_lowercase()
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-' && *c != '_')
        .collect();

    match normalized.as_str() {
        "utf8" => Some(Encoding::Utf8),
        "utf8bom" | "utf8withbom" => Some(Encoding::Utf8Bom),
        "utf16le" => Some(Encoding::Utf16Le),
        "utf16be" => Some(Encoding::Utf16Be),
        "ascii" => Some(Encoding::Ascii),
        "latin1" | "iso88591" => Some(Encoding::Latin1),
        "iso88592" | "latin2" => Some(Encoding::Iso8859_2),
        "iso885915" | "latin9" => Some(Encoding::Iso8859_15),
        "windows1250" | "cp1250" => Some(Encoding::Windows1250),
        "windows1251" | "cp1251" => Some(Encoding::Windows1251),
        "windows1252" | "cp1252" => Some(Encoding::Windows1252),
        "shiftjis" | "sjis" => Some(Encoding::ShiftJis),
        "eucjp" => Some(Encoding::EucJp),
        "gbk" | "gb2312" => Some(Encoding::Gbk),
        "gb18030" => Some(Encoding::Gb18030),
        "big5" => Some(Encoding::Big5),
        "euckr" => Some(Encoding::EucKr),
        "koi8r" => Some(Encoding::Koi8R),
        "macroman" | "macintosh" => Some(Encoding::MacRoman),
        _ => None,
    }
}

// ── BOM constants ────────────────────────────────────────────────────

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
const UTF16_LE_BOM: &[u8] = &[0xFF, 0xFE];
const UTF16_BE_BOM: &[u8] = &[0xFE, 0xFF];

// ── Detection ────────────────────────────────────────────────────────

/// Detect the encoding of `bytes` using BOM detection and heuristics.
///
/// Checks for a Byte Order Mark first, then applies statistical
/// heuristics for common encodings. Falls back to `Utf8` when unsure.
pub fn detect_encoding(bytes: &[u8]) -> Encoding {
    if bytes.starts_with(UTF8_BOM) {
        return Encoding::Utf8Bom;
    }
    if bytes.starts_with(UTF16_BE_BOM) {
        return Encoding::Utf16Be;
    }
    if bytes.starts_with(UTF16_LE_BOM) {
        return Encoding::Utf16Le;
    }

    // Check for null bytes which suggest UTF-16 without BOM
    if bytes.len() >= 2 {
        let null_even = bytes.iter().step_by(2).filter(|&&b| b == 0).count();
        let null_odd = bytes.iter().skip(1).step_by(2).filter(|&&b| b == 0).count();
        let total_pairs = bytes.len() / 2;
        if total_pairs > 0 {
            // Many nulls in even positions → likely UTF-16 BE
            if null_even > total_pairs / 3 && null_odd == 0 {
                return Encoding::Utf16Be;
            }
            // Many nulls in odd positions → likely UTF-16 LE
            if null_odd > total_pairs / 3 && null_even == 0 {
                return Encoding::Utf16Le;
            }
        }
    }

    // Pure ASCII check
    if bytes.iter().all(|&b| b < 0x80) {
        return Encoding::Ascii;
    }

    // Valid UTF-8 check
    if std::str::from_utf8(bytes).is_ok() {
        return Encoding::Utf8;
    }

    // Heuristic: Shift JIS detection
    if looks_like_shift_jis(bytes) {
        return Encoding::ShiftJis;
    }

    // Heuristic: GBK detection
    if looks_like_gbk(bytes) {
        return Encoding::Gbk;
    }

    // Fallback: Latin-1 accepts all single-byte values.
    Encoding::Latin1
}

fn looks_like_shift_jis(bytes: &[u8]) -> bool {
    let mut i = 0;
    let mut multi_count = 0;
    let mut invalid = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b < 0x80 {
            i += 1;
        } else if (0x81..=0x9F).contains(&b) || (0xE0..=0xEF).contains(&b) {
            if i + 1 >= bytes.len() {
                invalid += 1;
                break;
            }
            let b2 = bytes[i + 1];
            if (0x40..=0x7E).contains(&b2) || (0x80..=0xFC).contains(&b2) {
                multi_count += 1;
                i += 2;
            } else {
                invalid += 1;
                i += 1;
            }
        } else if (0xA1..=0xDF).contains(&b) {
            // Half-width katakana
            multi_count += 1;
            i += 1;
        } else {
            invalid += 1;
            i += 1;
        }
    }
    multi_count > 0 && invalid <= multi_count / 4
}

fn looks_like_gbk(bytes: &[u8]) -> bool {
    let mut i = 0;
    let mut multi_count = 0;
    let mut invalid = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b < 0x80 {
            i += 1;
        } else if (0x81..=0xFE).contains(&b) {
            if i + 1 >= bytes.len() {
                invalid += 1;
                break;
            }
            let b2 = bytes[i + 1];
            if (0x40..=0xFE).contains(&b2) && b2 != 0x7F {
                multi_count += 1;
                i += 2;
            } else {
                invalid += 1;
                i += 1;
            }
        } else {
            invalid += 1;
            i += 1;
        }
    }
    multi_count > 0 && invalid <= multi_count / 4
}

// ── Decoding ─────────────────────────────────────────────────────────

/// Decode `bytes` using the specified `encoding` into a Rust `String`.
///
/// # Errors
///
/// Returns an error if the bytes are not valid for the given encoding.
pub fn decode(bytes: &[u8], encoding: Encoding) -> Result<String, EncodingError> {
    match encoding {
        Encoding::Utf8 => {
            String::from_utf8(bytes.to_vec()).map_err(|_| EncodingError::InvalidData(encoding))
        }
        Encoding::Utf8Bom => {
            let data = if bytes.starts_with(UTF8_BOM) {
                &bytes[3..]
            } else {
                bytes
            };
            String::from_utf8(data.to_vec()).map_err(|_| EncodingError::InvalidData(encoding))
        }
        Encoding::Utf16Le => decode_utf16(bytes, true),
        Encoding::Utf16Be => decode_utf16(bytes, false),
        Encoding::Latin1 => Ok(bytes.iter().map(|&b| b as char).collect()),
        Encoding::Iso8859_2 => decode_single_byte(bytes, &ISO_8859_2_TABLE),
        Encoding::Iso8859_15 => decode_single_byte(bytes, &ISO_8859_15_TABLE),
        Encoding::Windows1250 => decode_single_byte(bytes, &WINDOWS_1250_TABLE),
        Encoding::Windows1251 => decode_single_byte(bytes, &WINDOWS_1251_TABLE),
        Encoding::Windows1252 => decode_single_byte(bytes, &WINDOWS_1252_TABLE),
        Encoding::MacRoman => decode_single_byte(bytes, &MAC_ROMAN_TABLE),
        Encoding::Koi8R => decode_single_byte(bytes, &KOI8_R_TABLE),
        Encoding::Ascii => {
            if bytes.iter().all(|&b| b < 0x80) {
                Ok(bytes.iter().map(|&b| b as char).collect())
            } else {
                Err(EncodingError::InvalidData(encoding))
            }
        }
        Encoding::ShiftJis
        | Encoding::EucJp
        | Encoding::Gbk
        | Encoding::Gb18030
        | Encoding::Big5
        | Encoding::EucKr => Err(EncodingError::UnsupportedEncoding(encoding)),
    }
}

fn decode_utf16(bytes: &[u8], little_endian: bool) -> Result<String, EncodingError> {
    let encoding = if little_endian {
        Encoding::Utf16Le
    } else {
        Encoding::Utf16Be
    };

    // Strip BOM if present
    let bom = if little_endian {
        UTF16_LE_BOM
    } else {
        UTF16_BE_BOM
    };
    let data = if bytes.starts_with(bom) {
        &bytes[2..]
    } else {
        bytes
    };

    if data.len() % 2 != 0 {
        return Err(EncodingError::InvalidData(encoding));
    }

    let code_units: Vec<u16> = data
        .chunks_exact(2)
        .map(|pair| {
            if little_endian {
                u16::from_le_bytes([pair[0], pair[1]])
            } else {
                u16::from_be_bytes([pair[0], pair[1]])
            }
        })
        .collect();

    String::from_utf16(&code_units).map_err(|_| EncodingError::InvalidData(encoding))
}

// ── Encoding ─────────────────────────────────────────────────────────

/// Encode a `&str` into bytes using the specified `encoding`.
///
/// # Errors
///
/// Returns an error for unsupported encodings (multi-byte CJK) or if
/// the text contains characters outside the encoding's range.
pub fn encode(text: &str, encoding: Encoding) -> Result<Vec<u8>, EncodingError> {
    match encoding {
        Encoding::Utf8 => Ok(text.as_bytes().to_vec()),
        Encoding::Utf8Bom => {
            let mut out = Vec::with_capacity(3 + text.len());
            out.extend_from_slice(UTF8_BOM);
            out.extend_from_slice(text.as_bytes());
            Ok(out)
        }
        Encoding::Utf16Le => Ok(encode_utf16(text, true)),
        Encoding::Utf16Be => Ok(encode_utf16(text, false)),
        Encoding::Latin1 => {
            let mut out = Vec::with_capacity(text.len());
            for c in text.chars() {
                let cp = c as u32;
                if cp > 0xFF {
                    return Err(EncodingError::InvalidData(encoding));
                }
                #[allow(clippy::cast_possible_truncation)]
                out.push(cp as u8);
            }
            Ok(out)
        }
        Encoding::Iso8859_2 => encode_single_byte(text, &ISO_8859_2_TABLE, encoding),
        Encoding::Iso8859_15 => encode_single_byte(text, &ISO_8859_15_TABLE, encoding),
        Encoding::Windows1250 => encode_single_byte(text, &WINDOWS_1250_TABLE, encoding),
        Encoding::Windows1251 => encode_single_byte(text, &WINDOWS_1251_TABLE, encoding),
        Encoding::Windows1252 => encode_single_byte(text, &WINDOWS_1252_TABLE, encoding),
        Encoding::MacRoman => encode_single_byte(text, &MAC_ROMAN_TABLE, encoding),
        Encoding::Koi8R => encode_single_byte(text, &KOI8_R_TABLE, encoding),
        Encoding::Ascii => {
            let mut out = Vec::with_capacity(text.len());
            for c in text.chars() {
                if !c.is_ascii() {
                    return Err(EncodingError::InvalidData(encoding));
                }
                out.push(c as u8);
            }
            Ok(out)
        }
        Encoding::ShiftJis
        | Encoding::EucJp
        | Encoding::Gbk
        | Encoding::Gb18030
        | Encoding::Big5
        | Encoding::EucKr => Err(EncodingError::UnsupportedEncoding(encoding)),
    }
}

fn encode_utf16(text: &str, little_endian: bool) -> Vec<u8> {
    let mut out = Vec::new();
    if little_endian {
        out.extend_from_slice(UTF16_LE_BOM);
    } else {
        out.extend_from_slice(UTF16_BE_BOM);
    }

    for code_unit in text.encode_utf16() {
        let bytes = if little_endian {
            code_unit.to_le_bytes()
        } else {
            code_unit.to_be_bytes()
        };
        out.extend_from_slice(&bytes);
    }
    out
}

// ── Single-byte codec helpers ────────────────────────────────────────

fn decode_single_byte(bytes: &[u8], table: &[char; 128]) -> Result<String, EncodingError> {
    let mut out = String::with_capacity(bytes.len());
    for &b in bytes {
        if b < 0x80 {
            out.push(b as char);
        } else {
            let c = table[(b - 0x80) as usize];
            if c == '\u{FFFD}' {
                return Err(EncodingError::InvalidData(Encoding::Utf8));
            }
            out.push(c);
        }
    }
    Ok(out)
}

fn encode_single_byte(
    text: &str,
    table: &[char; 128],
    encoding: Encoding,
) -> Result<Vec<u8>, EncodingError> {
    let mut out = Vec::with_capacity(text.len());
    for c in text.chars() {
        if (c as u32) < 0x80 {
            out.push(c as u8);
        } else if let Some(pos) = table.iter().position(|&tc| tc == c) {
            #[allow(clippy::cast_possible_truncation)]
            out.push((pos as u8) + 0x80);
        } else {
            return Err(EncodingError::InvalidData(encoding));
        }
    }
    Ok(out)
}

// ── Single-byte encoding tables (high 128 bytes: 0x80..0xFF) ────────

#[rustfmt::skip]
static WINDOWS_1252_TABLE: [char; 128] = [
    '\u{20AC}','\u{FFFD}','\u{201A}','\u{0192}','\u{201E}','\u{2026}','\u{2020}','\u{2021}',
    '\u{02C6}','\u{2030}','\u{0160}','\u{2039}','\u{0152}','\u{FFFD}','\u{017D}','\u{FFFD}',
    '\u{FFFD}','\u{2018}','\u{2019}','\u{201C}','\u{201D}','\u{2022}','\u{2013}','\u{2014}',
    '\u{02DC}','\u{2122}','\u{0161}','\u{203A}','\u{0153}','\u{FFFD}','\u{017E}','\u{0178}',
    '\u{00A0}','\u{00A1}','\u{00A2}','\u{00A3}','\u{00A4}','\u{00A5}','\u{00A6}','\u{00A7}',
    '\u{00A8}','\u{00A9}','\u{00AA}','\u{00AB}','\u{00AC}','\u{00AD}','\u{00AE}','\u{00AF}',
    '\u{00B0}','\u{00B1}','\u{00B2}','\u{00B3}','\u{00B4}','\u{00B5}','\u{00B6}','\u{00B7}',
    '\u{00B8}','\u{00B9}','\u{00BA}','\u{00BB}','\u{00BC}','\u{00BD}','\u{00BE}','\u{00BF}',
    '\u{00C0}','\u{00C1}','\u{00C2}','\u{00C3}','\u{00C4}','\u{00C5}','\u{00C6}','\u{00C7}',
    '\u{00C8}','\u{00C9}','\u{00CA}','\u{00CB}','\u{00CC}','\u{00CD}','\u{00CE}','\u{00CF}',
    '\u{00D0}','\u{00D1}','\u{00D2}','\u{00D3}','\u{00D4}','\u{00D5}','\u{00D6}','\u{00D7}',
    '\u{00D8}','\u{00D9}','\u{00DA}','\u{00DB}','\u{00DC}','\u{00DD}','\u{00DE}','\u{00DF}',
    '\u{00E0}','\u{00E1}','\u{00E2}','\u{00E3}','\u{00E4}','\u{00E5}','\u{00E6}','\u{00E7}',
    '\u{00E8}','\u{00E9}','\u{00EA}','\u{00EB}','\u{00EC}','\u{00ED}','\u{00EE}','\u{00EF}',
    '\u{00F0}','\u{00F1}','\u{00F2}','\u{00F3}','\u{00F4}','\u{00F5}','\u{00F6}','\u{00F7}',
    '\u{00F8}','\u{00F9}','\u{00FA}','\u{00FB}','\u{00FC}','\u{00FD}','\u{00FE}','\u{00FF}',
];

#[rustfmt::skip]
static WINDOWS_1251_TABLE: [char; 128] = [
    '\u{0402}','\u{0403}','\u{201A}','\u{0453}','\u{201E}','\u{2026}','\u{2020}','\u{2021}',
    '\u{20AC}','\u{2030}','\u{0409}','\u{2039}','\u{040A}','\u{040C}','\u{040B}','\u{040F}',
    '\u{0452}','\u{2018}','\u{2019}','\u{201C}','\u{201D}','\u{2022}','\u{2013}','\u{2014}',
    '\u{FFFD}','\u{2122}','\u{0459}','\u{203A}','\u{045A}','\u{045C}','\u{045B}','\u{045F}',
    '\u{00A0}','\u{040E}','\u{045E}','\u{0408}','\u{00A4}','\u{0490}','\u{00A6}','\u{00A7}',
    '\u{0401}','\u{00A9}','\u{0404}','\u{00AB}','\u{00AC}','\u{00AD}','\u{00AE}','\u{0407}',
    '\u{00B0}','\u{00B1}','\u{0406}','\u{0456}','\u{0491}','\u{00B5}','\u{00B6}','\u{00B7}',
    '\u{0451}','\u{2116}','\u{0454}','\u{00BB}','\u{0458}','\u{0405}','\u{0455}','\u{0457}',
    '\u{0410}','\u{0411}','\u{0412}','\u{0413}','\u{0414}','\u{0415}','\u{0416}','\u{0417}',
    '\u{0418}','\u{0419}','\u{041A}','\u{041B}','\u{041C}','\u{041D}','\u{041E}','\u{041F}',
    '\u{0420}','\u{0421}','\u{0422}','\u{0423}','\u{0424}','\u{0425}','\u{0426}','\u{0427}',
    '\u{0428}','\u{0429}','\u{042A}','\u{042B}','\u{042C}','\u{042D}','\u{042E}','\u{042F}',
    '\u{0430}','\u{0431}','\u{0432}','\u{0433}','\u{0434}','\u{0435}','\u{0436}','\u{0437}',
    '\u{0438}','\u{0439}','\u{043A}','\u{043B}','\u{043C}','\u{043D}','\u{043E}','\u{043F}',
    '\u{0440}','\u{0441}','\u{0442}','\u{0443}','\u{0444}','\u{0445}','\u{0446}','\u{0447}',
    '\u{0448}','\u{0449}','\u{044A}','\u{044B}','\u{044C}','\u{044D}','\u{044E}','\u{044F}',
];

#[rustfmt::skip]
static WINDOWS_1250_TABLE: [char; 128] = [
    '\u{20AC}','\u{FFFD}','\u{201A}','\u{FFFD}','\u{201E}','\u{2026}','\u{2020}','\u{2021}',
    '\u{FFFD}','\u{2030}','\u{0160}','\u{2039}','\u{015A}','\u{0164}','\u{017D}','\u{0179}',
    '\u{FFFD}','\u{2018}','\u{2019}','\u{201C}','\u{201D}','\u{2022}','\u{2013}','\u{2014}',
    '\u{FFFD}','\u{2122}','\u{0161}','\u{203A}','\u{015B}','\u{0165}','\u{017E}','\u{017A}',
    '\u{00A0}','\u{02C7}','\u{02D8}','\u{0141}','\u{00A4}','\u{0104}','\u{00A6}','\u{00A7}',
    '\u{00A8}','\u{00A9}','\u{015E}','\u{00AB}','\u{00AC}','\u{00AD}','\u{00AE}','\u{017B}',
    '\u{00B0}','\u{00B1}','\u{02DB}','\u{0142}','\u{00B4}','\u{00B5}','\u{00B6}','\u{00B7}',
    '\u{00B8}','\u{0105}','\u{015F}','\u{00BB}','\u{013D}','\u{02DD}','\u{013E}','\u{017C}',
    '\u{0154}','\u{00C1}','\u{00C2}','\u{0102}','\u{00C4}','\u{0139}','\u{0106}','\u{00C7}',
    '\u{010C}','\u{00C9}','\u{0118}','\u{00CB}','\u{011A}','\u{00CD}','\u{00CE}','\u{010E}',
    '\u{0110}','\u{0143}','\u{0147}','\u{00D3}','\u{00D4}','\u{0150}','\u{00D6}','\u{00D7}',
    '\u{0158}','\u{016E}','\u{00DA}','\u{0170}','\u{00DC}','\u{00DD}','\u{0162}','\u{00DF}',
    '\u{0155}','\u{00E1}','\u{00E2}','\u{0103}','\u{00E4}','\u{013A}','\u{0107}','\u{00E7}',
    '\u{010D}','\u{00E9}','\u{0119}','\u{00EB}','\u{011B}','\u{00ED}','\u{00EE}','\u{010F}',
    '\u{0111}','\u{0144}','\u{0148}','\u{00F3}','\u{00F4}','\u{0151}','\u{00F6}','\u{00F7}',
    '\u{0159}','\u{016F}','\u{00FA}','\u{0171}','\u{00FC}','\u{00FD}','\u{0163}','\u{02D9}',
];

#[rustfmt::skip]
static ISO_8859_2_TABLE: [char; 128] = [
    '\u{0080}','\u{0081}','\u{0082}','\u{0083}','\u{0084}','\u{0085}','\u{0086}','\u{0087}',
    '\u{0088}','\u{0089}','\u{008A}','\u{008B}','\u{008C}','\u{008D}','\u{008E}','\u{008F}',
    '\u{0090}','\u{0091}','\u{0092}','\u{0093}','\u{0094}','\u{0095}','\u{0096}','\u{0097}',
    '\u{0098}','\u{0099}','\u{009A}','\u{009B}','\u{009C}','\u{009D}','\u{009E}','\u{009F}',
    '\u{00A0}','\u{0104}','\u{02D8}','\u{0141}','\u{00A4}','\u{013D}','\u{015A}','\u{00A7}',
    '\u{00A8}','\u{0160}','\u{015E}','\u{0164}','\u{0179}','\u{00AD}','\u{017D}','\u{017B}',
    '\u{00B0}','\u{0105}','\u{02DB}','\u{0142}','\u{00B4}','\u{013E}','\u{015B}','\u{02C7}',
    '\u{00B8}','\u{0161}','\u{015F}','\u{0165}','\u{017A}','\u{02DD}','\u{017E}','\u{017C}',
    '\u{0154}','\u{00C1}','\u{00C2}','\u{0102}','\u{00C4}','\u{0139}','\u{0106}','\u{00C7}',
    '\u{010C}','\u{00C9}','\u{0118}','\u{00CB}','\u{011A}','\u{00CD}','\u{00CE}','\u{010E}',
    '\u{0110}','\u{0143}','\u{0147}','\u{00D3}','\u{00D4}','\u{0150}','\u{00D6}','\u{00D7}',
    '\u{0158}','\u{016E}','\u{00DA}','\u{0170}','\u{00DC}','\u{00DD}','\u{0162}','\u{00DF}',
    '\u{0155}','\u{00E1}','\u{00E2}','\u{0103}','\u{00E4}','\u{013A}','\u{0107}','\u{00E7}',
    '\u{010D}','\u{00E9}','\u{0119}','\u{00EB}','\u{011B}','\u{00ED}','\u{00EE}','\u{010F}',
    '\u{0111}','\u{0144}','\u{0148}','\u{00F3}','\u{00F4}','\u{0151}','\u{00F6}','\u{00F7}',
    '\u{0159}','\u{016F}','\u{00FA}','\u{0171}','\u{00FC}','\u{00FD}','\u{0163}','\u{02D9}',
];

#[rustfmt::skip]
static ISO_8859_15_TABLE: [char; 128] = [
    '\u{0080}','\u{0081}','\u{0082}','\u{0083}','\u{0084}','\u{0085}','\u{0086}','\u{0087}',
    '\u{0088}','\u{0089}','\u{008A}','\u{008B}','\u{008C}','\u{008D}','\u{008E}','\u{008F}',
    '\u{0090}','\u{0091}','\u{0092}','\u{0093}','\u{0094}','\u{0095}','\u{0096}','\u{0097}',
    '\u{0098}','\u{0099}','\u{009A}','\u{009B}','\u{009C}','\u{009D}','\u{009E}','\u{009F}',
    '\u{00A0}','\u{00A1}','\u{00A2}','\u{00A3}','\u{20AC}','\u{00A5}','\u{0160}','\u{00A7}',
    '\u{0161}','\u{00A9}','\u{00AA}','\u{00AB}','\u{00AC}','\u{00AD}','\u{00AE}','\u{00AF}',
    '\u{00B0}','\u{00B1}','\u{00B2}','\u{00B3}','\u{017D}','\u{00B5}','\u{00B6}','\u{00B7}',
    '\u{017E}','\u{00B9}','\u{00BA}','\u{00BB}','\u{0152}','\u{0153}','\u{0178}','\u{00BF}',
    '\u{00C0}','\u{00C1}','\u{00C2}','\u{00C3}','\u{00C4}','\u{00C5}','\u{00C6}','\u{00C7}',
    '\u{00C8}','\u{00C9}','\u{00CA}','\u{00CB}','\u{00CC}','\u{00CD}','\u{00CE}','\u{00CF}',
    '\u{00D0}','\u{00D1}','\u{00D2}','\u{00D3}','\u{00D4}','\u{00D5}','\u{00D6}','\u{00D7}',
    '\u{00D8}','\u{00D9}','\u{00DA}','\u{00DB}','\u{00DC}','\u{00DD}','\u{00DE}','\u{00DF}',
    '\u{00E0}','\u{00E1}','\u{00E2}','\u{00E3}','\u{00E4}','\u{00E5}','\u{00E6}','\u{00E7}',
    '\u{00E8}','\u{00E9}','\u{00EA}','\u{00EB}','\u{00EC}','\u{00ED}','\u{00EE}','\u{00EF}',
    '\u{00F0}','\u{00F1}','\u{00F2}','\u{00F3}','\u{00F4}','\u{00F5}','\u{00F6}','\u{00F7}',
    '\u{00F8}','\u{00F9}','\u{00FA}','\u{00FB}','\u{00FC}','\u{00FD}','\u{00FE}','\u{00FF}',
];

#[rustfmt::skip]
static KOI8_R_TABLE: [char; 128] = [
    '\u{2500}','\u{2502}','\u{250C}','\u{2510}','\u{2514}','\u{2518}','\u{251C}','\u{2524}',
    '\u{252C}','\u{2534}','\u{253C}','\u{2580}','\u{2584}','\u{2588}','\u{258C}','\u{2590}',
    '\u{2591}','\u{2592}','\u{2593}','\u{2320}','\u{25A0}','\u{2219}','\u{221A}','\u{2248}',
    '\u{2264}','\u{2265}','\u{00A0}','\u{2321}','\u{00B0}','\u{00B2}','\u{00B7}','\u{00F7}',
    '\u{2550}','\u{2551}','\u{2552}','\u{0451}','\u{2553}','\u{2554}','\u{2555}','\u{2556}',
    '\u{2557}','\u{2558}','\u{2559}','\u{255A}','\u{255B}','\u{255C}','\u{255D}','\u{255E}',
    '\u{255F}','\u{2560}','\u{2561}','\u{0401}','\u{2562}','\u{2563}','\u{2564}','\u{2565}',
    '\u{2566}','\u{2567}','\u{2568}','\u{2569}','\u{256A}','\u{256B}','\u{256C}','\u{00A9}',
    '\u{044E}','\u{0430}','\u{0431}','\u{0446}','\u{0434}','\u{0435}','\u{0444}','\u{0433}',
    '\u{0445}','\u{0438}','\u{0439}','\u{043A}','\u{043B}','\u{043C}','\u{043D}','\u{043E}',
    '\u{043F}','\u{044F}','\u{0440}','\u{0441}','\u{0442}','\u{0443}','\u{0436}','\u{0432}',
    '\u{044C}','\u{044B}','\u{0437}','\u{0448}','\u{044D}','\u{0449}','\u{0447}','\u{044A}',
    '\u{042E}','\u{0410}','\u{0411}','\u{0426}','\u{0414}','\u{0415}','\u{0424}','\u{0413}',
    '\u{0425}','\u{0418}','\u{0419}','\u{041A}','\u{041B}','\u{041C}','\u{041D}','\u{041E}',
    '\u{041F}','\u{042F}','\u{0420}','\u{0421}','\u{0422}','\u{0423}','\u{0416}','\u{0412}',
    '\u{042C}','\u{042B}','\u{0417}','\u{0428}','\u{042D}','\u{0429}','\u{0427}','\u{042A}',
];

#[rustfmt::skip]
static MAC_ROMAN_TABLE: [char; 128] = [
    '\u{00C4}','\u{00C5}','\u{00C7}','\u{00C9}','\u{00D1}','\u{00D6}','\u{00DC}','\u{00E1}',
    '\u{00E0}','\u{00E2}','\u{00E4}','\u{00E3}','\u{00E5}','\u{00E7}','\u{00E9}','\u{00E8}',
    '\u{00EA}','\u{00EB}','\u{00ED}','\u{00EC}','\u{00EE}','\u{00EF}','\u{00F1}','\u{00F3}',
    '\u{00F2}','\u{00F4}','\u{00F6}','\u{00F5}','\u{00FA}','\u{00F9}','\u{00FB}','\u{00FC}',
    '\u{2020}','\u{00B0}','\u{00A2}','\u{00A3}','\u{00A7}','\u{2022}','\u{00B6}','\u{00DF}',
    '\u{00AE}','\u{00A9}','\u{2122}','\u{00B4}','\u{00A8}','\u{2260}','\u{00C6}','\u{00D8}',
    '\u{221E}','\u{00B1}','\u{2264}','\u{2265}','\u{00A5}','\u{00B5}','\u{2202}','\u{2211}',
    '\u{220F}','\u{03C0}','\u{222B}','\u{00AA}','\u{00BA}','\u{03A9}','\u{00E6}','\u{00F8}',
    '\u{00BF}','\u{00A1}','\u{00AC}','\u{221A}','\u{0192}','\u{2248}','\u{2206}','\u{00AB}',
    '\u{00BB}','\u{2026}','\u{00A0}','\u{00C0}','\u{00C3}','\u{00D5}','\u{0152}','\u{0153}',
    '\u{2013}','\u{2014}','\u{201C}','\u{201D}','\u{2018}','\u{2019}','\u{00F7}','\u{25CA}',
    '\u{00FF}','\u{0178}','\u{2044}','\u{20AC}','\u{2039}','\u{203A}','\u{FB01}','\u{FB02}',
    '\u{2021}','\u{00B7}','\u{201A}','\u{201E}','\u{2030}','\u{00C2}','\u{00CA}','\u{00C1}',
    '\u{00CB}','\u{00C8}','\u{00CD}','\u{00CE}','\u{00CF}','\u{00CC}','\u{00D3}','\u{00D4}',
    '\u{F8FF}','\u{00D2}','\u{00DA}','\u{00DB}','\u{00D9}','\u{0131}','\u{02C6}','\u{02DC}',
    '\u{00AF}','\u{02D8}','\u{02D9}','\u{02DA}','\u{00B8}','\u{02DD}','\u{02DB}','\u{02C7}',
];

// ── Error type ───────────────────────────────────────────────────────

/// Errors that can occur during encoding/decoding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EncodingError {
    /// The data is not valid for the given encoding.
    InvalidData(Encoding),
    /// The encoding is recognized but full encode/decode is not implemented.
    UnsupportedEncoding(Encoding),
}

impl std::fmt::Display for EncodingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidData(enc) => write!(f, "invalid data for encoding {enc}"),
            Self::UnsupportedEncoding(enc) => write!(f, "unsupported encoding: {enc}"),
        }
    }
}

impl std::error::Error for EncodingError {}

#[cfg(test)]
mod tests {
    use super::*;

    // ── detect_encoding ──────────────────────────────────────────────

    #[test]
    fn detect_utf8_bom() {
        let data = [0xEF, 0xBB, 0xBF, b'h', b'i'];
        assert_eq!(detect_encoding(&data), Encoding::Utf8Bom);
    }

    #[test]
    fn detect_utf16_le_bom() {
        let data = [0xFF, 0xFE, b'h', 0x00];
        assert_eq!(detect_encoding(&data), Encoding::Utf16Le);
    }

    #[test]
    fn detect_utf16_be_bom() {
        let data = [0xFE, 0xFF, 0x00, b'h'];
        assert_eq!(detect_encoding(&data), Encoding::Utf16Be);
    }

    #[test]
    fn detect_pure_ascii() {
        let data = b"hello world\n";
        assert_eq!(detect_encoding(data), Encoding::Ascii);
    }

    #[test]
    fn detect_utf8_multibyte() {
        let data = "héllo wörld".as_bytes();
        assert_eq!(detect_encoding(data), Encoding::Utf8);
    }

    #[test]
    fn detect_empty() {
        assert_eq!(detect_encoding(&[]), Encoding::Ascii);
    }

    #[test]
    fn detect_latin1_fallback() {
        // Bytes 0x80-0xFF that are NOT valid UTF-8 sequences
        let data: Vec<u8> = vec![0x80, 0x81, 0x82, 0x83, 0x84];
        let enc = detect_encoding(&data);
        // Should be either Latin1, ShiftJis, or Gbk depending on heuristics
        assert!(
            enc == Encoding::Latin1 || enc == Encoding::ShiftJis || enc == Encoding::Gbk,
            "unexpected encoding: {enc:?}"
        );
    }

    // ── decode ───────────────────────────────────────────────────────

    #[test]
    fn decode_utf8() {
        let data = "hello".as_bytes();
        assert_eq!(decode(data, Encoding::Utf8).unwrap(), "hello");
    }

    #[test]
    fn decode_utf8_bom() {
        let mut data = vec![0xEF, 0xBB, 0xBF];
        data.extend_from_slice(b"hello");
        assert_eq!(decode(&data, Encoding::Utf8Bom).unwrap(), "hello");
    }

    #[test]
    fn decode_utf8_bom_without_bom() {
        assert_eq!(decode(b"hello", Encoding::Utf8Bom).unwrap(), "hello");
    }

    #[test]
    fn decode_latin1() {
        let data = vec![0xE9]; // é in Latin-1
        assert_eq!(decode(&data, Encoding::Latin1).unwrap(), "é");
    }

    #[test]
    fn decode_ascii() {
        assert_eq!(decode(b"hello", Encoding::Ascii).unwrap(), "hello");
    }

    #[test]
    fn decode_ascii_rejects_high_bytes() {
        assert!(decode(&[0x80], Encoding::Ascii).is_err());
    }

    #[test]
    fn decode_utf16_le() {
        let mut data = vec![0xFF, 0xFE]; // BOM
        for unit in "hi".encode_utf16() {
            data.extend_from_slice(&unit.to_le_bytes());
        }
        assert_eq!(decode(&data, Encoding::Utf16Le).unwrap(), "hi");
    }

    #[test]
    fn decode_utf16_be() {
        let mut data = vec![0xFE, 0xFF]; // BOM
        for unit in "hi".encode_utf16() {
            data.extend_from_slice(&unit.to_be_bytes());
        }
        assert_eq!(decode(&data, Encoding::Utf16Be).unwrap(), "hi");
    }

    #[test]
    fn decode_utf16_odd_bytes() {
        assert!(decode(&[0xFF, 0xFE, 0x00], Encoding::Utf16Le).is_err());
    }

    // ── encode ───────────────────────────────────────────────────────

    #[test]
    fn encode_utf8() {
        assert_eq!(encode("hello", Encoding::Utf8).unwrap(), b"hello");
    }

    #[test]
    fn encode_utf8_bom() {
        let result = encode("hi", Encoding::Utf8Bom).unwrap();
        assert!(result.starts_with(&[0xEF, 0xBB, 0xBF]));
        assert_eq!(&result[3..], b"hi");
    }

    #[test]
    fn encode_latin1() {
        assert_eq!(encode("é", Encoding::Latin1).unwrap(), vec![0xE9]);
    }

    #[test]
    fn encode_latin1_rejects_out_of_range() {
        assert!(encode("你", Encoding::Latin1).is_err());
    }

    #[test]
    fn encode_ascii() {
        assert_eq!(encode("hi", Encoding::Ascii).unwrap(), b"hi");
    }

    #[test]
    fn encode_ascii_rejects_non_ascii() {
        assert!(encode("é", Encoding::Ascii).is_err());
    }

    #[test]
    fn encode_utf16_le_roundtrip() {
        let encoded = encode("hello", Encoding::Utf16Le).unwrap();
        let decoded = decode(&encoded, Encoding::Utf16Le).unwrap();
        assert_eq!(decoded, "hello");
    }

    #[test]
    fn encode_utf16_be_roundtrip() {
        let encoded = encode("hello", Encoding::Utf16Be).unwrap();
        let decoded = decode(&encoded, Encoding::Utf16Be).unwrap();
        assert_eq!(decoded, "hello");
    }

    #[test]
    fn encode_utf16_emoji_roundtrip() {
        let text = "hello 😀 world";
        let encoded = encode(text, Encoding::Utf16Le).unwrap();
        let decoded = decode(&encoded, Encoding::Utf16Le).unwrap();
        assert_eq!(decoded, text);
    }

    // ── Encoding label ───────────────────────────────────────────────

    #[test]
    fn encoding_labels() {
        assert_eq!(Encoding::Utf8.label(), "UTF-8");
        assert_eq!(Encoding::Utf16Le.label(), "UTF-16 LE");
        assert_eq!(Encoding::Latin1.label(), "ISO 8859-1");
    }

    #[test]
    fn encoding_display() {
        assert_eq!(format!("{}", Encoding::Utf8), "UTF-8");
    }

    // ── Error display ────────────────────────────────────────────────

    #[test]
    fn error_display() {
        let err = EncodingError::InvalidData(Encoding::Utf8);
        assert!(format!("{err}").contains("invalid data"));
    }

    #[test]
    fn error_unsupported() {
        let err = EncodingError::UnsupportedEncoding(Encoding::ShiftJis);
        assert!(format!("{err}").contains("unsupported"));
    }

    // ── encoding_from_label ─────────────────────────────────────────

    #[test]
    fn from_label_utf8() {
        assert_eq!(encoding_from_label("UTF-8"), Some(Encoding::Utf8));
        assert_eq!(encoding_from_label("utf8"), Some(Encoding::Utf8));
    }

    #[test]
    fn from_label_windows1252() {
        assert_eq!(encoding_from_label("Windows-1252"), Some(Encoding::Windows1252));
        assert_eq!(encoding_from_label("cp1252"), Some(Encoding::Windows1252));
    }

    #[test]
    fn from_label_koi8r() {
        assert_eq!(encoding_from_label("KOI8-R"), Some(Encoding::Koi8R));
    }

    #[test]
    fn from_label_unknown() {
        assert_eq!(encoding_from_label("nonexistent"), None);
    }

    // ── all_encodings ───────────────────────────────────────────────

    #[test]
    fn all_encodings_has_at_least_19() {
        assert!(ALL_ENCODINGS.len() >= 19);
    }

    // ── EncodingService ─────────────────────────────────────────────

    #[test]
    fn service_detect_bom_none() {
        assert_eq!(EncodingService::detect_bom(b"hello"), None);
    }

    #[test]
    fn service_detect_bom_utf8() {
        assert_eq!(
            EncodingService::detect_bom(&[0xEF, 0xBB, 0xBF, b'h']),
            Some(Encoding::Utf8Bom)
        );
    }

    // ── Windows-1252 roundtrip ──────────────────────────────────────

    #[test]
    fn windows1252_euro_sign() {
        let encoded = encode("\u{20AC}", Encoding::Windows1252).unwrap();
        assert_eq!(encoded, vec![0x80]);
        let decoded = decode(&[0x80], Encoding::Windows1252).unwrap();
        assert_eq!(decoded, "\u{20AC}");
    }

    // ── Windows-1251 Cyrillic roundtrip ────────────────────────────

    #[test]
    fn windows1251_cyrillic_a() {
        let decoded = decode(&[0xC0], Encoding::Windows1251).unwrap();
        assert_eq!(decoded, "\u{0410}"); // А
        let encoded = encode("\u{0410}", Encoding::Windows1251).unwrap();
        assert_eq!(encoded, vec![0xC0]);
    }

    // ── KOI8-R roundtrip ──────────────────────────────────────────

    #[test]
    fn koi8r_roundtrip() {
        let decoded = decode(&[0xC1], Encoding::Koi8R).unwrap();
        let encoded = encode(&decoded, Encoding::Koi8R).unwrap();
        assert_eq!(encoded, vec![0xC1]);
    }

    // ── Encoding has 20+ variants ───────────────────────────────────

    #[test]
    fn encoding_variant_count() {
        let labels: Vec<_> = EncodingService::all_encodings()
            .iter()
            .map(|(_, l)| *l)
            .collect();
        assert!(labels.len() >= 19, "need at least 19 encodings, got {}", labels.len());
    }
}
