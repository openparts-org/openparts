//! Installing OpenParts parts into a KiCad project's own libraries
//! (`openparts.kicad_sym`, `openparts.pretty/`, `openparts_3d/` and the
//! project's `sym-lib-table`/`fp-lib-table`), shared by `openparts kicad
//! install` and the KiCad plugin.
//!
//! Pure file-manipulation logic: no `eframe`/`egui`, no KiCad API --
//! everything here operates on plain strings and is fully unit-testable
//! in this environment. `sym-lib-table`/`fp-lib-table` and
//! `.kicad_sym`/`.kicad_mod` are themselves simple S-expression text
//! files (same format `openparts-kicad` already emits), so none of this
//! needs a KiCad install or library.
//!
//! The block-extraction/insertion here is deliberately a simple
//! balanced-paren scanner, not a general S-expression parser -- the
//! same "our own generator's output is well-formed, so scanning
//! suffices" approach `openparts-kicad`'s read-back parsers already use,
//! since every input this module reads was itself produced by
//! `openparts-kicad`'s renderer.

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("could not find a balanced `{0}` block in the input")]
    BlockNotFound(String),
}

/// Finds the byte range `[start, end)` of the first balanced
/// parenthesized block whose text begins with `marker` (which must
/// itself start with `(`).
fn find_balanced_block(text: &str, marker: &str) -> Option<(usize, usize)> {
    let start = text.find(marker)?;
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut i = start;
    while i < bytes.len() {
        match bytes[i] {
            b'(' => depth += 1,
            b')' => {
                depth -= 1;
                if depth == 0 {
                    return Some((start, i + 1));
                }
            }
            _ => {}
        }
        i += 1;
    }
    None
}

/// Extracts just the `(symbol "MPN" ...)` block out of a full
/// single-symbol `.kicad_sym` library file as returned by
/// `openparts-server`'s kicad-symbol artifact.
pub fn extract_symbol_block(fetched_symbol_lib_text: &str, mpn: &str) -> Result<String, SyncError> {
    let marker = format!("(symbol \"{mpn}\"");
    let (start, end) = find_balanced_block(fetched_symbol_lib_text, &marker)
        .ok_or_else(|| SyncError::BlockNotFound(marker.clone()))?;
    Ok(fetched_symbol_lib_text[start..end].to_string())
}

/// Merges `symbol_block` (as returned by [`extract_symbol_block`]) into
/// `existing` (the project's own combined `openparts.kicad_sym`, or
/// `None` if it doesn't exist yet). If a symbol with the same name is
/// already present, it's replaced in place (idempotent: installing the
/// same part twice doesn't duplicate it).
pub fn merge_symbol_library(existing: Option<&str>, mpn: &str, symbol_block: &str) -> String {
    let marker = format!("(symbol \"{mpn}\"");
    match existing {
        Some(text) => {
            if let Some((start, end)) = find_balanced_block(text, &marker) {
                format!("{}{}{}", &text[..start], symbol_block, &text[end..])
            } else {
                // Insert just before the final closing paren of the
                // (kicad_symbol_lib ...) wrapper.
                let trimmed = text.trim_end();
                let insert_at = trimmed.rfind(')').unwrap_or(trimmed.len());
                format!("{}  {}\n{}", &trimmed[..insert_at], symbol_block, &trimmed[insert_at..])
            }
        }
        None => format!(
            "(kicad_symbol_lib\n  (version 20211014)\n  (generator openparts-kicad-plugin)\n  {symbol_block}\n)\n"
        ),
    }
}

/// File-name sanitisation for a part's local files: an MPN may contain
/// path separators (e.g. `LPC1114FBD48/302`), which are replaced by
/// `-`. Must stay identical to `openparts_pcbcad::safe_filename` (the
/// convention `openparts-cli`/`openparts-kicad` use to name
/// `<name>.kicad_mod` / `<name>.step`); duplicated rather than imported
/// so this plugin doesn't pull the whole pcbcad/mcad stack in for a
/// one-liner.
pub fn safe_filename(name: &str) -> String {
    name.replace(['/', '\\'], "-")
}

