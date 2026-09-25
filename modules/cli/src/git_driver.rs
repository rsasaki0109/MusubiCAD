//! `opencad merge-driver` and `opencad conflicts`: Git integration for
//! expanded `.ocad.d` documents (ADR-015).
//!
//! Git merges files; a design is a directory.  For any file of a document the
//! driver reconstructs the complete base, ours, and theirs documents from Git
//! objects, runs the whole-document semantic merge, and writes the requested
//! file of the merged result.  Every uncertain case fails closed (exit 1),
//! which Git records as an ordinary conflict.

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use opencad_ai::SemanticConflict;
use opencad_core::{OpenCadError, Result};
use opencad_file::expanded_dir::{parse_document_files, serialize_document_files};
use opencad_file::OcadDocument;
use serde_json::Value;

use crate::git_workflow::merge_documents;

/// Result of a driver run, mapped to the process exit status by the caller.
#[derive(Debug, PartialEq, Eq)]
pub enum DriverOutcome {
    /// `%A` now holds the merged file.
    Merged,
    /// Left `%A` untouched; Git records a conflict.  The reason is printed.
    Conflict(String),
}

fn git(repo: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .map_err(|error| OpenCadError::Other(format!("failed to run git: {error}")))?;
    if !output.status.success() {
        return Err(OpenCadError::Other(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(output.stdout)
}

fn git_line(repo: &Path, args: &[&str]) -> Result<String> {
    Ok(String::from_utf8_lossy(&git(repo, args)?)
        .trim()
        .to_string())
}

/// Split a repository-relative path into its `*.ocad.d` document directory
/// and the file path inside it (both with `/` separators).
pub fn split_document_path(path: &str) -> Option<(String, String)> {
    let parts: Vec<String> = Path::new(path)
        .components()
        .filter_map(|component| match component {
            Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            _ => None,
        })
        .collect();
    let index = parts.iter().rposition(|part| part.ends_with(".ocad.d"))?;
    let (document, file) = parts.split_at(index + 1);
    (!file.is_empty()).then(|| (document.join("/"), file.join("/")))
}

/// Read every file of `document` at `revision` into a map keyed by the path
/// inside the document.
fn document_files_at(
    repo: &Path,
    revision: &str,
    document: &str,
) -> Result<BTreeMap<String, Vec<u8>>> {
    let listing = git(
        repo,
        &[
            "ls-tree",
            "-r",
            "-z",
            "--name-only",
            revision,
            "--",
            document,
        ],
    )?;
    let prefix = format!("{document}/");
    let mut files = BTreeMap::new();
    for path in listing
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        let path = String::from_utf8_lossy(path).to_string();
        let Some(relative) = path.strip_prefix(&prefix) else {
            continue;
        };
        let bytes = git(repo, &["show", &format!("{revision}:{path}")])?;
        files.insert(relative.to_string(), bytes);
    }
    Ok(files)
}

/// Parsed JSON of a file, or `Null` when absent/empty, for semantic comparison.
fn json_of(bytes: Option<&[u8]>) -> Value {
    match bytes {
        Some(bytes) if !bytes.iter().all(u8::is_ascii_whitespace) => serde_json::from_slice(bytes)
            .unwrap_or(Value::String(String::from_utf8_lossy(bytes).to_string())),
        _ => Value::Null,
    }
}

/// The three revisions of an in-progress `git merge`.
struct MergeRevisions {
    base: String,
    ours: String,
    theirs: String,
}

/// The commit being merged into `HEAD`.
///
/// While a merge driver runs, `git merge` has not written `MERGE_HEAD` yet;
/// it exports `GITHEAD_<sha>` for the merged commits instead.  `MERGE_HEAD`
/// is still honoured when present (for example `opencad conflicts` after a
/// stopped merge).
fn theirs_revision(repo: &Path, ours: &str) -> std::result::Result<String, String> {
    let mut heads: Vec<String> = std::env::vars()
        .filter_map(|(key, _)| key.strip_prefix("GITHEAD_").map(str::to_string))
        .filter(|sha| !sha.eq_ignore_ascii_case(ours))
        .collect();
    heads.sort();
    heads.dedup();
    match heads.as_slice() {
        [theirs] => return Ok(theirs.clone()),
        [] => {}
        _ => return Err("octopus merges are resolved with `opencad merge`".into()),
    }
    git_line(repo, &["rev-parse", "-q", "--verify", "MERGE_HEAD"])
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            "no merge in progress: only `git merge` is supported; resolve with `opencad merge`"
                .to_string()
        })
}

fn merge_revisions(repo: &Path) -> std::result::Result<MergeRevisions, String> {
    let ours = git_line(repo, &["rev-parse", "HEAD"]).map_err(|error| error.to_string())?;
    let theirs = theirs_revision(repo, &ours)?;
    let bases = git_line(repo, &["merge-base", "--all", &ours, &theirs])
        .map_err(|error| error.to_string())?;
    let bases: Vec<&str> = bases.lines().filter(|line| !line.is_empty()).collect();
    match bases.as_slice() {
        [base] => Ok(MergeRevisions {
            base: base.to_string(),
            ours,
            theirs,
        }),
        _ => Err(format!(
            "{} merge bases: criss-cross merges are resolved with `opencad merge`",
            bases.len()
        )),
    }
}

/// A document at one revision plus its raw files keyed by path inside it.
type LoadedDocument = (OcadDocument, BTreeMap<String, Vec<u8>>);

