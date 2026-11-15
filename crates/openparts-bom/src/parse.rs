//! BOM input: KiCad's BOM CSV export (Reference/Value/Footprint/
//! Datasheet plus custom fields such as MPN and Manufacturer), the KiCad
//! netlist (`.net`, s-expression), and a generic CSV with MPN and
//! manufacturer columns.
//!
//! The CSV reader takes what spreadsheets and KiCad's BOM scripts write:
//! a UTF-8 byte-order mark, `,` `;` or tab delimiters (detected from the
//! header), quoted fields with `""` escapes and line breaks, CRLF, and
//! metadata lines before the header (`"Source:"`, `"Date:"`...). Columns
//! are found by their header names, in any order and case.

use serde::Serialize;

/// Largest BOM accepted (bytes).
pub const MAX_BYTES: usize = 2 * 1024 * 1024;
/// Most data rows (CSV) or components (netlist) accepted.
pub const MAX_ROWS: usize = 5000;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BomError {
    #[error("the BOM is larger than {MAX_BYTES} bytes")]
    TooLarge,
    #[error("the BOM has more than {MAX_ROWS} rows")]
    TooManyRows,
    #[error("the BOM is empty")]
    Empty,
    #[error("the BOM is not text (UTF-8 or UTF-16 with a byte-order mark)")]
    NotText,
    #[error("no header row with a reference, value, footprint or MPN column was found")]
    NoColumns,
    #[error("malformed netlist: {0}")]
    Netlist(String),
    #[error("malformed CSV: {0}")]
    Csv(String),
}

/// The input format.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BomFormat {
    /// KiCad's BOM export: reference, value and footprint columns.
    KicadCsv,
    /// A KiCad netlist (`(export (version "E") ... (components ...))`).
    KicadNetlist,
    /// Any CSV with an MPN column (and optionally a manufacturer one).
    GenericCsv,
}

/// What a column holds.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ColumnRole {
    Reference,
    Value,
    Footprint,
    Datasheet,
    Quantity,
    Mpn,
    Manufacturer,
    Description,
    Dnp,
}

/// A detected column: its role and the header text it was found under.
#[cfg_attr(feature = "schema", derive(schemars::JsonSchema))]
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct DetectedColumn {
    pub role: ColumnRole,
    pub header: String,
}

/// One BOM line: a group of identical components.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct BomLine {
    /// 1-based row in the file (CSV) or the first component's position
    /// (netlist).
    pub row: usize,
    pub references: Vec<String>,
    pub quantity: u32,
    pub value: String,
    pub footprint: String,
    pub datasheet: String,
    pub mpn: String,
    pub manufacturer: String,
    pub description: String,
    pub dnp: bool,
    /// The row's cells as read (CSV), for the annotated export.
    pub cells: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParsedBom {
    pub format: BomFormat,
    pub columns: Vec<DetectedColumn>,
    /// The header row's cells (CSV).
    pub header: Vec<String>,
    pub lines: Vec<BomLine>,
}

/// Decodes uploaded bytes: UTF-8 (with or without a byte-order mark),
/// or UTF-16 with one.
pub fn decode(bytes: &[u8]) -> Result<String, BomError> {
    if bytes.len() > MAX_BYTES {
        return Err(BomError::TooLarge);
    }
    let utf16 = |be: bool| -> Result<String, BomError> {
        let units: Vec<u16> = bytes[2..]
            .as_chunks::<2>()
            .0
            .iter()
            .map(|c| {
                if be {
                    u16::from_be_bytes([c[0], c[1]])
                } else {
                    u16::from_le_bytes([c[0], c[1]])
                }
            })
            .collect();
        String::from_utf16(&units).map_err(|_| BomError::NotText)
    };
    let text = match bytes {
        [0xFF, 0xFE, ..] => utf16(false)?,
        [0xFE, 0xFF, ..] => utf16(true)?,
        _ => String::from_utf8(bytes.to_vec()).map_err(|_| BomError::NotText)?,
    };
    if text.contains('\0') {
        return Err(BomError::NotText);
    }
    Ok(text.trim_start_matches('\u{FEFF}').to_string())
}

