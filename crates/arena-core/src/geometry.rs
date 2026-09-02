//! Integer geometry (P0-GEOMETRY). Coordinates are `i32`; every subtraction,
//! square, and cross product promotes to `i64` first (ADR-0007). All
//! arithmetic is checked; `GameConfig::validate` caps inputs so the
//! `expect` calls below cannot fire — a panic here is a server bug.

use crate::types::{Area2, Coord};

/// Squared Euclidean distance `dx*dx + dy*dy` in cm².
pub fn dist2(a: Coord, b: Coord) -> i64 {
    let dx = (a.x as i64)
        .checked_sub(b.x as i64)
        .expect("bounded by GameConfig::validate");
    let dy = (a.y as i64)
        .checked_sub(b.y as i64)
        .expect("bounded by GameConfig::validate");
    let sx = dx.checked_mul(dx).expect("bounded by GameConfig::validate");
    let sy = dy.checked_mul(dy).expect("bounded by GameConfig::validate");
    sx.checked_add(sy).expect("bounded by GameConfig::validate")
}

/// Whether `a` and `b` lie within `range_cm` of each other, boundary
/// inclusive (compares squared distances; no square roots, no floats).
pub fn within_closed_range(a: Coord, b: Coord, range_cm: u32) -> bool {
    let d2 = dist2(a, b);
    let r = range_cm as i64;
    let r2 = r.checked_mul(r).expect("bounded by GameConfig::validate");
    d2 <= r2
}

/// Smallest `r` with `r*r >= n`; exact for perfect squares; `n == 0 -> 0`.
/// Integer bisection on u64 with a u128 midpoint product (no floats).
pub fn ceil_integer_sqrt(n: u64) -> u64 {
    if n <= 1 {
        return n;
    }
    let (mut lo, mut hi) = (1u64, n);
    // Invariant: lo*lo < n <= hi*hi (checked once up front).
    while hi.checked_sub(lo).expect("hi > lo invariant") > 1 {
        let mid = lo + ((hi - lo) / 2);
        let m2 = (mid as u128)
            .checked_mul(mid as u128)
            .expect("mid <= n < 2^64");
        if m2 >= n as u128 {
            hi = mid;
        } else {
            lo = mid;
        }
    }
    hi
}

/// `MOVE` cost for one segment (R-MOVE-01): `ceil_integer_sqrt(dx² + dy²)`
/// in coordinate units; the payload is an absolute destination, the caller
/// bills from the agent's latest position.
pub fn move_cost(from: Coord, to: Coord) -> u64 {
    let d2 = dist2(from, to);
    debug_assert!(d2 >= 0);
    ceil_integer_sqrt(d2 as u64)
}

/// z-component of the cross product `(a - o) x (b - o)`.
pub fn cross(o: Coord, a: Coord, b: Coord) -> i64 {
    let ax = (a.x as i64)
        .checked_sub(o.x as i64)
        .expect("bounded by GameConfig::validate");
    let ay = (a.y as i64)
        .checked_sub(o.y as i64)
        .expect("bounded by GameConfig::validate");
    let bx = (b.x as i64)
        .checked_sub(o.x as i64)
        .expect("bounded by GameConfig::validate");
    let by = (b.y as i64)
        .checked_sub(o.y as i64)
        .expect("bounded by GameConfig::validate");
    let lhs = ax.checked_mul(by).expect("bounded by GameConfig::validate");
    let rhs = ay.checked_mul(bx).expect("bounded by GameConfig::validate");
    lhs.checked_sub(rhs)
        .expect("bounded by GameConfig::validate")
}

/// Orientation of the ordered triple `(a, b, c)` by the sign of the cross
/// product. The clockwise/counterclockwise naming assumes a y-up plane.
#[derive(PartialEq, Eq, Debug)]
pub enum Orientation {
    Collinear,
    Clockwise,
    Counterclockwise,
}

pub fn orientation(a: Coord, b: Coord, c: Coord) -> Orientation {
    let v = cross(a, b, c);
    if v > 0 {
        Orientation::Counterclockwise
    } else if v < 0 {
        Orientation::Clockwise
    } else {
        Orientation::Collinear
    }
}

