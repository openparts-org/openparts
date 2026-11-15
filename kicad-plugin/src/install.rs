//! Installing a part into a KiCad project: the `kicad-install` path
//! (openparts-client's hash-verified downloads, then
//! `openparts_kicad::library::install_into_project`).

use openparts_client::{ArtifactKind, Client};
use openparts_kicad::library;
use std::path::Path;

/// Fetches the three artifacts and writes/merges them into `project_dir`'s
/// local libraries.
pub fn install_part(
    client: &Client,
    manufacturer: &str,
    mpn: &str,
    revision: Option<&str>,
    project_dir: &Path,
) -> Result<String, String> {
    let symbol_artifact = client
        .get_artifact(manufacturer, mpn, ArtifactKind::KicadSymbol, revision)
        .map_err(|e| e.to_string())?;
    let footprint_artifact = client
        .get_artifact(manufacturer, mpn, ArtifactKind::KicadFootprint, revision)
        .map_err(|e| e.to_string())?;
    let step_artifact = client
        .get_artifact(manufacturer, mpn, ArtifactKind::Step, revision)
        .map_err(|e| e.to_string())?;

    library::install_into_project(
        project_dir,
        mpn,
        &symbol_artifact.content,
        &footprint_artifact.content,
        step_artifact.content.as_bytes(),
    )
    .map_err(|e| e.to_string())?;

    Ok(format!("Installed {mpn} into {}", project_dir.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercises `install_part`'s file-writing side entirely against a
    /// local filesystem fixture -- no real `openparts-server` needed,
    /// since `Client` itself already has its own network tests
    /// (openparts-client's dependency-free mock-server suite). This
    /// just confirms the orchestration + library_sync wiring is
    /// correct by pre-populating the client's cache and running fully
    /// offline.
    #[test]
    fn install_part_writes_expected_files_offline_from_cache() {
        let cache_dir = tempfile::tempdir().unwrap();
        let project_dir = tempfile::tempdir().unwrap();

        // Pre-populate the client cache exactly like openparts-client's
        // own cache layout (metadata/artifact-pointers + artifacts/sha256).
        let symbol_text = "(kicad_symbol_lib\n  (version 20211014)\n  (generator openparts)\n  (symbol \"TESTPART\"\n    (in_bom yes)\n  )\n)\n";
        // As `openparts-kicad` renders it today: with its default,
        // library-relative model block.
        let footprint_text = "(footprint \"TESTPART\" (version 20221018) (generator openparts)\n  (layer \"F.Cu\")\n  (pad \"1\" smd rect (at 0 0) (size 0.4 0.25) (layers \"F.Cu\"))\n  (model \"TESTPART.step\"\n    (offset (xyz 0 0 0))\n    (scale (xyz 1 1 1))\n    (rotate (xyz 0 0 0))\n  )\n)\n";
        let step_text = "ISO-10303-21;\nHEADER;\nENDSEC;\nDATA;\nENDSEC;\nEND-ISO-10303-21;\n";

        write_cached_artifact(
            cache_dir.path(),
            "ex",
            "TESTPART",
            "kicad-symbol",
            symbol_text,
        );
        write_cached_artifact(
            cache_dir.path(),
            "ex",
            "TESTPART",
            "kicad-footprint",
            footprint_text,
        );
        write_cached_artifact(cache_dir.path(), "ex", "TESTPART", "step", step_text);

        let client = Client::new("http://127.0.0.1:1")
            .with_cache_dir(cache_dir.path().to_path_buf())
            .offline(true);

        let message = install_part(&client, "ex", "TESTPART", None, project_dir.path()).unwrap();
        assert!(message.contains("Installed TESTPART"));

        assert!(project_dir
            .path()
            .join("openparts_3d/TESTPART.step")
            .exists());
        assert!(project_dir
            .path()
            .join("openparts.pretty/TESTPART.kicad_mod")
            .exists());
        assert!(project_dir.path().join("openparts.kicad_sym").exists());

        let footprint_out = std::fs::read_to_string(
            project_dir
                .path()
                .join("openparts.pretty/TESTPART.kicad_mod"),
        )
        .unwrap();
        assert!(footprint_out.contains("(model \"${KIPRJMOD}/openparts_3d/TESTPART.step\""));
        assert_eq!(footprint_out.matches("(model ").count(), 1);
        assert_eq!(
            std::fs::read_to_string(project_dir.path().join("openparts_3d/TESTPART.step")).unwrap(),
            step_text
        );

        let sym_table = std::fs::read_to_string(project_dir.path().join("sym-lib-table")).unwrap();
        assert!(sym_table.contains("(name \"openparts\")"));
        let fp_table = std::fs::read_to_string(project_dir.path().join("fp-lib-table")).unwrap();
        assert!(fp_table.contains("(name \"openparts\")"));
    }

    #[test]
    fn install_part_sanitises_an_mpn_containing_a_slash() {
        let cache_dir = tempfile::tempdir().unwrap();
        let project_dir = tempfile::tempdir().unwrap();
        let mpn = "LPC1114FBD48/302";
        let symbol_text = "(kicad_symbol_lib\n  (version 20211014)\n  (generator openparts)\n  (symbol \"LPC1114FBD48/302\"\n    (in_bom yes)\n  )\n)\n";
        let footprint_text = "(footprint \"LPC1114FBD48/302\" (version 20221018) (generator openparts)\n  (layer \"F.Cu\")\n  (pad \"1\" smd rect (at 0 0) (size 0.4 0.25) (layers \"F.Cu\"))\n  (model \"LPC1114FBD48-302.step\"\n    (offset (xyz 0 0 0))\n    (scale (xyz 1 1 1))\n    (rotate (xyz 0 0 0))\n  )\n)\n";
        let step_text = "ISO-10303-21;\nEND-ISO-10303-21;\n";
        write_cached_artifact(cache_dir.path(), "nxp", mpn, "kicad-symbol", symbol_text);
        write_cached_artifact(
            cache_dir.path(),
            "nxp",
            mpn,
            "kicad-footprint",
            footprint_text,
        );
        write_cached_artifact(cache_dir.path(), "nxp", mpn, "step", step_text);

        let client = Client::new("http://127.0.0.1:1")
            .with_cache_dir(cache_dir.path().to_path_buf())
            .offline(true);
        install_part(&client, "nxp", mpn, None, project_dir.path()).unwrap();

        let step = project_dir
            .path()
            .join("openparts_3d/LPC1114FBD48-302.step");
        let fp = project_dir
            .path()
            .join("openparts.pretty/LPC1114FBD48-302.kicad_mod");
        assert!(step.exists());
        let footprint_out = std::fs::read_to_string(fp).unwrap();
        assert!(footprint_out.contains("(model \"${KIPRJMOD}/openparts_3d/LPC1114FBD48-302.step\""));
        assert_eq!(footprint_out.matches("(model ").count(), 1);
    }

    fn write_cached_artifact(
        cache_dir: &Path,
        manufacturer: &str,
        mpn: &str,
        kind: &str,
        content: &str,
    ) {
        use sha2::{Digest, Sha256};
        let hash: String = {
            let mut h = Sha256::new();
            h.update(content.as_bytes());
            h.finalize().iter().map(|b| format!("{b:02x}")).collect()
        };
        // The client's cache layout: percent-encoded segments (an MPN's
        // `/` stays in one file name).
        let pointer_path = cache_dir.join(format!(
            "metadata/artifact-pointers/{}/{}/{kind}.hash",
            openparts_client::percent_encode(manufacturer),
            openparts_client::percent_encode(mpn)
        ));
        std::fs::create_dir_all(pointer_path.parent().unwrap()).unwrap();
        std::fs::write(&pointer_path, &hash).unwrap();
        let content_path = cache_dir.join(format!("artifacts/sha256/{hash}"));
        std::fs::create_dir_all(content_path.parent().unwrap()).unwrap();
        std::fs::write(&content_path, content).unwrap();
    }
}
