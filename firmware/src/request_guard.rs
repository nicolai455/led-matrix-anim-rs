//! Browser-origin protection for unauthenticated LAN recovery controls.
//! This is CSRF protection, not authentication against clients on the LAN.
pub fn trusted_control(origin: Option<&str>, host: Option<&str>, marker: Option<&str>) -> bool {
    if marker != Some("1") {
        return false;
    }
    match origin {
        None => true, // CLI clients must explicitly opt in with the marker.
        Some(origin) => host.is_some_and(|h| origin == format!("http://{h}")),
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_cross_site_forms_and_foreign_fetches() {
        assert!(!trusted_control(None, Some("matrix.local"), None));
        assert!(!trusted_control(
            Some("http://evil.test"),
            Some("matrix.local"),
            Some("1")
        ));
        assert!(trusted_control(
            Some("http://matrix.local"),
            Some("matrix.local"),
            Some("1")
        ));
        assert!(trusted_control(None, None, Some("1")));
    }
}
