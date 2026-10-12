//! System address (id64) sanity checks.

/// True if `pos` lies inside the boxel encoded in the id64 system address `addr`.
///
/// EDDI uses this to reject a `StarPos` that does not belong to a `SystemAddress`; Elite Nexus
/// uses it before trusting a location that came from the app's model rather than from a
/// `Location`/`FSDJump`/`CarrierJump` event the uplink saw itself.
pub fn star_pos_matches_address(addr: u64, pos: [f64; 3]) -> bool {
    // Old CodexEntry bugs carried SystemAddress 1; 0 means unknown.
    if addr <= 1 || !pos.iter().all(|p| p.is_finite()) {
        return false;
    }
    let mc = (addr & 7) as u32;
    let size = f64::from(10u32 << mc);
    let x0 = ((((addr >> (30 - 2 * mc)) & (0x3FFF >> mc)) << mc) * 10) as f64 - 49_985.0;
    let y0 = ((((addr >> (17 - mc)) & (0x1FFF >> mc)) << mc) * 10) as f64 - 40_985.0;
    let z0 = ((((addr >> 3) & (0x3FFF >> mc)) << mc) * 10) as f64 - 24_105.0;
    const EPS: f64 = 0.05;
    let within = |p: f64, origin: f64| {
        let d = p - origin;
        (-EPS..=size + EPS).contains(&d)
    };
    within(pos[0], x0) && within(pos[1], y0) && within(pos[2], z0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_systems_match_their_boxel() {
        // (SystemAddress, StarPos) pairs from real FSDJump/Location events.
        let real: &[(u64, [f64; 3])] = &[
            (10_477_373_803, [0.0, 0.0, 0.0]),                                 // Sol
            (6_406_446_944_978, [220.96875, -124.8125, -12.59375]),            // Synuefe BR-L c24-23
            (6_681_324_851_922, [228.375, -130.09375, -8.8125]),               // Synuefe BR-L c24-24
            (9_469_731_415_457, [207.21875, -127.28125, -14.90625]),           // Col 285 Sector FK-X b15-4
            (673_101_653_409, [159.46875, -103.75, -19.6875]),                 // Brestla
            (5_856_556_913_362, [171.71875, -109.875, -20.71875]),             // Yu Shun
            (11_668_486_301_089, [183.78125, -113.3125, -19.4375]),            // Col 285 Sector CP-X b15-5
            (3_932_277_478_106, [55.71875, 17.59375, 27.15625]),               // Shinrarta Dezhra
        ];
        for &(addr, pos) in real {
            assert!(star_pos_matches_address(addr, pos), "{addr} {pos:?}");
        }
    }

    #[test]
    fn wrong_pairs_are_rejected() {
        // Sol's address with Synuefe's position, and vice versa.
        assert!(!star_pos_matches_address(10_477_373_803, [220.96875, -124.8125, -12.59375]));
        assert!(!star_pos_matches_address(6_406_446_944_978, [0.0, 0.0, 0.0]));
        assert!(!star_pos_matches_address(1, [0.0, 0.0, 0.0]));
        assert!(!star_pos_matches_address(10_477_373_803, [f64::NAN, 0.0, 0.0]));
    }
}