/// Load base, ours, and theirs documents plus their raw file maps.
fn load_documents(
    repo: &Path,
    revisions: &MergeRevisions,
    document: &str,
) -> Result<[LoadedDocument; 3]> {
    let load = |revision: &str| -> Result<LoadedDocument> {
        let files = document_files_at(repo, revision, document)?;
        Ok((parse_document_files(&files)?, files))
    };
    Ok([
        load(&revisions.base)?,
        load(&revisions.ours)?,
        load(&revisions.theirs)?,
    ])
}

/// Run the merge driver for one file.  `base`, `current`, and `other` are the
/// `%O`, `%A`, and `%B` paths Git supplies; `path` is `%P`.
pub fn run_merge_driver(
    repo: &Path,
    base: &Path,
    current: &Path,
    other: &Path,
    path: &str,
) -> Result<DriverOutcome> {
    let Some((document, file)) = split_document_path(path) else {
        return Ok(DriverOutcome::Conflict(format!(
            "'{path}' is not inside a .ocad.d document"
        )));
    };
    let revisions = match merge_revisions(repo) {
        Ok(revisions) => revisions,
        Err(reason) => return Ok(DriverOutcome::Conflict(reason)),
    };
    let [(base_doc, base_files), (ours_doc, ours_files), (theirs_doc, theirs_files)] =
        load_documents(repo, &revisions, &document)?;

    // Git's own inputs must match the reconstruction, or we do not merge.
    let read = |path: &Path| std::fs::read(path).ok();
    for (label, supplied, reconstructed) in [
        ("base", read(base), base_files.get(&file)),
        ("ours", read(current), ours_files.get(&file)),
        ("theirs", read(other), theirs_files.get(&file)),
    ] {
        if json_of(supplied.as_deref()) != json_of(reconstructed.map(Vec::as_slice)) {
            return Ok(DriverOutcome::Conflict(format!(
                "{label} version of '{path}' does not match the reconstructed document"
            )));
        }
    }

    match merge_documents(&base_doc, &ours_doc, &theirs_doc)? {
        Ok(merged) => {
            let files = serialize_document_files(&merged)?;
            let Some(bytes) = files.get(&file) else {
                return Ok(DriverOutcome::Conflict(format!(
                    "'{file}' is not a canonical file of the merged document"
                )));
            };
            std::fs::write(current, bytes).map_err(|error| {
                OpenCadError::Other(format!("failed to write merge result: {error}"))
            })?;
            Ok(DriverOutcome::Merged)
        }
        Err(conflicts) => Ok(DriverOutcome::Conflict(conflict_report(
            &document, &conflicts,
        ))),
    }
}

fn conflict_report(document: &str, conflicts: &[SemanticConflict]) -> String {
    format!(
        "semantic merge of '{document}' has {} conflict(s):\n{}",
        conflicts.len(),
        serde_json::to_string_pretty(conflicts).unwrap_or_default()
    )
}

/// Conflicts of an in-progress `git merge` for one document directory.
pub fn merge_conflicts(repo: &Path, document: &str) -> Result<Vec<SemanticConflict>> {
    let revisions = merge_revisions(repo).map_err(OpenCadError::validation)?;
    let document = document.trim_end_matches(['/', '\\']).replace('\\', "/");
    let [(base, _), (ours, _), (theirs, _)] = load_documents(repo, &revisions, &document)?;
    Ok(merge_documents(&base, &ours, &theirs)?
        .err()
        .unwrap_or_default())
}

/// Repository-local `git config` entries that register the driver.
pub fn install(repo: &Path, executable: &Path) -> Result<Vec<String>> {
    let command = format!(
        "\"{}\" merge-driver %O %A %B %P",
        executable.to_string_lossy().replace('\\', "/")
    );
    git(
        repo,
        &["config", "merge.musubicad.name", "MusubiCAD semantic merge"],
    )?;
    git(repo, &["config", "merge.musubicad.driver", &command])?;
    Ok(vec![
        "*.ocad.d/*.json merge=musubicad".to_string(),
        "*.ocad.d/graph/*.json merge=musubicad".to_string(),
    ])
}

/// Repository root containing the current directory.
pub fn repository_root() -> Result<PathBuf> {
    let current = std::env::current_dir()
        .map_err(|error| OpenCadError::Other(format!("no current directory: {error}")))?;
    Ok(PathBuf::from(git_line(
        &current,
        &["rev-parse", "--show-toplevel"],
    )?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_paths_split_at_the_innermost_ocad_d_directory() {
        assert_eq!(
            split_document_path("designs/bracket.ocad.d/graph/features.json"),
            Some((
                "designs/bracket.ocad.d".into(),
                "graph/features.json".into()
            ))
        );
        assert_eq!(
            split_document_path("bracket.ocad.d/checksums.json"),
            Some(("bracket.ocad.d".into(), "checksums.json".into()))
        );
        assert_eq!(split_document_path("README.md"), None);
        assert_eq!(split_document_path("bracket.ocad.d"), None);
    }

    #[test]
    fn json_comparison_ignores_formatting_and_line_endings() {
        assert_eq!(
            json_of(Some(b"{\"a\": 1}\n")),
            json_of(Some(b"{\r\n  \"a\": 1\r\n}\r\n"))
        );
        assert_eq!(json_of(None), json_of(Some(b"  \n")));
        assert_ne!(json_of(Some(b"{\"a\": 1}")), json_of(Some(b"{\"a\": 2}")));
    }
}
