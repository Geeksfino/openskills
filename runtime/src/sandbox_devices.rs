//! Shared platform device grants for Landlock (Linux) and Seatbelt (macOS).
//!
//! Single source of truth so `run_sandboxed_command` and native skill
//! execution cannot drift on `/dev/null` redirects.
//!
//! **Landlock invariant:** never add `/dev` as a read-only parent. Nested
//! Landlock rules take the intersection; an RO grant on `/dev` strips write
//! from `/dev/null` even when `/dev/null` is listed as RW. Seatbelt does not
//! have that intersection rule, so macOS may still grant `file-read*` on
//! `/dev` as a directory.

use std::path::PathBuf;

/// Character devices every confined process may **read**.
///
/// `/dev/null` and `/dev/zero` are **not** here — they need write (shell
/// redirects) and live in [`DEVICE_READ_WRITE_PATHS`].
///
/// Production Landlock collectors are Linux-only; the table stays compiled on
/// every OS so the invariant tests can run on macOS.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) const DEVICE_READ_ONLY_PATHS: &[&str] = &["/dev/urandom", "/dev/random"];

/// Devices that must be **read-write**.
///
/// Shell redirects (`cmd >/dev/null`, `2>/dev/null`) write these nodes.
/// Used as Landlock RW on Linux and `file-write*` literals on macOS Seatbelt.
pub(crate) const DEVICE_READ_WRITE_PATHS: &[&str] = &["/dev/null", "/dev/zero"];

/// Append existing paths from `entries` into `dest`, skipping duplicates.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub(crate) fn extend_existing_paths(dest: &mut Vec<PathBuf>, entries: &[&str]) {
    for entry in entries {
        let path = PathBuf::from(entry);
        if path.exists() && !dest.contains(&path) {
            dest.push(path);
        }
    }
}

/// Seatbelt `file-write*` literals for [`DEVICE_READ_WRITE_PATHS`].
/// `escape` is the platform-specific seatbelt path escaper.
pub(crate) fn append_seatbelt_write_device_rules(
    profile: &mut String,
    escape: impl Fn(&str) -> String,
) {
    for path in DEVICE_READ_WRITE_PATHS {
        profile.push_str(&format!(
            "(allow file-write* (literal \"{}\"))\n",
            escape(path)
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_only_and_read_write_are_disjoint() {
        for path in DEVICE_READ_ONLY_PATHS {
            assert!(
                !DEVICE_READ_WRITE_PATHS.contains(path),
                "{path} must not appear in both RO and RW device lists"
            );
        }
    }

    #[test]
    fn neither_list_includes_dev_parent() {
        // Landlock nested rules take the intersection. An RO grant on `/dev`
        // would strip write from `/dev/null`.
        assert!(!DEVICE_READ_ONLY_PATHS.contains(&"/dev"));
        assert!(!DEVICE_READ_WRITE_PATHS.contains(&"/dev"));
    }

    #[test]
    fn redirect_devices_are_read_write_only() {
        assert!(DEVICE_READ_WRITE_PATHS.contains(&"/dev/null"));
        assert!(DEVICE_READ_WRITE_PATHS.contains(&"/dev/zero"));
        assert!(!DEVICE_READ_ONLY_PATHS.contains(&"/dev/null"));
        assert!(!DEVICE_READ_ONLY_PATHS.contains(&"/dev/zero"));
    }

    #[test]
    fn prng_devices_are_read_only() {
        assert!(DEVICE_READ_ONLY_PATHS.contains(&"/dev/urandom"));
        assert!(DEVICE_READ_ONLY_PATHS.contains(&"/dev/random"));
        assert!(!DEVICE_READ_WRITE_PATHS.contains(&"/dev/urandom"));
        assert!(!DEVICE_READ_WRITE_PATHS.contains(&"/dev/random"));
    }

    #[test]
    fn seatbelt_rules_emit_literal_writes_for_rw_devices() {
        let mut profile = String::new();
        append_seatbelt_write_device_rules(&mut profile, |p| p.to_string());
        assert!(profile.contains("(allow file-write* (literal \"/dev/null\"))"));
        assert!(profile.contains("(allow file-write* (literal \"/dev/zero\"))"));
        assert!(!profile.contains("(subpath \"/dev\")"));
    }

    #[test]
    fn extend_existing_paths_skips_missing_and_duplicates() {
        let mut dest = vec![PathBuf::from("/dev/null")];
        extend_existing_paths(
            &mut dest,
            &["/dev/null", "/this/does/not/exist/openskills-device-test"],
        );
        assert_eq!(
            dest.iter().filter(|p| p.as_os_str() == "/dev/null").count(),
            1
        );
        assert!(!dest
            .iter()
            .any(|p| { p.as_os_str() == "/this/does/not/exist/openskills-device-test" }));
    }
}