/// Parses a BOM in any of the [`BomFormat`]s (detected).
pub fn parse_bom(text: &str) -> Result<ParsedBom, BomError> {
    if text.len() > MAX_BYTES {
        return Err(BomError::TooLarge);
    }
    let text = text.trim_start_matches('\u{FEFF}');
    if text.trim().is_empty() {
        return Err(BomError::Empty);
    }
    if text.trim_start().starts_with('(') {
        return parse_netlist(text);
    }
    parse_csv_bom(text)
}

// ---------------------------------------------------------------- CSV

/// Splits CSV text into rows of cells with `delim`. Quotes may enclose
/// delimiters, `""` and line breaks.
pub fn csv_rows(text: &str, delim: char) -> Result<Vec<Vec<String>>, BomError> {
    let mut rows = Vec::new();
    let mut row = Vec::new();
    let mut cell = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    let mut cell_started_quoted = false;
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    cell.push('"');
                    chars.next();
                } else {
                    quoted = false;
                }
            } else {
                cell.push(c);
            }
            continue;
        }
        match c {
            '"' if cell.trim().is_empty() && !cell_started_quoted => {
                cell.clear();
                quoted = true;
                cell_started_quoted = true;
            }
            c if c == delim => {
                row.push(std::mem::take(&mut cell));
                cell_started_quoted = false;
            }
            '\r' => {}
            '\n' => {
                row.push(std::mem::take(&mut cell));
                cell_started_quoted = false;
                rows.push(std::mem::take(&mut row));
                if rows.len() > MAX_ROWS + 50 {
                    return Err(BomError::TooManyRows);
                }
            }
            c => cell.push(c),
        }
    }
    if quoted {
        return Err(BomError::Csv("a quoted field is not closed".into()));
    }
    if !cell.is_empty() || !row.is_empty() {
        row.push(cell);
        rows.push(row);
    }
    Ok(rows
        .into_iter()
        .map(|r| r.into_iter().map(|c| c.trim().to_string()).collect())
        .collect())
}