/// Whether `p` lies on the CLOSED segment `[a, b]` (endpoints included).
/// Handles the degenerate zero-length segment: true iff `p == a == b`.
pub fn point_on_segment(p: Coord, a: Coord, b: Coord) -> bool {
    if cross(a, b, p) != 0 {
        return false;
    }
    let px = p.x as i64;
    let py = p.y as i64;
    let ax = a.x as i64;
    let ay = a.y as i64;
    let bx = b.x as i64;
    let by = b.y as i64;
    px >= ax.min(bx) && px <= ax.max(bx) && py >= ay.min(by) && py <= ay.max(by)
}

/// Pure CLOSED-segment intersection predicate: any shared point — proper
/// crossing, shared endpoint, T-touch, or collinear overlap — yields `true`
/// (D8). Link-rule exceptions (shared portals, portals on segments) are
/// composed by the link rules in later milestones, not here.
pub fn segments_intersect(a1: Coord, a2: Coord, b1: Coord, b2: Coord) -> bool {
    let d1 = cross(b1, b2, a1);
    let d2 = cross(b1, b2, a2);
    let d3 = cross(a1, a2, b1);
    let d4 = cross(a1, a2, b2);

    if ((d1 > 0 && d2 < 0) || (d1 < 0 && d2 > 0)) && ((d3 > 0 && d4 < 0) || (d3 < 0 && d4 > 0)) {
        return true;
    }
    d1 == 0 && point_on_segment(a1, b1, b2)
        || d2 == 0 && point_on_segment(a2, b1, b2)
        || d3 == 0 && point_on_segment(b1, a1, a2)
        || d4 == 0 && point_on_segment(b2, a1, a2)
}

/// Twice the signed-area magnitude of triangle `(a, b, c)`, as an unsigned
/// double-square unit count; zero for collinear triples. Independent of
/// vertex ordering (absolute value).
pub fn area2(a: Coord, b: Coord, c: Coord) -> Area2 {
    let v = cross(a, b, c);
    v.unsigned_abs()
}

