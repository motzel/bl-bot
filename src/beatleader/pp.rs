use std::cmp::Ordering;

pub const WEIGHT_COEFFICIENT: f64 = 0.965;
pub const CLAN_WEIGHT_COEFFICIENT: f64 = 0.8;
const CURVE: &[(f64, f64)] = &[
    (1.0, 7.424),
    (0.999, 6.241),
    (0.9975, 5.158),
    (0.995, 4.01),
    (0.9925, 3.241),
    (0.99, 2.7),
    (0.9875, 2.303),
    (0.985, 2.007),
    (0.9825, 1.786),
    (0.98, 1.618),
    (0.9775, 1.49),
    (0.975, 1.392),
    (0.9725, 1.315),
    (0.97, 1.256),
    (0.965, 1.167),
    (0.96, 1.094),
    (0.955, 1.039),
    (0.95, 1.0),
    (0.94, 0.931),
    (0.93, 0.867),
    (0.92, 0.813),
    (0.91, 0.768),
    (0.9, 0.729),
    (0.875, 0.65),
    (0.85, 0.581),
    (0.825, 0.522),
    (0.8, 0.473),
    (0.75, 0.404),
    (0.7, 0.345),
    (0.65, 0.296),
    (0.6, 0.256),
    (0.0, 0.0),
];

#[derive(Debug, Clone)]
pub(crate) struct StarRating {
    pub pass: f64,
    pub tech: f64,
    pub acc: f64,
}

pub(crate) fn calculate_total_pp_from_sorted(coefficient: f64, pps: &[f64], start_idx: u32) -> f64 {
    pps.iter().enumerate().fold(0.0, |acc, (idx, pp)| {
        acc + coefficient.powi(idx as i32 + start_idx as i32) * pp
    })
}

fn calculate_raw_pp_at_idx(coefficient: f64, bottom_pps: &[f64], idx: u32, expected: f64) -> f64 {
    let old_bottom_pp = calculate_total_pp_from_sorted(coefficient, bottom_pps, idx);
    let new_bottom_pp = calculate_total_pp_from_sorted(coefficient, bottom_pps, idx + 1);

    // 0.965^idx * rawPpToFind = expected + oldBottomPp - newBottomPp;
    // rawPpToFind = (expected + oldBottomPp - newBottomPp) / 0.965^idx;
    (expected + old_bottom_pp - new_bottom_pp) / coefficient.powi(idx as i32)
}

pub(crate) fn calculate_pp_boundary(coefficient: f64, pps: &mut [f64], expected_pp: f64) -> f64 {
    if pps.is_empty() {
        return 0.0;
    }

    pps.sort_unstable_by(|a, b| b.partial_cmp(a).unwrap_or(Ordering::Equal));

    let mut idx = pps.len() as i32 - 1;
    while idx >= 0 {
        let mut bottom_data: Vec<f64> = pps[idx as usize..].to_vec();
        let bottom_pp = calculate_total_pp_from_sorted(coefficient, &bottom_data, idx as u32);

        bottom_data.insert(0, pps[idx as usize]);
        let modified_bottom_pp =
            calculate_total_pp_from_sorted(coefficient, &bottom_data, idx as u32);

        let diff = modified_bottom_pp - bottom_pp;

        if diff > expected_pp {
            return calculate_raw_pp_at_idx(
                coefficient,
                &pps[(idx as usize + 1)..],
                idx as u32 + 1,
                expected_pp,
            );
        }

        idx -= 1;
    }

    calculate_raw_pp_at_idx(coefficient, pps, 0, expected_pp)
}

pub fn curve_at_value(value: f64) -> f64 {
    let mut idx: usize = 0;
    for (i, val) in CURVE.iter().enumerate() {
        if val.0 <= value {
            idx = i;
            break;
        }
    }

    if idx == 0 {
        idx = 1;
    }

    let middle = (value - CURVE[idx - 1].0) / (CURVE[idx].0 - CURVE[idx - 1].0);

    CURVE[idx - 1].1 + middle * (CURVE[idx].1 - CURVE[idx - 1].1)
}

