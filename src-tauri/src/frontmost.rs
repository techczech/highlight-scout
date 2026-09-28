//! Which app the user came from: the frontmost app other than Highlight
//! Scout, so ⌘⇧C can paste Markdown into WriteFlex and rich text elsewhere.
//!
//! macOS: `lsappinfo visibleProcessList` lists visible apps front to back;
//! the first one that is not this app is where the user was. No
//! Accessibility or Automation permission is needed. Other platforms: None.

/// The application serial numbers in an `lsappinfo visibleProcessList`
/// reply (`ASN:0x0-0x31b31b-"cmux": ASN:…`), front to back, as `ASN:0x0-0x31b31b`.
pub fn parse_visible_list(out: &str) -> Vec<String> {
    out.split("ASN:")
        .skip(1)
        .filter_map(|chunk| {
            let id: String = chunk
                .chars()
                .take_while(|c| c.is_ascii_hexdigit() || *c == 'x' || *c == '-')
                .collect();
            // "0x0-0x31b31b-" → drop the separator before the quoted name.
            let id = id.trim_end_matches('-');
            (id.contains('-') && !id.is_empty()).then(|| format!("ASN:{id}"))
        })
        .collect()
}

/// The bundle id in an `lsappinfo info -only bundleid` reply
/// (`"CFBundleIdentifier"="net.dominiklukes.writeflex"`).
pub fn parse_bundle_id(out: &str) -> Option<String> {
    let (_, v) = out.trim().split_once('=')?;
    let v = v.trim().trim_matches('"');
    (!v.is_empty() && v != "[ NULL ]").then(|| v.to_string())
}

#[cfg(target_os = "macos")]
fn lsappinfo(args: &[&str]) -> Option<String> {
    let out = std::process::Command::new("/usr/bin/lsappinfo")
        .args(args)
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The bundle id of the frontmost visible app that is not `own_bundle_id`.
#[cfg(target_os = "macos")]
pub fn other_frontmost_app(own_bundle_id: &str) -> Option<String> {
    let list = lsappinfo(&["visibleProcessList"])?;
    parse_visible_list(&list)
        .iter()
        .take(4)
        .filter_map(|asn| lsappinfo(&["info", "-only", "bundleid", asn]).and_then(|o| parse_bundle_id(&o)))
        .find(|b| b != own_bundle_id)
}

#[cfg(not(target_os = "macos"))]
pub fn other_frontmost_app(_own_bundle_id: &str) -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_visible_list_front_to_back() {
        let out = "ASN:0x0-0x5b6eb69-\"Highlight_Scout\": ASN:0x0-0x539f39a-\"WriteFlex\": ASN:0x0-0x10010-\"Finder\": \n";
        assert_eq!(
            parse_visible_list(out),
            vec!["ASN:0x0-0x5b6eb69", "ASN:0x0-0x539f39a", "ASN:0x0-0x10010"]
        );
        assert!(parse_visible_list("").is_empty());
    }

    #[test]
    fn parses_a_bundle_id() {
        assert_eq!(
            parse_bundle_id("\"CFBundleIdentifier\"=\"net.dominiklukes.writeflex\"\n").as_deref(),
            Some("net.dominiklukes.writeflex")
        );
        assert_eq!(parse_bundle_id(""), None);
        assert_eq!(parse_bundle_id("\"CFBundleIdentifier\"=[ NULL ]"), None);
    }
}
