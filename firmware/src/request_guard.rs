//! Defense-in-depth source filter for unauthenticated HTTP management controls.
//! A source IPv4 prefix is not authentication and does not prove AP ingress.
//!
//! Two ingress scopes are accepted for management writes:
//! - the fallback AP `/24` (bootstrap path, no token);
//! - the station subnet the device is currently attached to (home LAN), which
//!   additionally requires the per-device setup token.
//! Everything else is untrusted. The CSRF guard (`X-Matrix-Control: 1` plus
//! same-origin) applies to every scope and is never relaxed here.
use std::net::Ipv4Addr;

/// First three octets of the fallback AP network (`192.168.4.0/24`).
pub const FALLBACK_AP_PREFIX: [u8; 3] = [192, 168, 4];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ControlScope {
    /// Peer inside the station (home network) subnet; writes need a setup token.
    LocalSubnet,
    /// Peer inside the fallback AP `/24`; bootstrap writes need no token.
    FallbackAp,
    /// Anything else; never allowed to mutate configuration.
    Untrusted,
}

/// Accept only unicast clients in the configured fallback-AP `/24` source range.
pub fn trusted_management_peer(peer: Option<Ipv4Addr>) -> bool {
    peer.is_some_and(fallback_ap_peer)
}

pub fn fallback_ap_peer(peer: Ipv4Addr) -> bool {
    let octets = peer.octets();
    octets[0] == FALLBACK_AP_PREFIX[0]
        && octets[1] == FALLBACK_AP_PREFIX[1]
        && octets[2] == FALLBACK_AP_PREFIX[2]
        && (2..=254).contains(&octets[3])
}

/// True when `peer` is inside `network/prefix`. Prefixes outside 8..=30 are
/// rejected so a bogus lease can never widen the scope to a huge range.
pub fn ipv4_in_subnet(peer: Ipv4Addr, network: Ipv4Addr, prefix: u8) -> bool {
    if !(8..=30).contains(&prefix) {
        return false;
    }
    let mask = u32::MAX << (32 - prefix as u32);
    (u32::from(peer) & mask) == (u32::from(network) & mask)
}

/// Classify an HTTP peer for management access. `station_subnet` is the
/// device's own STA address plus prefix length, or `None` while the station is
/// not connected.
pub fn classify_peer(peer: Option<Ipv4Addr>, station_subnet: Option<(Ipv4Addr, u8)>) -> ControlScope {
    let Some(peer) = peer else {
        return ControlScope::Untrusted;
    };
    // Loopback and broadcast addresses never legitimately arrive over Wi-Fi;
    // treat them as untrusted so a local proxy cannot gain management rights.
    if peer.is_loopback() || peer.is_unspecified() || peer.is_broadcast() {
        return ControlScope::Untrusted;
    }
    if fallback_ap_peer(peer) {
        return ControlScope::FallbackAp;
    }
    match station_subnet {
        Some((network, prefix)) if ipv4_in_subnet(peer, network, prefix) => ControlScope::LocalSubnet,
        _ => ControlScope::Untrusted,
    }
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

/// Decide whether a mutating request may proceed.
///
/// The CSRF guard always applies. `FallbackAp` writes keep working with the AP
/// passphrase as before; `LocalSubnet` writes additionally require the
/// per-device setup token, so the home LAN cannot be reconfigured by a
/// cross-site request alone.
pub fn trusted_write(
    scope: ControlScope,
    marker: Option<&str>,
    origin: Option<&str>,
    host: Option<&str>,
    provided_token: Option<&str>,
    expected_token: &str,
) -> bool {
    if !trusted_control(origin, host, marker) {
        return false;
    }
    match scope {
        ControlScope::FallbackAp => true,
        ControlScope::LocalSubnet => crate::setup_token::verify(expected_token, provided_token),
        ControlScope::Untrusted => false,
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
    fn peer_classification_prefers_the_fallback_ap_and_rejects_foreign_networks() {
        let home = Some((Ipv4Addr::new(192, 168, 178, 35), 24));
        assert_eq!(
            classify_peer(Some(Ipv4Addr::new(192, 168, 4, 7)), home),
            ControlScope::FallbackAp
        );
        assert_eq!(
            classify_peer(Some(Ipv4Addr::new(192, 168, 178, 99)), home),
            ControlScope::LocalSubnet
        );
        assert_eq!(
            classify_peer(Some(Ipv4Addr::new(192, 168, 179, 99)), home),
            ControlScope::Untrusted
        );
        assert_eq!(classify_peer(Some(Ipv4Addr::new(10, 0, 0, 5)), home), ControlScope::Untrusted);
        assert_eq!(classify_peer(None, home), ControlScope::Untrusted);
        assert_eq!(
            classify_peer(Some(Ipv4Addr::new(192, 168, 178, 99)), None),
            ControlScope::Untrusted
        );
        // Bogus prefix lengths must not widen the scope.
        assert_eq!(
            classify_peer(Some(Ipv4Addr::new(192, 168, 178, 99)), Some((Ipv4Addr::new(0, 0, 0, 0), 0))),
            ControlScope::Untrusted
        );
        // A local peer is never trusted just because it looks like loopback.
        assert_eq!(
            classify_peer(Some(Ipv4Addr::new(127, 0, 0, 1)), Some((Ipv4Addr::new(127, 0, 0, 1), 8))),
            ControlScope::Untrusted
        );
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

    #[test]
    fn writes_from_the_home_network_require_the_setup_token() {
        let token = "ABCDEFGH";
        assert!(trusted_write(
            ControlScope::LocalSubnet,
            Some("1"),
            Some("http://192.168.178.10"),
            Some("192.168.178.10"),
            Some(token),
            token
        ));
        assert!(trusted_write(
            ControlScope::LocalSubnet,
            Some("1"),
            None,
            None,
            Some("abcdefgh"),
            token
        ));
        assert!(!trusted_write(
            ControlScope::LocalSubnet,
            Some("1"),
            Some("http://192.168.178.10"),
            Some("192.168.178.10"),
            None,
            token
        ));
        assert!(!trusted_write(
            ControlScope::LocalSubnet,
            Some("1"),
            Some("http://192.168.178.10"),
            Some("192.168.178.10"),
            Some("WRONGTKN"),
            token
        ));
        // CSRF guard still wins over a valid token.
        assert!(!trusted_write(
            ControlScope::LocalSubnet,
            Some("1"),
            Some("http://evil.test"),
            Some("192.168.178.10"),
            Some(token),
            token
        ));
        assert!(!trusted_write(ControlScope::LocalSubnet, None, None, None, Some(token), token));
    }

    #[test]
    fn fallback_ap_writes_stay_token_free_and_untrusted_peers_never_write() {
        assert!(trusted_write(
            ControlScope::FallbackAp,
            Some("1"),
            None,
            None,
            None,
            "ABCDEFGH"
        ));
        assert!(!trusted_write(
            ControlScope::Untrusted,
            Some("1"),
            None,
            None,
            Some("ABCDEFGH"),
            "ABCDEFGH"
        ));
    }
}