/// A header cell folded for matching: lower case, letters and digits
/// only (`Mfr. Part #` -> `mfrpart`).
fn fold_header(h: &str) -> String {
    h.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// The role of a header cell.
fn header_role(h: &str) -> Option<ColumnRole> {
    use ColumnRole as R;
    let f = fold_header(h);
    let role = match f.as_str() {
        "reference"
        | "references"
        | "ref"
        | "refs"
        | "designator"
        | "designators"
        | "refdes"
        | "partreference"
        | "referencedesignator"
        | "referencedesignators" => R::Reference,
        "value" | "val" | "comment" | "values" => R::Value,
        "footprint" | "pcbfootprint" | "package" | "case" | "casepackage" | "packagecase"
        | "footprints" => R::Footprint,
        "datasheet" | "datasheets" => R::Datasheet,
        "qty" | "quantity" | "qnty" | "count" | "quantityperpcb" | "qtyperboard" => R::Quantity,
        "mpn"
        | "manufacturerpartnumber"
        | "mfrpartnumber"
        | "mfrpart"
        | "mfrpn"
        | "mfgpn"
        | "mfgpartnumber"
        | "manufacturerpn"
        | "mfpn"
        | "partnumber"
        | "pn"
        | "mpn1"
        | "manufacturerpartno"
        | "mfrno"
        | "manfpartnumber"
        | "manufacturerpart"
        | "mfrpartno"
        | "partno" => R::Mpn,
        "manufacturer" | "mfr" | "mfg" | "manufacturername" | "mfrname" | "maker"
        | "manufacturer1" | "brand" | "manf" => R::Manufacturer,
        "description" | "desc" => R::Description,
        "dnp" | "donotpopulate" | "donotplace" | "excludefrombom" | "nopopulate" => R::Dnp,
        _ => return None,
    };
    Some(role)
}

/// Picks the delimiter that splits `line` into the most cells (outside
/// quotes).
fn detect_delimiter(text: &str) -> char {
    let mut best = (',', 0usize);
    for d in [',', ';', '\t', '|'] {
        // The most cells any of the first lines has with this delimiter.
        let n = text
            .lines()
            .take(30)
            .map(|line| {
                let mut quoted = false;
                line.chars()
                    .filter(|&c| {
                        if c == '"' {
                            quoted = !quoted;
                        }
                        !quoted && c == d
                    })
                    .count()
            })
            .max()
            .unwrap_or(0);
        if n > best.1 {
            best = (d, n);
        }
    }
    best.0
}

/// Expands a reference cell: `C1, C2 C3`, `C1-C4`, `R5..R7`.
pub fn expand_references(cell: &str) -> Vec<String> {
    let mut out = Vec::new();
    for tok in cell
        .split([',', ' ', ';', '\t'])
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        let range = tok
            .split_once("..")
            .or_else(|| tok.split_once('-'))
            .and_then(|(a, b)| {
                let split = |s: &str| {
                    let i = s.find(|c: char| c.is_ascii_digit())?;
                    let (p, n) = s.split_at(i);
                    Some((p.to_string(), n.parse::<u32>().ok()?))
                };
                let (pa, na) = split(a)?;
                let (pb, nb) = match split(b) {
                    Some(x) => x,
                    // `C1-4`
                    None => (pa.clone(), b.parse().ok()?),
                };
                let pb = if pb.is_empty() { pa.clone() } else { pb };
                (pa == pb && !pa.is_empty() && na <= nb && nb - na < 1000).then_some((pa, na, nb))
            });
        match range {
            Some((p, a, b)) => out.extend((a..=b).map(|n| format!("{p}{n}"))),
            None => out.push(tok.to_string()),
        }
    }
    out
}

fn truthy(s: &str) -> bool {
    matches!(
        s.trim().to_ascii_lowercase().as_str(),
        "1" | "y" | "yes" | "true" | "x" | "dnp" | "excluded"
    )
}

fn parse_csv_bom(text: &str) -> Result<ParsedBom, BomError> {
    let delim = detect_delimiter(text);
    let rows = csv_rows(text, delim)?;
    // The header: the first of the first rows with a reference/value/
    // footprint/MPN column (KiCad's legacy scripts write metadata first).
    let (h, roles) = rows
        .iter()
        .enumerate()
        .take(40)
        .map(|(i, r)| (i, r.iter().map(|c| header_role(c)).collect::<Vec<_>>()))
        .find(|(_, roles)| {
            let has = |x: ColumnRole| roles.contains(&Some(x));
            has(ColumnRole::Mpn)
                || (has(ColumnRole::Value)
                    && (has(ColumnRole::Reference) || has(ColumnRole::Footprint)))
        })
        .ok_or(BomError::NoColumns)?;
    let header = rows[h].clone();
    // The first column of each role wins.
    let mut columns: Vec<DetectedColumn> = Vec::new();
    let mut col: [Option<usize>; 9] = [None; 9];
    for (i, role) in roles.iter().enumerate() {
        if let Some(r) = role {
            let slot = &mut col[*r as usize];
            if slot.is_none() {
                *slot = Some(i);
                columns.push(DetectedColumn {
                    role: *r,
                    header: header[i].clone(),
                });
            }
        }
    }
    columns.sort_by_key(|c| c.role);
    let has = |r: ColumnRole| col[r as usize].is_some();
    let format =
        if has(ColumnRole::Reference) && has(ColumnRole::Value) && has(ColumnRole::Footprint) {
            BomFormat::KicadCsv
        } else {
            BomFormat::GenericCsv
        };
    let data = &rows[h + 1..];
    let data_rows = data
        .iter()
        .filter(|r| r.iter().any(|c| !c.is_empty()))
        .count();
    if data_rows > MAX_ROWS {
        return Err(BomError::TooManyRows);
    }
    let mut lines = Vec::new();
    for (k, r) in data.iter().enumerate() {
        if r.iter().all(|c| c.is_empty()) {
            continue;
        }
        let get = |role: ColumnRole| {
            col[role as usize]
                .and_then(|i| r.get(i))
                .cloned()
                .unwrap_or_default()
        };
        let references = expand_references(&get(ColumnRole::Reference));
        let qty_cell = get(ColumnRole::Quantity);
        let quantity = qty_cell
            .trim()
            .parse::<f64>()
            .ok()
            .filter(|q| q.is_finite() && *q >= 0.0)
            .map(|q| q.round() as u32)
            .unwrap_or(references.len().max(1) as u32);
        let mut line = BomLine {
            row: h + k + 2,
            quantity,
            value: get(ColumnRole::Value),
            footprint: get(ColumnRole::Footprint),
            datasheet: get(ColumnRole::Datasheet),
            mpn: get(ColumnRole::Mpn),
            manufacturer: get(ColumnRole::Manufacturer),
            description: get(ColumnRole::Description),
            dnp: truthy(&get(ColumnRole::Dnp)),
            references,
            cells: r.clone(),
        };
        // Spreadsheet leftovers: "~" is KiCad's empty field.
        for f in [&mut line.datasheet, &mut line.mpn, &mut line.manufacturer] {
            if f == "~" {
                f.clear();
            }
        }
        // A totals row or a stray note: nothing to identify a part by.
        if line.value.is_empty() && line.mpn.is_empty() && line.footprint.is_empty() {
            continue;
        }
        lines.push(line);
    }
    Ok(ParsedBom {
        format,
        columns,
        header,
        lines,
    })
}

