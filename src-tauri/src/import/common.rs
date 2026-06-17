// Shared helpers for sourceless imports (CSV, Kindle, JSON): deterministic
// content-hash IDs so re-importing a file upserts rather than duplicates
// (ADR-0011), plus stable work IDs.

use sha1::{Digest, Sha1};

fn sha1_hex(parts: &[&str]) -> String {
    let mut hasher = Sha1::new();
    for (i, p) in parts.iter().enumerate() {
        if i > 0 {
            hasher.update(b"\x1f"); // unit separator between fields
        }
        hasher.update(p.as_bytes());
    }
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Stable highlight ID: same content → same ID (idempotent re-import).
pub fn highlight_id(source: &str, title: &str, author: &str, text: &str, location: &str) -> String {
    format!("{}-{}", source, sha1_hex(&[title, author, text, location]))
}

/// Stable work ID derived from title+author so highlights regroup consistently.
pub fn work_id(source: &str, title: &str, author: &str) -> String {
    format!("{}-w-{}", source, sha1_hex(&[title, author]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_deterministic() {
        let a = highlight_id("csv", "Book", "Author", "quote", "12");
        let b = highlight_id("csv", "Book", "Author", "quote", "12");
        assert_eq!(a, b);
        assert!(a.starts_with("csv-"));
    }

    #[test]
    fn ids_differ_on_content() {
        let a = highlight_id("csv", "Book", "Author", "quote one", "");
        let b = highlight_id("csv", "Book", "Author", "quote two", "");
        assert_ne!(a, b);
    }

    #[test]
    fn field_separator_prevents_collisions() {
        // "ab"+"c" must not equal "a"+"bc"
        assert_ne!(
            highlight_id("csv", "ab", "c", "", ""),
            highlight_id("csv", "a", "bc", "", "")
        );
    }
}