fn curve_pp(acc: f64, star_rating: StarRating, is_golf: bool) -> f64 {
    let mut pass_pp = 15.2 * (star_rating.pass.powf(1.0 / 2.62)).exp() - 30.0;
    if pass_pp.is_nan() || !pass_pp.is_finite() {
        pass_pp = 0.0;
    }
    let tech_pp = (acc * 1.9).exp() * 1.08 * star_rating.tech;
    let acc_pp = if is_golf {
        acc * star_rating.acc * 42.0
    } else {
        curve_at_value(acc) * star_rating.acc * 34.0
    };

    (650.0 * (pass_pp + tech_pp + acc_pp).powf(1.3)) / 650.0_f64.powf(1.3)
}

pub(crate) fn calculate_pp_from_acc(
    acc: f64,
    star_rating: StarRating,
    mode_name: &str,
    is_golf: bool,
) -> f64 {
    if !(0.00..=1.0).contains(&acc) || (is_golf && acc > 0.5) {
        return 0.0;
    }

    if mode_name == "rhythmgamestandard" {
        return acc * star_rating.pass * 55.0;
    }

    curve_pp(if is_golf { 1.0 - acc } else { acc }, star_rating, is_golf)
}

pub(crate) fn calculate_acc_from_pp(
    pp: f64,
    star_rating: StarRating,
    mode_name: &str,
) -> Option<f64> {
    if pp < 0.0 || CURVE.len() < 2 {
        return None;
    }

    if mode_name == "rhythmgamestandard" {
        return if star_rating.pass == 0.0 {
            None
        } else {
            Some(pp / (star_rating.pass * 55.0))
        };
    }

    let max_pp = curve_pp(1.0, star_rating.clone(), false);

    if pp > max_pp {
        return None;
    }

    if pp == max_pp {
        return Some(1.0);
    }

    let mut min_idx = 0;
    let mut max_idx = CURVE.len() - 1;

    let mut iteration = 0;
    while max_idx - min_idx > 1 && iteration < CURVE.len() {
        let middle_idx = (max_idx - min_idx + 1) / 2 + min_idx;
        if middle_idx > CURVE.len() - 1 {
            return None;
        }

        let middle_pp = curve_pp(CURVE[middle_idx].0, star_rating.clone(), false);

        (min_idx, max_idx) = if pp > middle_pp {
            (min_idx, middle_idx)
        } else {
            (middle_idx, max_idx)
        };

        iteration += 1;
    }

    let mut min_acc = CURVE[max_idx].0;
    let mut max_acc = CURVE[min_idx].0;

    const MAX_ITERATIONS: u32 = 50;
    let mut iteration = 0;

    loop {
        let middle_acc = (max_acc - min_acc) / 2.0 + min_acc;
        let middle_pp = curve_pp(middle_acc, star_rating.clone(), false);

        if format!("{:.2}", pp) == format!("{:.2}", middle_pp) {
            return Some(middle_acc);
        }

        (min_acc, max_acc) = if pp > middle_pp {
            (middle_acc, max_acc)
        } else {
            (min_acc, middle_acc)
        };

        iteration += 1;
        if iteration >= MAX_ITERATIONS {
            return Some(middle_acc);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use crate::beatleader::pp::{
        calculate_acc_from_pp, calculate_pp_boundary, calculate_pp_from_acc,
        calculate_total_pp_from_sorted, StarRating, WEIGHT_COEFFICIENT,
    };

    #[test]
    fn it_properly_calculates_total_pp() {
        let mut pps = vec![100.0, 200.0, 300.0, 400.0];
        pps.sort_unstable_by(|a, b| b.partial_cmp(a).unwrap_or(Ordering::Equal));

        assert_eq!(
            format!("{:.5}", 965.60821),
            format!(
                "{:.5}",
                calculate_total_pp_from_sorted(WEIGHT_COEFFICIENT, &pps, 0)
            )
        );

        assert_eq!(
            format!("{:.5}", 899.19851),
            format!(
                "{:.5}",
                calculate_total_pp_from_sorted(WEIGHT_COEFFICIENT, &pps, 2)
            )
        );

        let mut pps = vec![100.0, 200.0, 300.0, 400.0, 500.0];
        pps.sort_unstable_by(|a, b| b.partial_cmp(a).unwrap_or(Ordering::Equal));

        assert_eq!(
            format!("{:.5}", 1431.81193),
            format!(
                "{:.5}",
                calculate_total_pp_from_sorted(WEIGHT_COEFFICIENT, &pps, 0)
            )
        );

        assert_eq!(
            format!("{:.5}", 1333.33906),
            format!(
                "{:.5}",
                calculate_total_pp_from_sorted(WEIGHT_COEFFICIENT, &pps, 2)
            )
        );
    }

    #[test]
    fn it_properly_calculates_pp_boundary() {
        let mut pps = vec![100.0, 200.0, 300.0, 400.0];

        assert_eq!(
            format!("{:.5}", 1.15316),
            format!(
                "{:.5}",
                calculate_pp_boundary(WEIGHT_COEFFICIENT, &mut pps, 1.0)
            )
        );

        assert_eq!(
            format!("{:.5}", 383.20859),
            format!(
                "{:.5}",
                calculate_pp_boundary(WEIGHT_COEFFICIENT, &mut pps, 350.0)
            )
        );

        assert_eq!(
            format!("{:.5}", 142.60030),
            format!(
                "{:.5}",
                calculate_pp_boundary(WEIGHT_COEFFICIENT, &mut pps, 125.0)
            )
        );

        let mut pps = vec![100.0, 200.0, 300.0, 400.0, 500.0];

        assert_eq!(
            format!("{:.5}", 1.19499),
            format!(
                "{:.5}",
                calculate_pp_boundary(WEIGHT_COEFFICIENT, &mut pps, 1.0)
            )
        );

        assert_eq!(
            format!("{:.5}", 396.36330),
            format!(
                "{:.5}",
                calculate_pp_boundary(WEIGHT_COEFFICIENT, &mut pps, 350.0)
            )
        );

        assert_eq!(
            format!("{:.5}", 147.645390),
            format!(
                "{:.5}",
                calculate_pp_boundary(WEIGHT_COEFFICIENT, &mut pps, 125.0)
            )
        );
    }

    #[test]
    fn it_properly_calculates_pp_from_acc_for_standard_map() {
        let pp = calculate_pp_from_acc(
            0.9714286,
            StarRating {
                pass: 1.0176061,
                tech: 1.0611571,
                acc: 5.0896196,
            },
            "Standard",
            false,
        );

        assert_eq!(format!("{:.5}", 179.95542), format!("{:.5}", pp));
    }

    #[test]
    fn it_properly_calculates_pp_from_acc_for_rythm_map() {
        let pp = calculate_pp_from_acc(
            0.9714286,
            StarRating {
                pass: 1.0176061,
                tech: 1.0611571,
                acc: 5.0896196,
            },
            "rhythmgamestandard",
            false,
        );

        assert_eq!(format!("{:.5}", 54.3692417990953), format!("{:.5}", pp));
    }

    #[test]
    fn it_properly_calculates_pp_from_acc_for_golf_standard_map() {
        let pp = calculate_pp_from_acc(
            0.110933885,
            StarRating {
                pass: 21.964252,
                tech: 5.369696,
                acc: 12.5513315,
            },
            "Standard",
            true,
        );

        assert_eq!(format!("{:.5}", 939.23509), format!("{:.5}", pp));
    }

    #[test]
    fn it_returns_not_possible_acc_for_pp_lower_than_zero() {
        let acc = calculate_acc_from_pp(
            -1.0,
            StarRating {
                pass: 1.0176061,
                tech: 1.0611571,
                acc: 5.0896196,
            },
            "Standard",
        );

        assert!(acc.is_none());
    }

    #[test]
    fn it_properly_calculates_acc_from_pp_for_standard_map() {
        let acc = calculate_acc_from_pp(
            179.95542,
            StarRating {
                pass: 1.0176061,
                tech: 1.0611571,
                acc: 5.0896196,
            },
            "Standard",
        );

        assert!(acc.is_some());
        assert_eq!(format!("{:.5}", 0.9714286), format!("{:.5}", acc.unwrap()));
    }

    #[test]
    fn it_properly_calculates_acc_from_pp_for_rythm_map() {
        let acc = calculate_acc_from_pp(
            54.3692417990953,
            StarRating {
                pass: 1.0176061,
                tech: 1.0611571,
                acc: 5.0896196,
            },
            "rhythmgamestandard",
        );

        assert!(acc.is_some());
        assert_eq!(format!("{:.5}", 0.9714286), format!("{:.5}", acc.unwrap()));
    }
}