/// Whether `p` lies inside the closed map rectangle
/// `0 <= x <= width_cm && 0 <= y <= height_cm` (R-MAP-01).
pub fn in_bounds(p: Coord, width_cm: i32, height_cm: i32) -> bool {
    p.x >= 0 && p.x <= width_cm && p.y >= 0 && p.y <= height_cm
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dist2_basic_pythagorean() {
        assert_eq!(dist2(Coord::new(0, 0), Coord::new(3, 4)), 25);
    }

    #[test]
    fn dist2_max_map_coordinates_no_overflow() {
        assert_eq!(
            dist2(Coord::new(0, 0), Coord::new(600_000, 600_000)),
            720_000_000_000
        );
    }

    #[test]
    fn dist2_symmetric_and_zero() {
        let a = Coord::new(351_230, 178_452);
        let b = Coord::new(-4, 99_999);
        assert_eq!(dist2(a, b), dist2(b, a));
        assert_eq!(dist2(a, a), 0);
    }

    #[test]
    fn ceil_sqrt_exact_squares() {
        assert_eq!(ceil_integer_sqrt(0), 0);
        assert_eq!(ceil_integer_sqrt(1), 1);
        assert_eq!(ceil_integer_sqrt(25), 5);
        assert_eq!(ceil_integer_sqrt(10_000), 100);
        assert_eq!(ceil_integer_sqrt(720_000_000_000), 848_529);
    }

    #[test]
    fn ceil_sqrt_rounds_up_non_squares() {
        assert_eq!(ceil_integer_sqrt(2), 2);
        assert_eq!(ceil_integer_sqrt(3), 2);
        assert_eq!(ceil_integer_sqrt(5), 3);
        assert_eq!(ceil_integer_sqrt(10), 4);
        // One past a perfect square rounds up to that square's root.
        assert_eq!(ceil_integer_sqrt(848_528u64 * 848_528u64 + 1), 848_529);
        // One before a perfect square still rounds up to its root.
        assert_eq!(ceil_integer_sqrt(25u64 - 1), 5);
    }

    #[test]
    fn ceil_sqrt_property_smallest_r_with_r_squared_ge_n() {
        let check = |n: u64| {
            let r = ceil_integer_sqrt(n);
            let rr = (r as u128) * (r as u128);
            assert!(rr >= n as u128, "r*r >= n failed for n={n}");
            if r > 0 {
                let pm = (r - 1) as u128 * (r - 1) as u128;
                assert!(pm < n as u128, "(r-1)^2 < n failed for n={n}");
            }
        };
        for n in 0..=200_000u64 {
            check(n);
        }
        for &n in &[
            720_000_000_000u64,
            848_528u64 * 848_528u64,
            u32::MAX as u64,
            u32::MAX as u64 + 1,
            (1u64 << 62),
            u64::MAX,
        ] {
            check(n);
        }
    }

    #[test]
    fn move_cost_diagonal_unit_costs_two() {
        // R-MOVE-01: moving from (0,0) to (1,1) costs 2 coordinate units.
        assert_eq!(move_cost(Coord::new(0, 0), Coord::new(1, 1)), 2);
    }

    #[test]
    fn move_cost_zero_for_same_position() {
        let p = Coord::new(123_456, 654_321);
        assert_eq!(move_cost(p, p), 0);
    }

    #[test]
    fn move_cost_absolute_destination() {
        // Axis-aligned across the whole default map: payload is the absolute
        // destination, not a displacement.
        assert_eq!(move_cost(Coord::new(0, 0), Coord::new(600_000, 0)), 600_000);
        assert_eq!(
            move_cost(Coord::new(300_000, 0), Coord::new(600_000, 0)),
            300_000
        );
    }

    #[test]
    fn move_cost_per_segment_from_latest_position() {
        // (0,0) -> (3,4) -> (0,0): two segments, each billed from the latest
        // position, 5 + 5.
        let seg1 = move_cost(Coord::new(0, 0), Coord::new(3, 4));
        let seg2 = move_cost(Coord::new(3, 4), Coord::new(0, 0));
        assert_eq!(seg1 + seg2, 5 + 5);
    }

    #[test]
    fn move_cost_cumulative_80_40_50m_within_budget() {
        // R-MOVE-01 example: 80m + 40m + 50m = 170m <= 200m budget.
        let c1 = move_cost(Coord::new(0, 0), Coord::new(8_000, 0));
        let c2 = move_cost(Coord::new(8_000, 0), Coord::new(8_000, 4_000));
        let c3 = move_cost(Coord::new(8_000, 4_000), Coord::new(3_000, 4_000));
        assert_eq!(c1 + c2 + c3, 17_000);
        assert!(c1 + c2 + c3 <= 20_000);
    }

    #[test]
    fn move_cost_split_path_never_cheaper_than_direct() {
        // Splitting a move cannot beat the direct cost: triangle inequality
        // under ceil-integer cost.
        let direct = move_cost(Coord::new(0, 0), Coord::new(1, 1));
        let split = move_cost(Coord::new(0, 0), Coord::new(1, 0))
            + move_cost(Coord::new(1, 0), Coord::new(1, 1));
        assert_eq!(direct, 2);
        assert_eq!(split, 2);
        assert!(split >= direct);
        // A tighter split attempt still cannot undercut.
        let direct_far = move_cost(Coord::new(0, 0), Coord::new(3, 4));
        let split_far = move_cost(Coord::new(0, 0), Coord::new(3, 0))
            + move_cost(Coord::new(3, 0), Coord::new(3, 4));
        assert_eq!(direct_far, 5);
        assert_eq!(split_far, 7);
        assert!(split_far >= direct_far);
    }

    #[test]
    fn in_bounds_all_four_corners_true() {
        let (w, h) = (600_000, 600_000);
        for p in [
            Coord::new(0, 0),
            Coord::new(w, 0),
            Coord::new(0, h),
            Coord::new(w, h),
        ] {
            assert!(in_bounds(p, w, h), "corner {p:?} must be in closed bounds");
        }
    }

    #[test]
    fn in_bounds_all_four_closed_edges_true() {
        let (w, h) = (600_000, 600_000);
        for p in [
            Coord::new(0, h / 2),
            Coord::new(w, h / 2),
            Coord::new(w / 2, 0),
            Coord::new(w / 2, h),
        ] {
            assert!(
                in_bounds(p, w, h),
                "edge midpoint {p:?} must be in closed bounds"
            );
        }
    }

    #[test]
    fn in_bounds_rejects_width_height_plus_one() {
        let (w, h) = (600_000, 600_000);
        assert!(!in_bounds(Coord::new(w + 1, 0), w, h));
        assert!(!in_bounds(Coord::new(0, h + 1), w, h));
        assert!(!in_bounds(Coord::new(w + 1, h + 1), w, h));
    }

    #[test]
    fn in_bounds_rejects_negative_coordinates() {
        let (w, h) = (600_000, 600_000);
        assert!(!in_bounds(Coord::new(-1, 0), w, h));
        assert!(!in_bounds(Coord::new(0, -1), w, h));
        assert!(!in_bounds(Coord::new(-1, -1), w, h));
    }

    #[test]
    fn orientation_odd_cross_product_values() {
        // cross((0,0),(1,0),(0,5)) == 5 -> Counterclockwise (positive, y-up).
        assert_eq!(
            orientation(Coord::new(0, 0), Coord::new(1, 0), Coord::new(0, 5)),
            Orientation::Counterclockwise
        );
        assert_eq!(
            cross(Coord::new(0, 0), Coord::new(1, 0), Coord::new(0, 5)),
            5
        );
        // Reversed order flips the sign: -7 -> Clockwise.
        assert_eq!(
            orientation(Coord::new(0, 0), Coord::new(0, 5), Coord::new(1, 0)),
            Orientation::Clockwise
        );
        assert_eq!(
            cross(Coord::new(0, 0), Coord::new(0, 5), Coord::new(1, 0)),
            -5
        );
        // An odd negative cross value too: cross == -7.
        assert_eq!(
            orientation(Coord::new(0, 0), Coord::new(2, 3), Coord::new(1, -2)),
            Orientation::Clockwise
        );
        assert_eq!(
            cross(Coord::new(0, 0), Coord::new(2, 3), Coord::new(1, -2)),
            -7
        );
    }

    #[test]
    fn orientation_collinear() {
        assert_eq!(
            orientation(Coord::new(0, 0), Coord::new(1, 1), Coord::new(2, 2)),
            Orientation::Collinear
        );
        assert_eq!(
            orientation(Coord::new(2, 2), Coord::new(1, 1), Coord::new(0, 0)),
            Orientation::Collinear
        );
    }

    #[test]
    fn cross_at_map_extremes_no_overflow() {
        assert_eq!(
            cross(
                Coord::new(0, 0),
                Coord::new(600_000, 600_000),
                Coord::new(600_000, 0)
            ),
            -360_000_000_000
        );
    }

    #[test]
    fn point_on_segment_closed_interior_and_endpoints() {
        let a = Coord::new(0, 0);
        let b = Coord::new(10, 10);
        assert!(point_on_segment(Coord::new(5, 5), a, b));
        assert!(point_on_segment(a, a, b));
        assert!(point_on_segment(b, a, b));
    }

    #[test]
    fn point_on_segment_collinear_outside_false() {
        let a = Coord::new(0, 0);
        let b = Coord::new(10, 10);
        assert!(!point_on_segment(Coord::new(11, 11), a, b));
        assert!(!point_on_segment(Coord::new(-1, -1), a, b));
    }

    #[test]
    fn point_on_segment_off_line_false() {
        assert!(!point_on_segment(
            Coord::new(5, 6),
            Coord::new(0, 0),
            Coord::new(10, 10)
        ));
    }

    #[test]
    fn point_on_segment_zero_length_segment() {
        let a = Coord::new(3, 7);
        assert!(point_on_segment(a, a, a));
        assert!(!point_on_segment(Coord::new(3, 8), a, a));
        assert!(!point_on_segment(Coord::new(4, 7), a, a));
    }

    #[test]
    fn segments_intersect_proper_crossing() {
        // Illegal link crossing geometry.
        assert!(segments_intersect(
            Coord::new(0, 0),
            Coord::new(10, 10),
            Coord::new(0, 10),
            Coord::new(10, 0)
        ));
    }

    #[test]
    fn segments_intersect_shared_endpoint() {
        // Shared portal endpoint: the predicate reports the touch; the link
        // rules in M4 compose the "shared portal endpoint is legal" exception.
        assert!(segments_intersect(
            Coord::new(0, 0),
            Coord::new(5, 5),
            Coord::new(5, 5),
            Coord::new(10, 0)
        ));
    }

    #[test]
    fn segments_intersect_collinear_overlap() {
        assert!(segments_intersect(
            Coord::new(0, 0),
            Coord::new(10, 0),
            Coord::new(5, 0),
            Coord::new(15, 0)
        ));
    }

    #[test]
    fn segments_intersect_collinear_disjoint_false() {
        assert!(!segments_intersect(
            Coord::new(0, 0),
            Coord::new(4, 0),
            Coord::new(6, 0),
            Coord::new(10, 0)
        ));
    }

    #[test]
    fn segments_intersect_collinear_touching() {
        // Single shared point on a line.
        assert!(segments_intersect(
            Coord::new(0, 0),
            Coord::new(5, 0),
            Coord::new(5, 0),
            Coord::new(10, 0)
        ));
    }

    #[test]
    fn segments_intersect_parallel_disjoint_false() {
        assert!(!segments_intersect(
            Coord::new(0, 0),
            Coord::new(10, 0),
            Coord::new(0, 1),
            Coord::new(10, 1)
        ));
    }

    #[test]
    fn segments_intersect_t_touch_endpoint_on_interior() {
        // Endpoint of one segment resting on the interior of the other.
        assert!(segments_intersect(
            Coord::new(0, 0),
            Coord::new(10, 0),
            Coord::new(5, 0),
            Coord::new(5, 5)
        ));
    }

    #[test]
    fn segments_intersect_degenerate_point() {
        // Zero-length segments reduce to point_on_segment.
        assert!(segments_intersect(
            Coord::new(5, 5),
            Coord::new(5, 5),
            Coord::new(0, 0),
            Coord::new(10, 10)
        ));
        assert!(!segments_intersect(
            Coord::new(5, 6),
            Coord::new(5, 6),
            Coord::new(0, 0),
            Coord::new(10, 10)
        ));
        // Two distinct points never intersect.
        assert!(!segments_intersect(
            Coord::new(1, 1),
            Coord::new(1, 1),
            Coord::new(2, 2),
            Coord::new(2, 2)
        ));
        // Same degenerate point twice.
        assert!(segments_intersect(
            Coord::new(1, 1),
            Coord::new(1, 1),
            Coord::new(1, 1),
            Coord::new(1, 1)
        ));
    }

    #[test]
    fn area2_minimal_triangle_unit() {
        // Area = 0.5 cm², exactly representable as Area2 == 1.
        assert_eq!(
            area2(Coord::new(0, 0), Coord::new(1, 0), Coord::new(0, 1)),
            1
        );
    }

    #[test]
    fn area2_collinear_zero() {
        assert_eq!(
            area2(Coord::new(0, 0), Coord::new(1, 1), Coord::new(2, 2)),
            0
        );
    }

    #[test]
    fn area2_absolute_value_sign_independent() {
        let a = Coord::new(0, 0);
        let b = Coord::new(4, 0);
        let c = Coord::new(0, 3);
        // Twice the area 0.5*4*3 == 12, in either vertex order.
        let ccw = area2(a, b, c);
        let cw = area2(a, c, b);
        assert_eq!(ccw, 12);
        assert_eq!(cw, 12);
    }

    #[test]
    fn area2_max_map_triangle_fits_u64() {
        // Twice the area 0.5*600000*600000 == 600000*600000.
        assert_eq!(
            area2(
                Coord::new(0, 0),
                Coord::new(600_000, 0),
                Coord::new(0, 600_000)
            ),
            360_000_000_000u64
        );
    }

    #[test]
    fn within_closed_range_portal_operation_boundary() {
        let origin = Coord::new(0, 0);
        // Exactly 5000 cm: inclusive boundary.
        assert!(within_closed_range(origin, Coord::new(5_000, 0), 5_000));
        // 5001 cm: outside.
        assert!(!within_closed_range(origin, Coord::new(5_001, 0), 5_000));
    }

    #[test]
    fn within_closed_range_max_link_distance_boundary() {
        let origin = Coord::new(0, 0);
        assert!(within_closed_range(origin, Coord::new(200_000, 0), 200_000));
        assert!(!within_closed_range(
            origin,
            Coord::new(200_001, 0),
            200_000
        ));
        // Same point is trivially within any range.
        assert!(within_closed_range(origin, origin, 0));
    }

    #[test]
    fn checked_arithmetic_at_config_upper_bounds_no_panic() {
        // Maxed-out valid config dimensions (2^20 cm); no expect may fire.
        let (w, h) = (1 << 20, 1 << 20);
        let corners = [
            Coord::new(0, 0),
            Coord::new(w, 0),
            Coord::new(0, h),
            Coord::new(w, h),
            Coord::new(-w, -h),
        ];
        for a in corners {
            for b in corners {
                let _ = dist2(a, b);
                let _ = within_closed_range(a, b, i32::MAX as u32);
                let _ = move_cost(a, b);
            }
            for b in corners {
                for c in corners {
                    let _ = cross(a, b, c);
                    let _ = orientation(a, b, c);
                    let _ = area2(a, b, c);
                }
            }
        }
        // Largest distance-like param at the cap: r^2 fits i64.
        assert!(within_closed_range(
            Coord::new(0, 0),
            Coord::new(i32::MAX, 0),
            i32::MAX as u32
        ));
    }
}
