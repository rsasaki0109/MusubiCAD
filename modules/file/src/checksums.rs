//! File checksum utilities.

use std::collections::BTreeMap;
use std::path::Path;

use opencad_core::{sha256_hex, OpenCadError, Result};
use serde::{Deserialize, Serialize};

/// Checksum manifest for files inside a `.ocad` container.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChecksumManifest {
    pub algorithm: String,
    pub files: BTreeMap<String, String>,
}

impl ChecksumManifest {
    pub fn compute(paths: &BTreeMap<String, Vec<u8>>) -> Self {
        let files = paths
            .iter()
            .map(|(path, bytes)| (path.clone(), checksum_hex(path, bytes)))
            .collect();
        Self {
            algorithm: "sha256".into(),
            files,
        }
    }

    pub fn verify(&self, paths: &BTreeMap<String, Vec<u8>>) -> Result<()> {
        for (path, expected) in &self.files {
            let actual_bytes = paths.get(path).ok_or_else(|| {
                OpenCadError::validation(format!("checksum entry missing file '{path}'"))
            })?;
            let actual = checksum_hex(path, actual_bytes);
            if &actual != expected {
                return Err(OpenCadError::ChecksumMismatch {
                    expected: expected.clone(),
                    actual,
                });
            }
        }
        Ok(())
    }
}

fn checksum_hex(path: &str, bytes: &[u8]) -> String {
    if path.ends_with(".json") {
        let canonical = bytes
            .split(|byte| *byte == b'\n')
            .flat_map(|line| {
                let line = line.strip_suffix(b"\r").unwrap_or(line);
                line.iter().copied().chain(std::iter::once(b'\n'))
            })
            .collect::<Vec<_>>();
        let canonical = canonical.strip_suffix(b"\n").unwrap_or(&canonical).to_vec();
        sha256_hex(&canonical)
    } else {
        sha256_hex(bytes)
    }
}

pub fn hash_file(path: &Path) -> Result<String> {
    let bytes = std::fs::read(path).map_err(io_error)?;
    Ok(sha256_hex(&bytes))
}

fn io_error(err: std::io::Error) -> OpenCadError {
    OpenCadError::Other(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checksum_manifest_round_trip() {
        let mut files = BTreeMap::new();
        files.insert("graph/sketches.json".into(), br#"{"sketches":[]}"#.to_vec());
        let manifest = ChecksumManifest::compute(&files);
        manifest.verify(&files).expect("verify");
    }

    #[test]
    fn json_checksums_are_independent_of_line_endings() {
        let mut lf = BTreeMap::new();
        lf.insert(
            "document.ocad.json".into(),
            b"{\n  \"name\": \"part\"\n}".to_vec(),
        );
        let manifest = ChecksumManifest::compute(&lf);

        let mut crlf = BTreeMap::new();
        crlf.insert(
            "document.ocad.json".into(),
            b"{\r\n  \"name\": \"part\"\r\n}".to_vec(),
        );
        manifest.verify(&crlf).expect("line-ending independent");
    }

    #[test]
    fn non_json_checksums_remain_byte_exact() {
        let mut original = BTreeMap::new();
        original.insert("preview.bin".into(), b"a\nb".to_vec());
        let manifest = ChecksumManifest::compute(&original);

        let mut changed = BTreeMap::new();
        changed.insert("preview.bin".into(), b"a\r\nb".to_vec());
        assert!(manifest.verify(&changed).is_err());
    }
}