/// Inserts a `(model "path" ...)` block into a footprint (as returned by
/// `openparts-server`'s kicad-footprint artifact) pointing at the
/// downloaded STEP file, so the 3D viewer works immediately.
///
/// Footprints rendered by `openparts-kicad` already carry a default
/// model block (`(model "<name>.step" ...)`, a path relative to the
/// footprint library / project, which is where `openparts-cli` puts the
/// STEP). The plugin installs the STEP somewhere else
/// (`${KIPRJMOD}/openparts_3d/`), so that existing block is *replaced*
/// with this path -- never duplicated -- and its transform is kept: the
/// Placement Model's 3D alignment (the STEP is drawn in its geometry
/// generator's frame, the footprint in the model's).
pub fn patch_footprint_3d_model(footprint_text: &str, step_relative_path: &str) -> String {
    let model_block = format!(
        "  (model \"{step_relative_path}\"\n    (offset (xyz 0 0 0))\n    (scale (xyz 1 1 1))\n    (rotate (xyz 0 0 0))\n  )\n"
    );
    if let Some((start, end)) = find_balanced_block(footprint_text, "(model ") {
        // Only the quoted path after `(model ` changes.
        let block = &footprint_text[start..end];
        let path_start = "(model ".len();
        let path_end = block[path_start + 1..]
            .find('"')
            .map(|i| path_start + 1 + i + 1)
            .unwrap_or(path_start);
        format!(
            "{}(model \"{step_relative_path}\"{}{}",
            &footprint_text[..start],
            &block[path_end..],
            &footprint_text[end..]
        )
    } else {
        let trimmed = footprint_text.trim_end();
        let insert_at = trimmed.rfind(')').unwrap_or(trimmed.len());
        format!(
            "{}{}{}",
            &trimmed[..insert_at],
            model_block,
            &trimmed[insert_at..]
        )
    }
}

/// Adds a `(lib (name "openparts") ...)` row to a `sym-lib-table` or
/// `fp-lib-table` file (same format for both). `existing` is `None` if
/// the project has no such table file yet. Idempotent: if a row with
/// `lib_name` already exists, it's left untouched (not duplicated).
pub fn add_lib_table_row(
    existing: Option<&str>,
    table_tag: &str,
    lib_name: &str,
    lib_type: &str,
    uri: &str,
) -> String {
    let name_marker = format!("(name \"{lib_name}\")");
    if let Some(text) = existing {
        if text.contains(&name_marker) {
            return text.to_string();
        }
        let row = format!(
            "  (lib (name \"{lib_name}\")(type \"{lib_type}\")(uri \"{uri}\")(options \"\")(descr \"\"))\n"
        );
        let trimmed = text.trim_end();
        let insert_at = trimmed.rfind(')').unwrap_or(trimmed.len());
        format!("{}{}{}", &trimmed[..insert_at], row, &trimmed[insert_at..])
    } else {
        format!(
            "({table_tag}\n  (lib (name \"{lib_name}\")(type \"{lib_type}\")(uri \"{uri}\")(options \"\")(descr \"\"))\n)\n"
        )
    }
}

/// Writes one part's fetched artifacts into `project_dir`: the STEP
/// under `openparts_3d/`, the footprint (its 3D model pointed there)
/// under `openparts.pretty/`, the symbol merged into
/// `openparts.kicad_sym`, and the `openparts` rows of both library
/// tables. Idempotent: installing a part again replaces it.
pub fn install_into_project(
    project_dir: &std::path::Path,
    mpn: &str,
    symbol_lib_text: &str,
    footprint_text: &str,
    step: &[u8],
) -> Result<(), InstallError> {
    let file_stem = safe_filename(mpn);
    let step_dir = project_dir.join("openparts_3d");
    std::fs::create_dir_all(&step_dir)?;
    std::fs::write(step_dir.join(format!("{file_stem}.step")), step)?;

    let step_rel = format!("${{KIPRJMOD}}/openparts_3d/{file_stem}.step");
    let pretty_dir = project_dir.join("openparts.pretty");
    std::fs::create_dir_all(&pretty_dir)?;
    std::fs::write(
        pretty_dir.join(format!("{file_stem}.kicad_mod")),
        patch_footprint_3d_model(footprint_text, &step_rel),
    )?;

    let symbol_block = extract_symbol_block(symbol_lib_text, mpn)?;
    let sym_path = project_dir.join("openparts.kicad_sym");
    let existing = std::fs::read_to_string(&sym_path).ok();
    std::fs::write(
        &sym_path,
        merge_symbol_library(existing.as_deref(), mpn, &symbol_block),
    )?;

    for (file, table, uri) in [
        (
            "sym-lib-table",
            "sym_lib_table",
            "${KIPRJMOD}/openparts.kicad_sym",
        ),
        (
            "fp-lib-table",
            "fp_lib_table",
            "${KIPRJMOD}/openparts.pretty",
        ),
    ] {
        let path = project_dir.join(file);
        let existing = std::fs::read_to_string(&path).ok();
        std::fs::write(
            &path,
            add_lib_table_row(existing.as_deref(), table, "openparts", "KiCad", uri),
        )?;
    }
    Ok(())
}

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Sync(#[from] SyncError),
}

