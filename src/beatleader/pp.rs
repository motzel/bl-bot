use std::cmp::Ordering;

pub const WEIGHT_COEFFICIENT: f64 = 0.965;
pub const CLAN_WEIGHT_COEFFICIENT: f64 = 0.8;

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

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;

    use crate::beatleader::pp::{
        calculate_pp_boundary, calculate_total_pp_from_sorted, WEIGHT_COEFFICIENT,
    };

    #[test]
    fn it_properly_calculates_total_pp() {
        let mut pps = vec![100.0, 200.0, 300.0, 400.0];
        pps.sort_unstable_by(|a, b| b.partial_cmp(a).unwrap_or(Ordering::Equal));

        assert_eq!(
            format!("{:.5}", 965.60821),
            format!("{:.5}", calculate_total_pp_from_sorted(&pps, 0))
        );

        assert_eq!(
            format!("{:.5}", 899.19851),
            format!("{:.5}", calculate_total_pp_from_sorted(&pps, 2))
        );

        let mut pps = vec![100.0, 200.0, 300.0, 400.0, 500.0];
        pps.sort_unstable_by(|a, b| b.partial_cmp(a).unwrap_or(Ordering::Equal));

        assert_eq!(
            format!("{:.5}", 1431.81193),
            format!("{:.5}", calculate_total_pp_from_sorted(&pps, 0))
        );

        assert_eq!(
            format!("{:.5}", 1333.33906),
            format!("{:.5}", calculate_total_pp_from_sorted(&pps, 2))
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
}