// ------------------------------------------------------------ netlist

/// An s-expression: an atom (symbol or string) or a list.
#[derive(Debug, Clone, PartialEq)]
pub enum Sexpr {
    Atom(String),
    List(Vec<Sexpr>),
}

impl Sexpr {
    fn head(&self) -> Option<&str> {
        match self {
            Sexpr::List(v) => match v.first() {
                Some(Sexpr::Atom(a)) => Some(a),
                _ => None,
            },
            _ => None,
        }
    }

    fn items(&self) -> &[Sexpr] {
        match self {
            Sexpr::List(v) => v,
            Sexpr::Atom(_) => &[],
        }
    }

    /// The child lists named `name`.
    fn children<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a Sexpr> + 'a {
        self.items().iter().filter(move |s| s.head() == Some(name))
    }

    fn child(&self, name: &str) -> Option<&Sexpr> {
        self.items().iter().find(|s| s.head() == Some(name))
    }

    /// The first atom after the head: `(ref "R1")` -> `R1`.
    fn value(&self) -> Option<&str> {
        match self.items().get(1) {
            Some(Sexpr::Atom(a)) => Some(a),
            _ => None,
        }
    }
}

/// Parses one s-expression (KiCad's dialect: `"..."` strings with `\`
/// escapes, bare symbols).
pub fn parse_sexpr(text: &str) -> Result<Sexpr, BomError> {
    let err = |m: &str| BomError::Netlist(m.to_string());
    let mut stack: Vec<Vec<Sexpr>> = Vec::new();
    let mut chars = text.chars().peekable();
    let mut done: Option<Sexpr> = None;
    while let Some(&c) = chars.peek() {
        match c {
            '(' => {
                chars.next();
                if stack.len() > 200 {
                    return Err(err("nested too deeply"));
                }
                stack.push(Vec::new());
            }
            ')' => {
                chars.next();
                let list = stack.pop().ok_or_else(|| err("unbalanced ')'"))?;
                let node = Sexpr::List(list);
                match stack.last_mut() {
                    Some(parent) => parent.push(node),
                    None => {
                        done = Some(node);
                        break;
                    }
                }
            }
            '"' => {
                chars.next();
                let mut s = String::new();
                loop {
                    match chars.next() {
                        Some('\\') => match chars.next() {
                            Some('n') => s.push('\n'),
                            Some('t') => s.push('\t'),
                            Some(other) => s.push(other),
                            None => return Err(err("unterminated string")),
                        },
                        Some('"') => break,
                        Some(other) => s.push(other),
                        None => return Err(err("unterminated string")),
                    }
                }
                stack
                    .last_mut()
                    .ok_or_else(|| err("text outside a list"))?
                    .push(Sexpr::Atom(s));
            }
            c if c.is_whitespace() => {
                chars.next();
            }
            _ => {
                let mut s = String::new();
                while let Some(&c) = chars.peek() {
                    if c.is_whitespace() || c == '(' || c == ')' || c == '"' {
                        break;
                    }
                    s.push(c);
                    chars.next();
                }
                stack
                    .last_mut()
                    .ok_or_else(|| err("text outside a list"))?
                    .push(Sexpr::Atom(s));
            }
        }
    }
    done.ok_or_else(|| err("unbalanced '('"))
}