#[cfg(test)]
mod tests {
    use super::*;

    const FETCHED_SYMBOL_LIB: &str = "(kicad_symbol_lib\n  (version 20211014)\n  (generator openparts)\n  (symbol \"RP2040\"\n    (in_bom yes) (on_board yes)\n    (symbol \"RP2040_1_1\"\n      (pin power_in line (at 0 0 0) (length 2.54)\n        (name \"IOVDD\" (effects (font (size 1.27 1.27))))\n        (number \"1\" (effects (font (size 1.27 1.27)))))\n    )\n  )\n)\n";

    #[test]
    fn extracts_the_symbol_block_from_a_fetched_library() {
        let block = extract_symbol_block(FETCHED_SYMBOL_LIB, "RP2040").unwrap();
        assert!(block.starts_with("(symbol \"RP2040\""));
        assert!(block.trim_end().ends_with(')'));
        // Balanced: same number of ( and ).
        assert_eq!(block.matches('(').count(), block.matches(')').count());
    }

    #[test]
    fn merge_creates_a_new_library_when_none_exists() {
        let block = extract_symbol_block(FETCHED_SYMBOL_LIB, "RP2040").unwrap();
        let merged = merge_symbol_library(None, "RP2040", &block);
        assert!(merged.starts_with("(kicad_symbol_lib"));
        assert!(merged.contains("(symbol \"RP2040\""));
    }

    #[test]
    fn merge_appends_to_an_existing_library_without_disturbing_other_symbols() {
        let existing = "(kicad_symbol_lib\n  (version 20211014)\n  (generator openparts-kicad-plugin)\n  (symbol \"OTHER_PART\"\n    (in_bom yes)\n  )\n)\n";
        let block = extract_symbol_block(FETCHED_SYMBOL_LIB, "RP2040").unwrap();
        let merged = merge_symbol_library(Some(existing), "RP2040", &block);
        assert!(merged.contains("(symbol \"OTHER_PART\""));
        assert!(merged.contains("(symbol \"RP2040\""));
    }

    #[test]
    fn merge_is_idempotent_installing_twice_does_not_duplicate() {
        let block = extract_symbol_block(FETCHED_SYMBOL_LIB, "RP2040").unwrap();
        let once = merge_symbol_library(None, "RP2040", &block);
        let twice = merge_symbol_library(Some(&once), "RP2040", &block);
        assert_eq!(twice.matches("(symbol \"RP2040\"").count(), 1);
    }

    const FETCHED_FOOTPRINT: &str = "(footprint \"RP2040\" (version 20221018) (generator openparts)\n  (layer \"F.Cu\")\n  (pad \"1\" smd rect (at 0 0) (size 0.4 0.25) (layers \"F.Cu\"))\n)\n";

    #[test]
    fn patches_3d_model_into_a_footprint_with_none() {
        let patched =
            patch_footprint_3d_model(FETCHED_FOOTPRINT, "${KIPRJMOD}/openparts_3d/RP2040.step");
        assert!(patched.contains("(model \"${KIPRJMOD}/openparts_3d/RP2040.step\""));
        // Still balanced overall.
        assert_eq!(patched.matches('(').count(), patched.matches(')').count());
    }

