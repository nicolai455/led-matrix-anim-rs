//! Defense-in-depth source filter for unauthenticated HTTP management controls.
//! A source IPv4 prefix is not authentication and does not prove AP ingress.
/// Accept only unicast clients in the configured fallback-AP `/24` source range.
pub fn trusted_management_peer(peer: Option<std::net::Ipv4Addr>) -> bool {
    let Some(peer) = peer else { return false; };
    let octets = peer.octets();
    octets[0] == 192 && octets[1] == 168 && octets[2] == 4 && (2..=254).contains(&octets[3])
}

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
    use std::net::Ipv4Addr;

    #[test]
    fn management_controls_only_accept_clients_from_the_protected_fallback_ap_subnet() {
        assert!(trusted_management_peer(Some(Ipv4Addr::new(192, 168, 4, 2))));
        assert!(trusted_management_peer(Some(Ipv4Addr::new(192, 168, 4, 254))));
        assert!(!trusted_management_peer(None));
        assert!(!trusted_management_peer(Some(Ipv4Addr::new(192, 168, 178, 35))));
        assert!(!trusted_management_peer(Some(Ipv4Addr::new(10, 0, 0, 2))));
        assert!(!trusted_management_peer(Some(Ipv4Addr::new(192, 168, 4, 0))));
        assert!(!trusted_management_peer(Some(Ipv4Addr::new(192, 168, 4, 1))));
        assert!(!trusted_management_peer(Some(Ipv4Addr::new(192, 168, 4, 255))));
    }

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

    #[test]
    fn generic_api_control_requires_marker_and_same_origin() {
        assert!(!trusted_control(Some("http://evil.test"), Some("matrix.local"), Some("1")));
        assert!(!trusted_control(Some("null"), Some("matrix.local"), Some("1")));
        assert!(!trusted_control(Some("http://matrix.local.attacker.test"), Some("matrix.local"), Some("1")));
        assert!(trusted_control(Some("http://matrix.local"), Some("matrix.local"), Some("1")));
        assert!(trusted_control(None, Some("matrix.local"), Some("1")));
    }
}