fn parse_netlist(text: &str) -> Result<ParsedBom, BomError> {
    let root = parse_sexpr(text)?;
    if root.head() != Some("export") {
        return Err(BomError::Netlist(
            "not a KiCad netlist (expected (export ...))".into(),
        ));
    }
    let comps: Vec<&Sexpr> = root
        .child("components")
        .map(|c| c.children("comp").collect())
        .unwrap_or_default();
    if comps.len() > MAX_ROWS {
        return Err(BomError::TooManyRows);
    }
    // Components grouped like a BOM: same value, footprint, MPN and
    // manufacturer on one line.
    let mut lines: Vec<BomLine> = Vec::new();
    let mut roles: Vec<ColumnRole> = Vec::new();
    for (k, comp) in comps.iter().enumerate() {
        let get = |name: &str| {
            comp.child(name)
                .and_then(|c| c.value())
                .unwrap_or("")
                .to_string()
        };
        let mut line = BomLine {
            row: k + 1,
            references: vec![get("ref")],
            quantity: 1,
            value: get("value"),
            footprint: get("footprint"),
            datasheet: get("datasheet"),
            ..Default::default()
        };
        // User fields: `(fields (field (name "MPN") "TLV75533PDBVR"))`.
        let mut fields: Vec<(String, String)> = Vec::new();
        if let Some(f) = comp.child("fields") {
            for field in f.children("field") {
                let name = field
                    .child("name")
                    .and_then(|n| n.value())
                    .unwrap_or("")
                    .to_string();
                let value = field
                    .items()
                    .iter()
                    .skip(1)
                    .find_map(|s| match s {
                        Sexpr::Atom(a) => Some(a.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                fields.push((name, value));
            }
        }
        for p in comp.children("property") {
            let name = p.child("name").and_then(|n| n.value()).unwrap_or("");
            let value = p.child("value").and_then(|n| n.value()).unwrap_or("");
            if fold_header(name) == "dnp" || fold_header(name) == "excludefrombom" {
                line.dnp = true;
            } else {
                fields.push((name.to_string(), value.to_string()));
            }
        }
        for (name, value) in fields {
            let value = if value == "~" { String::new() } else { value };
            match header_role(&name) {
                Some(ColumnRole::Mpn) if line.mpn.is_empty() => {
                    line.mpn = value;
                    roles.push(ColumnRole::Mpn);
                }
                Some(ColumnRole::Manufacturer) if line.manufacturer.is_empty() => {
                    line.manufacturer = value;
                    roles.push(ColumnRole::Manufacturer);
                }
                Some(ColumnRole::Description) if line.description.is_empty() => {
                    line.description = value
                }
                Some(ColumnRole::Dnp) => line.dnp = truthy(&value) || value.is_empty(),
                _ => {}
            }
        }
        if line.datasheet == "~" {
            line.datasheet.clear();
        }
        let same = lines.iter_mut().find(|l| {
            l.value == line.value
                && l.footprint == line.footprint
                && l.mpn == line.mpn
                && l.manufacturer == line.manufacturer
                && l.dnp == line.dnp
        });
        match same {
            Some(l) => {
                l.references.extend(line.references);
                l.quantity += 1;
            }
            None => lines.push(line),
        }
    }
    for l in &mut lines {
        l.references.sort_by_key(|r| natural(r));
    }
    let mut columns = vec![
        DetectedColumn {
            role: ColumnRole::Reference,
            header: "ref".into(),
        },
        DetectedColumn {
            role: ColumnRole::Value,
            header: "value".into(),
        },
        DetectedColumn {
            role: ColumnRole::Footprint,
            header: "footprint".into(),
        },
    ];
    roles.sort();
    roles.dedup();
    for r in roles {
        columns.push(DetectedColumn {
            role: r,
            header: format!("field {r:?}").to_lowercase(),
        });
    }
    Ok(ParsedBom {
        format: BomFormat::KicadNetlist,
        columns,
        header: vec![],
        lines,
    })
}

/// `R10` after `R9`.
fn natural(s: &str) -> (String, u64, String) {
    let i = s.find(|c: char| c.is_ascii_digit()).unwrap_or(s.len());
    let (p, rest) = s.split_at(i);
    let j = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());
    (
        p.to_string(),
        rest[..j].parse().unwrap_or(0),
        rest[j..].to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_quotes_and_line_breaks() {
        let rows = csv_rows("a,\"b,c\",\"say \"\"hi\"\"\"\r\n\"multi\nline\",x,\n", ',').unwrap();
        assert_eq!(rows[0], vec!["a", "b,c", "say \"hi\""]);
        assert_eq!(rows[1], vec!["multi\nline", "x", ""]);
        assert!(csv_rows("\"open", ',').is_err());
    }

    #[test]
    fn references_expand() {
        assert_eq!(expand_references("C1, C2 C3"), vec!["C1", "C2", "C3"]);
        assert_eq!(expand_references("R1-R4"), vec!["R1", "R2", "R3", "R4"]);
        assert_eq!(expand_references("R5..R6"), vec!["R5", "R6"]);
        assert_eq!(expand_references("U1-4"), vec!["U1", "U2", "U3", "U4"]);
        assert_eq!(expand_references("TP-GND"), vec!["TP-GND"]);
    }

    #[test]
    fn header_names() {
        assert_eq!(
            header_role("Manufacturer Part Number"),
            Some(ColumnRole::Mpn)
        );
        assert_eq!(header_role("Mfr. Part #"), Some(ColumnRole::Mpn));
        assert_eq!(header_role("MFR"), Some(ColumnRole::Manufacturer));
        assert_eq!(header_role("Refs"), Some(ColumnRole::Reference));
        assert_eq!(header_role("Qty"), Some(ColumnRole::Quantity));
        assert_eq!(header_role("Exclude from BOM"), Some(ColumnRole::Dnp));
        assert_eq!(header_role("Supplier"), None);
    }

    #[test]
    fn sexpr_strings_and_escapes() {
        let s = parse_sexpr(r#"(a "b \"c\"" (d e))"#).unwrap();
        assert_eq!(s.head(), Some("a"));
        assert_eq!(s.value(), Some("b \"c\""));
        assert!(parse_sexpr("(a (b)").is_err());
        assert!(parse_sexpr("(a \"b)").is_err());
    }

    #[test]
    fn decodes_utf16_and_rejects_binary() {
        let mut b = vec![0xFF, 0xFE];
        for u in "Ref,MPN\nU1,X".encode_utf16() {
            b.extend(u.to_le_bytes());
        }
        assert_eq!(decode(&b).unwrap(), "Ref,MPN\nU1,X");
        assert_eq!(
            decode(&[0x89, b'P', b'N', b'G', 0, 0]),
            Err(BomError::NotText)
        );
        assert_eq!(decode("\u{FEFF}a".as_bytes()).unwrap(), "a");
    }
}