    #[test]
    fn patching_twice_replaces_rather_than_duplicates() {
        let once = patch_footprint_3d_model(FETCHED_FOOTPRINT, "path/a.step");
        let twice = patch_footprint_3d_model(&once, "path/b.step");
        assert_eq!(twice.matches("(model ").count(), 1);
        assert!(twice.contains("path/b.step"));
        assert!(!twice.contains("path/a.step"));
    }

    /// Exactly what `openparts-kicad`'s `render_footprint` now emits:
    /// a default, library-relative model block after the pads.
    const FETCHED_FOOTPRINT_WITH_MODEL: &str = "(footprint \"A/B\" (version 20221018) (generator openparts)\n  (layer \"F.Cu\")\n  (pad \"1\" smd rect (at 0.000 -1.000) (size 0.400 0.250) (layers \"F.Cu\" \"F.Paste\" \"F.Mask\"))\n  (model \"A-B.step\"\n    (offset (xyz 0 0 0))\n    (scale (xyz 1 1 1))\n    (rotate (xyz 0 0 0))\n  )\n)\n";

    #[test]
    fn overrides_the_generators_default_model_block_with_the_local_path() {
        let patched = patch_footprint_3d_model(
            FETCHED_FOOTPRINT_WITH_MODEL,
            "${KIPRJMOD}/openparts_3d/A-B.step",
        );
        assert_eq!(patched.matches("(model ").count(), 1);
        assert!(patched.contains("  (model \"${KIPRJMOD}/openparts_3d/A-B.step\"\n"));
        assert!(!patched.contains("(model \"A-B.step\""));
        // Pads untouched, output still balanced and still a footprint.
        assert!(patched.contains("(pad \"1\" smd rect (at 0.000 -1.000)"));
        assert_eq!(patched.matches('(').count(), patched.matches(')').count());
        assert!(patched.trim_end().ends_with(')'));
    }

    #[test]
    fn the_model_transform_survives_the_path_patch() {
        let fetched = FETCHED_FOOTPRINT_WITH_MODEL
            .replace("(offset (xyz 0 0 0))", "(offset (xyz 0 -2.215 0))")
            .replace("(rotate (xyz 0 0 0))", "(rotate (xyz 0 0 -180))");
        let patched = patch_footprint_3d_model(&fetched, "${KIPRJMOD}/openparts_3d/A-B.step");
        assert!(patched.contains("(model \"${KIPRJMOD}/openparts_3d/A-B.step\""));
        assert!(patched.contains("(offset (xyz 0 -2.215 0))"));
        assert!(patched.contains("(rotate (xyz 0 0 -180))"));
        assert_eq!(patched.matches('(').count(), patched.matches(')').count());
    }

    #[test]
    fn safe_filename_matches_the_generator_convention() {
        assert_eq!(safe_filename("LPC1114FBD48/302"), "LPC1114FBD48-302");
        assert_eq!(safe_filename("A\\B"), "A-B");
        assert_eq!(safe_filename("RP2040"), "RP2040");
    }

    #[test]
    fn lib_table_creates_new_file_when_none_exists() {
        let table = add_lib_table_row(
            None,
            "sym_lib_table",
            "openparts",
            "KiCad",
            "${KIPRJMOD}/openparts.kicad_sym",
        );
        assert!(table.starts_with("(sym_lib_table"));
        assert!(table.contains("(name \"openparts\")"));
    }

    #[test]
    fn lib_table_preserves_other_libraries() {
        let existing = "(fp_lib_table\n  (lib (name \"Other\")(type \"KiCad\")(uri \"other\")(options \"\")(descr \"\"))\n)\n";
        let updated = add_lib_table_row(
            Some(existing),
            "fp_lib_table",
            "openparts",
            "KiCad",
            "${KIPRJMOD}/openparts.pretty",
        );
        assert!(updated.contains("(name \"Other\")"));
        assert!(updated.contains("(name \"openparts\")"));
    }

    #[test]
    fn lib_table_is_idempotent() {
        let once = add_lib_table_row(None, "sym_lib_table", "openparts", "KiCad", "uri");
        let twice = add_lib_table_row(Some(&once), "sym_lib_table", "openparts", "KiCad", "uri");
        assert_eq!(twice.matches("(name \"openparts\")").count(), 1);
    }
}
