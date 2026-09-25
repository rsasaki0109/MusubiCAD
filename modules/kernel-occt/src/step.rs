//! STEP text post-processing that keeps exports deterministic.

/// Fixed timestamp written into every exported STEP header.
pub const STEP_TIMESTAMP: &str = "1970-01-01T00:00:00";

/// Product name written instead of OCCT's per-process translator counter.
pub const STEP_PRODUCT_NAME: &str = "MusubiCAD";

/// OCCT names each exported product `Open CASCADE STEP translator <ver> <n>`,
/// where `<n>` counts exports in the process.
const OCCT_PRODUCT_PREFIX: &str = "Open CASCADE STEP translator ";

/// Make STEP output depend only on geometry: replace the wall-clock
/// `time_stamp` of the `FILE_NAME` header entity with [`STEP_TIMESTAMP`] and
/// OCCT's counting product names with [`STEP_PRODUCT_NAME`].
///
/// `FILE_NAME('name','time_stamp',(author),...)` — the time stamp is the
/// second quoted string.  Input without a `FILE_NAME` entity keeps its header.
pub fn normalize_step_header(step: &[u8]) -> Vec<u8> {
    let text = replace_product_names(&String::from_utf8_lossy(step));
    normalize_time_stamp(&text)
}

fn replace_product_names(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(start) = rest.find(OCCT_PRODUCT_PREFIX) {
        let Some(len) = rest[start..].find('\'') else {
            break;
        };
        out.push_str(&rest[..start]);
        out.push_str(STEP_PRODUCT_NAME);
        rest = &rest[start + len..];
    }
    out.push_str(rest);
    out
}

fn normalize_time_stamp(text: &str) -> Vec<u8> {
    let step = text.as_bytes();
    let Some(start) = text.find("FILE_NAME(") else {
        return step.to_vec();
    };
    // Find the second quoted string after FILE_NAME(, honouring the STEP
    // escape of a quote as two quotes.
    let bytes = text.as_bytes();
    let mut index = start + "FILE_NAME(".len();
    let mut quoted = Vec::new();
    while index < bytes.len() && quoted.len() < 2 {
        if bytes[index] == b'\'' {
            let open = index;
            index += 1;
            while index < bytes.len() {
                if bytes[index] == b'\'' {
                    if bytes.get(index + 1) == Some(&b'\'') {
                        index += 2;
                        continue;
                    }
                    break;
                }
                index += 1;
            }
            quoted.push((open, index));
        }
        index += 1;
    }
    let Some(&(open, close)) = quoted.get(1) else {
        return step.to_vec();
    };
    let mut normalized = String::with_capacity(text.len());
    normalized.push_str(&text[..=open]);
    normalized.push_str(STEP_TIMESTAMP);
    normalized.push_str(&text[close..]);
    normalized.into_bytes()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_only_the_file_name_time_stamp() {
        let step = b"HEADER;\nFILE_NAME('Open CASCADE Shape Model','2026-09-25T21:04:05',('Author'),(''),'x','y','');\nENDSEC;";
        let normalized = String::from_utf8(normalize_step_header(step)).unwrap();
        assert!(normalized
            .contains("FILE_NAME('Open CASCADE Shape Model','1970-01-01T00:00:00',('Author')"));
        assert!(normalized.ends_with("ENDSEC;"));
    }

    #[test]
    fn honours_escaped_quotes_and_missing_headers() {
        let step = b"FILE_NAME('it''s','2026-01-01T00:00:00',());";
        let normalized = String::from_utf8(normalize_step_header(step)).unwrap();
        assert_eq!(normalized, "FILE_NAME('it''s','1970-01-01T00:00:00',());");
        assert_eq!(normalize_step_header(b"no header"), b"no header".to_vec());
    }

    #[test]
    fn replaces_counting_product_names() {
        let step = b"#7 = PRODUCT('Open CASCADE STEP translator 8.0 12',
  'Open CASCADE STEP translator 8.0 12','',(#8));";
        let normalized = String::from_utf8(normalize_step_header(step)).unwrap();
        assert_eq!(
            normalized,
            "#7 = PRODUCT('MusubiCAD',
  'MusubiCAD','',(#8));"
        );
    }
}
