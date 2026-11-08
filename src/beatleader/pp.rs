use crate::beatleader::player::ModifiersRatings;
use std::cmp::Ordering;

pub const WEIGHT_COEFFICIENT: f64 = 0.965;
pub const CLAN_WEIGHT_COEFFICIENT: f64 = 0.8;

// Legacy pp formula. Difficulties keep it until they get the rebalanced ratings (BL sends noteSpan only for those).
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

// Rebalanced pp formula (v4.1), port of beatleader-website/src/utils/beatleader/rebalanced-pp.js
const BASE_CURVE: &[(f64, f64)] = &[
    (1.0, 7.2),
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
    (0.94, 0.926),
    (0.93, 0.861),
    (0.92, 0.804),
    (0.91, 0.76),
    (0.9, 0.727),
    (0.875, 0.675),
    (0.85, 0.627),
    (0.825, 0.584),
    (0.8, 0.549),
    (0.75, 0.469),
    (0.7, 0.4),
    (0.65, 0.343),
    (0.6, 0.297),
    (0.0, 0.0),
];

const PASS_SCALE: f64 = 12.3;
const PASS_EXPONENT: f64 = 2.62;
const PASS_OFFSET: f64 = 30.0;
const PASS_INNER: f64 = 0.85966;
const ACC_WEIGHT: f64 = 34.519;
const ACC_CURVE_POWER: f64 = 0.9;
const LOW_STAR_CURVE_POWER: f64 = 1.0;
const LOW_STAR_CURVE_START: f64 = 7.1529;
const LOW_STAR_CURVE_END: f64 = 9.6894;
const VERY_LOW_STAR_CURVE_POWER: f64 = 1.08;
const VERY_LOW_STAR_CURVE_START: f64 = 3.478;
const VERY_LOW_STAR_CURVE_END: f64 = 5.3718;
const TECH_WEIGHT: f64 = 5.7887;
const TECH_ACC_EXPONENT: f64 = 0.0;
const STAR_DIVISOR: f64 = 52.0;
const PP_MULTIPLIER: f64 = 1.0128;
const LOW_STAR_BONUS: f64 = 0.007593;
const LOW_STAR_BONUS_END: f64 = 8.8584;
const LOW_STAR_BONUS_POWER: f64 = 1.98;
const MID_STAR_TOP_BONUS: f64 = 0.05;
const MID_STAR_TOP_START: f64 = 4.2508;
const MID_STAR_TOP_FULL: f64 = 6.3762;
const MID_STAR_TOP_FULL_END: f64 = 8.0;
const MID_STAR_TOP_END: f64 = 10.5;
const MID_STAR_TOP_ACC_BELOW: f64 = 0.03;
const MID_STAR_TOP_ACC_ABOVE: f64 = 0.0;
const PASS_TOP_KNEE: f64 = 13.0;
const PASS_TOP_SLOPE: f64 = 0.5;
const TECH_TOP_KNEE: f64 = 6.2754;
const TECH_TOP_SLOPE: f64 = 0.7;
const PASS_ACC_EXPONENT: f64 = 6.0;
const PASS_ACC_MAX_MULTIPLIER: f64 = 3.0;
const PASS_ACC_MIN_MULTIPLIER: f64 = 0.8;
const PASS_ACC_ERROR_OFFSET: f64 = 1.0;
const GOLF_ACC_WEIGHT: f64 = 38.0;

const STAR_ACCURACY: f64 = 0.96;

// (predicted_acc, A, C, K, W, K2, P2)
const CURVE_PARAMS_TABLE: &[(f64, f64, f64, f64, f64, f64, f64)] = &[
    (0.955, 1.0, -5.3, 4.86, 0.6, 2.88, 2.46),
    (0.96, 0.99, -3.0, 4.3, 0.66, 4.8, 1.85),
    (0.965, 0.988, -2.4, 4.5, 0.64, 4.5, 1.95),
    (0.97, 0.986, -2.2, 4.7, 0.62, 4.2, 2.05),
    (0.975, 0.983, 0.0, 4.86, 0.6, 3.8, 2.15),
    (0.98, 0.983, 0.0, 4.86, 0.6, 3.35, 2.3),
    (0.985, 0.985, -0.6, 4.86, 0.6, 2.88, 2.46),
    (0.99, 0.99, 0.0, 4.86, 0.6, 2.88, 2.46),
    (0.995, 0.99, 0.0, 4.86, 0.6, 2.88, 2.46),
];
const CURVE_MAX_C: f64 = 1.0;
const SLIDER_TILT: f64 = 0.2;
const SLIDER_TILT_RANGE: f64 = 0.015;
const LOW_NOTE_START: f64 = 115.0;
const LOW_NOTE_REF_NOTES: f64 = 20.0;
const LOW_NOTE_REF_MULTIPLIER: f64 = 0.8031;
const LOW_NOTE_MIN_MULTIPLIER: f64 = 0.8031;
const SHORT_START: f64 = 75.0;
const SHORT_REF_SPAN: f64 = 12.0;
const SHORT_REF_MULTIPLIER: f64 = 0.9;
const SHORT_MIN_MULTIPLIER: f64 = 0.9;

#[derive(Debug, Clone, Default)]
pub(crate) struct StarRating {
    pub pass: f64,
    pub tech: f64,
    pub acc: f64,
    pub predicted_acc: Option<f64>,
    pub slider_percentage: Option<f64>,
    pub notes: Option<u32>,
    /// rebalanced pp formula is used only if note_span is present (as on beatleader.com)
    pub note_span: Option<f64>,
    /// stars used by the rebalanced formula; calculated from ratings if None
    /// (set by get_modified_ratings() to stars of ratings not multiplied by modifiers)
    pub stars: Option<f64>,
}

impl StarRating {
    fn is_rebalanced(&self) -> bool {
        self.note_span.is_some()
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct SpeedModifierRating {
    pub pass: f64,
    pub tech: f64,
    pub acc: f64,
    pub predicted_acc: Option<f64>,
    pub slider_percentage: Option<f64>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ModifiersRating {
    pub ss: Option<SpeedModifierRating>,
    pub fs: Option<SpeedModifierRating>,
    pub sf: Option<SpeedModifierRating>,
    pub bfs: Option<SpeedModifierRating>,
    pub bsf: Option<SpeedModifierRating>,
}

impl ModifiersRating {
    fn get(&self, modifier_name: &str) -> Option<(&SpeedModifierRating, f64)> {
        match modifier_name {
            "ss" => self.ss.as_ref().map(|r| (r, 0.85)),
            "fs" => self.fs.as_ref().map(|r| (r, 1.2)),
            "sf" => self.sf.as_ref().map(|r| (r, 1.5)),
            "bfs" => self.bfs.as_ref().map(|r| (r, 1.2)),
            "bsf" => self.bsf.as_ref().map(|r| (r, 1.5)),
            _ => None,
        }
    }
}

impl From<&ModifiersRatings> for ModifiersRating {
    fn from(r: &ModifiersRatings) -> Self {
        let speed = |stars: f64, pass, tech, acc, predicted_acc, slider_percentage| {
            (stars > 0.0).then_some(SpeedModifierRating {
                pass,
                tech,
                acc,
                predicted_acc,
                slider_percentage,
            })
        };

        Self {
            ss: speed(
                r.ss_stars,
                r.ss_pass_rating,
                r.ss_tech_rating,
                r.ss_acc_rating,
                r.ss_predicted_acc,
                r.ss_slider_percentage,
            ),
            fs: speed(
                r.fs_stars,
                r.fs_pass_rating,
                r.fs_tech_rating,
                r.fs_acc_rating,
                r.fs_predicted_acc,
                r.fs_slider_percentage,
            ),
            sf: speed(
                r.sf_stars,
                r.sf_pass_rating,
                r.sf_tech_rating,
                r.sf_acc_rating,
                r.sf_predicted_acc,
                r.sf_slider_percentage,
            ),
            bfs: speed(
                r.bfs_stars,
                r.bfs_pass_rating,
                r.bfs_tech_rating,
                r.bfs_acc_rating,
                r.bfs_predicted_acc,
                None,
            ),
            bsf: speed(
                r.bsf_stars,
                r.bsf_pass_rating,
                r.bsf_tech_rating,
                r.bsf_acc_rating,
                r.bsf_predicted_acc,
                None,
            ),
        }
    }
}

/// modifier name (eg. "SS", "GN") and its value from difficulty.modifierValues
#[derive(Debug, Clone)]
pub(crate) struct Modifier<'a> {
    pub name: &'a str,
    pub value: f64,
}

// excluded from multiplier if modifiers rating is present (bfs/bsf are not, as on beatleader.com)
const SPEED_MODIFIERS: &[&str] = &["ss", "fs", "sf"];

/// port of getModifiedRatings() from beatleader-website/src/utils/beatleader/pp.js
pub(crate) fn get_modified_ratings(
    star_rating: &StarRating,
    modifiers_rating: Option<&ModifiersRating>,
    modifiers: &[Modifier],
) -> StarRating {
    let mut rating = StarRating {
        stars: None,
        ..star_rating.clone()
    };
    let mut multiplier_mods: Vec<&Modifier> = modifiers.iter().collect();

    if let Some(modifiers_rating) = modifiers_rating {
        for modifier in modifiers {
            if let Some((speed_rating, timescale)) =
                modifiers_rating.get(&modifier.name.to_lowercase())
            {
                rating.pass = speed_rating.pass;
                rating.tech = speed_rating.tech;
                rating.acc = speed_rating.acc;
                rating.predicted_acc = speed_rating.predicted_acc;
                rating.slider_percentage = speed_rating
                    .slider_percentage
                    .or(star_rating.slider_percentage);
                rating.note_span = star_rating.note_span.map(|span| span / timescale);
                break;
            }
        }

        multiplier_mods.retain(|m| !SPEED_MODIFIERS.contains(&m.name.to_lowercase().as_str()));
    }

    let multiplier = 1.0 + multiplier_mods.iter().map(|m| m.value).sum::<f64>();

    if rating.is_rebalanced() {
        rating.stars = Some(RebalancedRating::new(&rating).stars);
    }

    rating.pass *= multiplier;
    rating.tech *= multiplier;
    rating.acc *= multiplier;

    rating
}

/// port of computeStarRating() from beatleader-website/src/utils/beatleader/pp.js
/// (stars calculated from given, possibly modified, ratings)
pub(crate) fn calculate_stars(star_rating: &StarRating) -> f64 {
    if star_rating.is_rebalanced() {
        RebalancedRating::new(&StarRating {
            stars: None,
            ..star_rating.clone()
        })
        .stars
    } else {
        legacy_curve_pp(STAR_ACCURACY, star_rating, false) / 52.0
    }
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

fn interpolate_curve(curve: &[(f64, f64)], value: f64) -> f64 {
    let mut idx: usize = curve.len() - 1;
    for (i, val) in curve.iter().enumerate() {
        if val.0 <= value {
            idx = i;
            break;
        }
    }

    if idx == 0 {
        idx = 1;
    }

    let middle = (value - curve[idx - 1].0) / (curve[idx].0 - curve[idx - 1].0);

    curve[idx - 1].1 + middle * (curve[idx].1 - curve[idx - 1].1)
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

fn legacy_curve_pp(acc: f64, star_rating: &StarRating, is_golf: bool) -> f64 {
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

#[derive(Debug, Clone, Copy)]
struct CurveParams {
    a: f64,
    c: f64,
    k: f64,
    w: f64,
    k2: f64,
    p2: f64,
}

impl Default for CurveParams {
    fn default() -> Self {
        Self {
            a: 1.0,
            c: 1.0,
            k: 4.86,
            w: 0.6,
            k2: 2.88,
            p2: 2.46,
        }
    }
}

const MODIFIABLE_CURVE_Y0: f64 = 0.256;

impl CurveParams {
    fn is_default_row(&self) -> bool {
        self.a >= 0.9995 && self.k == 4.86 && self.w == 0.6 && self.k2 == 2.88 && self.p2 == 2.46
    }

    fn m(&self) -> f64 {
        let term1 = (1.0 - MODIFIABLE_CURVE_Y0) / 0.35;
        let term2 = 1.0 - self.w + (self.w * (self.k * self.k.exp())) / (self.k.exp() - 1.0);
        term1 * term2
    }

    fn f1(&self, x: f64) -> f64 {
        let u = (x - 0.6) / 0.35;
        let blend =
            (1.0 - self.w) * u + (self.w * ((self.k * u).exp() - 1.0)) / (self.k.exp() - 1.0);
        MODIFIABLE_CURVE_Y0 + (1.0 - MODIFIABLE_CURVE_Y0) * blend
    }

    fn f2(&self, x: f64) -> f64 {
        let m = self.m();
        let u2 = (x - 0.95) / 0.05;
        let exp_term = ((self.k2 * u2.powf(self.p2)).exp() - 1.0) / (self.k2.exp() - 1.0);
        1.0 + m * (x - 0.95) + (6.424 - 0.05 * m) * exp_term
    }

    fn s(&self) -> f64 {
        let m = self.m();
        let u_a = (self.a - 0.95) / 0.05;
        let u_a_power = if u_a > 0.0 { u_a.powf(self.p2) } else { 0.0 };
        let u_a_power_minus_1 = if u_a > 0.0 {
            u_a.powf(self.p2 - 1.0)
        } else {
            0.0
        };
        let exp_term = (self.k2 * u_a_power).exp() / (self.k2.exp() - 1.0);
        let derivative_factor = ((self.k2 * self.p2) / 0.05) * u_a_power_minus_1;
        m + (6.424 - 0.05 * m) * exp_term * derivative_factor
    }

    fn l(&self, x: f64) -> f64 {
        let s = self.s();
        let s1 = s * (1.0 - self.c);
        let f2_a = self.f2(self.a);
        if (self.a - 1.0).abs() < 1e-9 {
            return f2_a + s1 * (x - self.a);
        }
        let exp_term = 1.0 - (-(x - self.a) / (1.0 - self.a)).exp();
        f2_a + s1 * (x - self.a) + (s - s1) * (1.0 - self.a) * exp_term
    }

    fn value(&self, x: f64) -> f64 {
        if x <= 0.6 {
            (MODIFIABLE_CURVE_Y0 / 0.6) * x
        } else if x <= 0.95 {
            self.f1(x)
        } else if x <= self.a {
            self.f2(x)
        } else {
            self.l(x)
        }
    }
}

fn params_at(predicted_acc: f64) -> CurveParams {
    let row = |r: &(f64, f64, f64, f64, f64, f64, f64)| CurveParams {
        a: r.1,
        c: r.2,
        k: r.3,
        w: r.4,
        k2: r.5,
        p2: r.6,
    };
    let clamp = |p: CurveParams| CurveParams {
        c: p.c.min(CURVE_MAX_C),
        ..p
    };

    let rows = CURVE_PARAMS_TABLE;
    if predicted_acc <= rows[0].0 {
        return clamp(row(&rows[0]));
    }

    for i in 1..rows.len() {
        if predicted_acc <= rows[i].0 {
            let p = row(&rows[i - 1]);
            let n = row(&rows[i]);
            let t = (predicted_acc - rows[i - 1].0) / (rows[i].0 - rows[i - 1].0);
            let lerp = |x: f64, y: f64| x + t * (y - x);
            return clamp(CurveParams {
                a: lerp(p.a, n.a),
                c: lerp(p.c, n.c),
                k: lerp(p.k, n.k),
                w: lerp(p.w, n.w),
                k2: lerp(p.k2, n.k2),
                p2: lerp(p.p2, n.p2),
            });
        }
    }

    clamp(row(&rows[rows.len() - 1]))
}

/// (modified, standard)
fn per_map_curves(predicted_acc: f64) -> Option<(CurveParams, CurveParams)> {
    if predicted_acc <= 0.6 || predicted_acc >= 0.9995 {
        return None;
    }

    let row = params_at(predicted_acc);
    if row.is_default_row() {
        return None;
    }

    Some((
        CurveParams {
            a: row.a.clamp(0.951, 1.0),
            ..row
        },
        CurveParams::default(),
    ))
}

/// monotone cubic (Fritsch-Carlson) interpolation of BASE_CURVE
fn smooth_base_value(acc: f64) -> f64 {
    let points: Vec<(f64, f64)> = BASE_CURVE.iter().rev().copied().collect();
    let n = points.len();
    let xs: Vec<f64> = points.iter().map(|p| p.0).collect();
    let ys: Vec<f64> = points.iter().map(|p| p.1).collect();

    if acc <= xs[0] {
        return ys[0];
    }
    if acc >= xs[n - 1] {
        return ys[n - 1];
    }

    let h: Vec<f64> = (0..n - 1).map(|i| xs[i + 1] - xs[i]).collect();
    let d: Vec<f64> = (0..n - 1).map(|i| (ys[i + 1] - ys[i]) / h[i]).collect();
    let mut slopes = vec![0.0; n];
    slopes[0] = d[0];
    slopes[n - 1] = d[n - 2];
    for i in 1..n - 1 {
        if d[i - 1] * d[i] <= 0.0 {
            continue;
        }
        let w1 = 2.0 * h[i] + h[i - 1];
        let w2 = h[i] + 2.0 * h[i - 1];
        slopes[i] = (w1 + w2) / (w1 / d[i - 1] + w2 / d[i]);
    }

    let mut i = 0;
    while xs[i + 1] < acc {
        i += 1;
    }
    let h = xs[i + 1] - xs[i];
    let t = (acc - xs[i]) / h;
    let t2 = t * t;
    let t3 = t2 * t;

    (2.0 * t3 - 3.0 * t2 + 1.0) * ys[i]
        + (t3 - 2.0 * t2 + t) * h * slopes[i]
        + (-2.0 * t3 + 3.0 * t2) * ys[i + 1]
        + (t3 - t2) * h * slopes[i + 1]
}

fn table_factor(standard: &CurveParams, x: f64) -> f64 {
    let reference = standard.value(x);
    if reference > 0.0 {
        smooth_base_value(x) / reference
    } else {
        0.0
    }
}

fn top_table_factor(standard: &CurveParams, a: f64, x: f64) -> f64 {
    const H: f64 = 1e-5;
    let at_a = table_factor(standard, a);
    let slope = (at_a - table_factor(standard, a - H)) / H;

    at_a + slope * (x - a)
}

fn per_map_value(curves: &(CurveParams, CurveParams), x: f64) -> f64 {
    let base = smooth_base_value(x);
    let a = curves.0.a;
    let factor = if a < 0.9995 && x > a {
        top_table_factor(&curves.1, a, x)
    } else {
        table_factor(&curves.1, x)
    };
    let value = if factor > 0.0 {
        curves.0.value(x) * factor
    } else {
        base
    };

    base.min(value)
}

fn slider_tilt_at(slider_percentage: f64, predicted_acc: f64) -> f64 {
    if SLIDER_TILT == 0.0 || predicted_acc <= 0.6 || predicted_acc >= 0.9995 {
        return 0.0;
    }

    SLIDER_TILT * slider_percentage
}

fn nerf_factor(
    value: f64,
    start: f64,
    reference: f64,
    ref_multiplier: f64,
    min_multiplier: f64,
) -> f64 {
    if start <= 0.0 || value <= 0.0 || value >= start || reference >= start {
        return 1.0;
    }

    let nerf = ((1.0 - ref_multiplier) * (start / value).ln()) / (start / reference).ln();

    (1.0 - nerf).clamp(min_multiplier, 1.0)
}

fn low_note_multiplier(notes: f64, span: f64) -> f64 {
    nerf_factor(
        notes,
        LOW_NOTE_START,
        LOW_NOTE_REF_NOTES,
        LOW_NOTE_REF_MULTIPLIER,
        LOW_NOTE_MIN_MULTIPLIER,
    ) * nerf_factor(
        span,
        SHORT_START,
        SHORT_REF_SPAN,
        SHORT_REF_MULTIPLIER,
        SHORT_MIN_MULTIPLIER,
    )
}

fn round6(x: f64) -> f64 {
    (x * 1e6).round() / 1e6
}

/// builds per map acc curve, sorted by acc descending
fn build_acc_curve(
    predicted_acc: f64,
    slider_percentage: f64,
    notes: f64,
    note_span: f64,
) -> Vec<(f64, f64)> {
    let curves = per_map_curves(predicted_acc);
    let tilt = slider_tilt_at(slider_percentage, predicted_acc);
    let nerf = low_note_multiplier(notes, note_span);

    if curves.is_none() && tilt == 0.0 {
        return BASE_CURVE.iter().map(|(x, y)| (*x, y * nerf)).collect();
    }

    let a = curves.map_or(1.0, |c| c.0.a);
    let mut xs: Vec<f64> = BASE_CURVE.iter().map(|(x, _)| round6(*x)).collect();
    xs.extend((0..71).map(|i| round6(0.6 + i as f64 * 0.005)));
    xs.extend((0..101).map(|i| round6(0.95 + i as f64 * 0.0005)));
    xs.push(round6(a));
    xs.retain(|x| (0.0..=1.0).contains(x));
    xs.sort_unstable_by(|a, b| b.partial_cmp(a).unwrap_or(Ordering::Equal));
    xs.dedup();

    xs.into_iter()
        .map(|x| {
            let mut value = match &curves {
                Some(curves) => per_map_value(curves, x),
                None => interpolate_curve(BASE_CURVE, x),
            };
            if tilt != 0.0 {
                let distance = (x - predicted_acc).clamp(-SLIDER_TILT_RANGE, SLIDER_TILT_RANGE);
                value *= (tilt * distance * 100.0).exp();
            }
            (x, value * nerf)
        })
        .collect()
}

fn pass_pp(pass_rating: f64) -> f64 {
    let mut pass = pass_rating;
    if PASS_TOP_SLOPE != 1.0 && pass > PASS_TOP_KNEE {
        pass = PASS_TOP_KNEE + (pass - PASS_TOP_KNEE) * PASS_TOP_SLOPE;
    }

    let result = PASS_SCALE * (PASS_INNER * pass.powf(1.0 / PASS_EXPONENT)).exp() - PASS_OFFSET;
    if !result.is_finite() || result < 0.0 {
        0.0
    } else {
        result
    }
}

fn curve_power_at(stars: f64) -> f64 {
    if VERY_LOW_STAR_CURVE_POWER != LOW_STAR_CURVE_POWER && stars < VERY_LOW_STAR_CURVE_END {
        if stars <= VERY_LOW_STAR_CURVE_START {
            return VERY_LOW_STAR_CURVE_POWER;
        }
        return VERY_LOW_STAR_CURVE_POWER
            + ((LOW_STAR_CURVE_POWER - VERY_LOW_STAR_CURVE_POWER)
                * (stars - VERY_LOW_STAR_CURVE_START))
                / (VERY_LOW_STAR_CURVE_END - VERY_LOW_STAR_CURVE_START);
    }

    if LOW_STAR_CURVE_POWER == ACC_CURVE_POWER || stars >= LOW_STAR_CURVE_END || stars.is_nan() {
        return ACC_CURVE_POWER;
    }
    if stars <= LOW_STAR_CURVE_START {
        return LOW_STAR_CURVE_POWER;
    }

    LOW_STAR_CURVE_POWER
        + ((ACC_CURVE_POWER - LOW_STAR_CURVE_POWER) * (stars - LOW_STAR_CURVE_START))
            / (LOW_STAR_CURVE_END - LOW_STAR_CURVE_START)
}

fn acc_curve(curve_value: f64, curve_at_96: f64, power: f64) -> f64 {
    if power == 1.0
        || curve_value <= 0.0
        || curve_at_96 <= 0.0
        || curve_value.is_nan()
        || curve_at_96.is_nan()
    {
        return curve_value;
    }

    curve_at_96 * (curve_value / curve_at_96).powf(power)
}

fn acc_pp(curve_value: f64, acc_rating: f64) -> f64 {
    if acc_rating > 0.0 {
        curve_value * ACC_WEIGHT * acc_rating
    } else {
        0.0
    }
}

fn ease(t: f64) -> f64 {
    let clamped = t.clamp(0.0, 1.0);
    clamped * clamped * (3.0 - 2.0 * clamped)
}

fn mid_star_top_bump(stars: f64) -> f64 {
    if stars.is_nan() || stars <= MID_STAR_TOP_START || stars >= MID_STAR_TOP_END {
        return 0.0;
    }
    if stars < MID_STAR_TOP_FULL {
        return ease((stars - MID_STAR_TOP_START) / (MID_STAR_TOP_FULL - MID_STAR_TOP_START));
    }
    if stars > MID_STAR_TOP_FULL_END {
        return ease((MID_STAR_TOP_END - stars) / (MID_STAR_TOP_END - MID_STAR_TOP_FULL_END));
    }

    1.0
}

fn mid_star_top_multiplier(stars: f64, accuracy: f64, predicted_acc: f64) -> f64 {
    if MID_STAR_TOP_BONUS == 0.0 || predicted_acc.is_nan() || predicted_acc <= 0.0 {
        return 1.0;
    }

    let from = predicted_acc - MID_STAR_TOP_ACC_BELOW;
    if accuracy.is_nan() || accuracy <= from {
        return 1.0;
    }

    let bump = mid_star_top_bump(stars);
    if bump == 0.0 {
        return 1.0;
    }

    let ramp = ease((accuracy - from) / (MID_STAR_TOP_ACC_BELOW + MID_STAR_TOP_ACC_ABOVE));

    1.0 + MID_STAR_TOP_BONUS * bump * ramp
}

fn pass_acc_multiplier(accuracy: f64, predicted_acc: f64) -> f64 {
    if PASS_ACC_EXPONENT == 0.0 || predicted_acc.is_nan() || predicted_acc <= 0.0 {
        return 1.0;
    }

    let gap = accuracy - (predicted_acc - PASS_ACC_ERROR_OFFSET * (1.0 - predicted_acc));
    let m = (PASS_ACC_EXPONENT * gap).exp();
    let min = if PASS_ACC_MIN_MULTIPLIER > 0.0 {
        PASS_ACC_MIN_MULTIPLIER
    } else {
        1.0 / PASS_ACC_MAX_MULTIPLIER
    };

    m.max(min).min(PASS_ACC_MAX_MULTIPLIER)
}

fn tech_pp(accuracy: f64, tech_rating: f64) -> f64 {
    if tech_rating <= 0.0 || tech_rating.is_nan() {
        return 0.0;
    }

    let mut tech = tech_rating;
    if tech > TECH_TOP_KNEE {
        tech = TECH_TOP_KNEE + (tech - TECH_TOP_KNEE) * TECH_TOP_SLOPE;
    }

    (TECH_ACC_EXPONENT * accuracy).exp() * TECH_WEIGHT * tech
}

fn stars_from_pp_no_bonus(pp_at_96: f64) -> f64 {
    pp_at_96 / STAR_DIVISOR
}

fn low_star_multiplier(stars: f64) -> f64 {
    if LOW_STAR_BONUS <= 0.0 || stars.is_nan() || stars >= LOW_STAR_BONUS_END {
        return 1.0;
    }

    1.0 + LOW_STAR_BONUS * (LOW_STAR_BONUS_END - stars.max(0.0)).powf(LOW_STAR_BONUS_POWER)
}

fn stars_from_pp(pp_at_96: f64) -> f64 {
    let stars = stars_from_pp_no_bonus(pp_at_96);
    if pp_at_96 <= 0.0 || pp_at_96.is_nan() || LOW_STAR_BONUS <= 0.0 || stars >= LOW_STAR_BONUS_END
    {
        return stars;
    }

    let mut lo = stars;
    let mut hi = LOW_STAR_BONUS_END;
    while hi - lo > 1e-6 {
        let mid = (lo + hi) / 2.0;
        if mid < stars_from_pp_no_bonus(pp_at_96 * low_star_multiplier(mid)) {
            lo = mid;
        } else {
            hi = mid;
        }
    }

    (lo + hi) / 2.0
}

struct RebalancedRating<'a> {
    star_rating: &'a StarRating,
    curve: Vec<(f64, f64)>,
    stars: f64,
}

impl<'a> RebalancedRating<'a> {
    fn new(star_rating: &'a StarRating) -> Self {
        let curve = build_acc_curve(
            star_rating.predicted_acc.unwrap_or(0.0),
            star_rating.slider_percentage.unwrap_or(0.0),
            star_rating.notes.unwrap_or(0) as f64,
            star_rating.note_span.unwrap_or(0.0),
        );

        let mut rating = Self {
            star_rating,
            curve,
            stars: 0.0,
        };

        rating.stars = star_rating.stars.unwrap_or_else(|| {
            let (pass, acc, tech) = rating.pp_parts(STAR_ACCURACY, None, false);
            stars_from_pp(pass + acc + tech)
        });

        rating
    }

    fn predicted_acc(&self) -> f64 {
        self.star_rating.predicted_acc.unwrap_or(0.0)
    }

    fn pp_parts(&self, accuracy: f64, stars: Option<f64>, is_golf: bool) -> (f64, f64, f64) {
        let power = stars.map_or(ACC_CURVE_POWER, curve_power_at);
        let acc = if is_golf {
            accuracy * self.star_rating.acc * GOLF_ACC_WEIGHT
        } else {
            let mut value = interpolate_curve(&self.curve, accuracy);
            if power != 1.0 {
                value = acc_curve(value, interpolate_curve(&self.curve, STAR_ACCURACY), power);
            }
            acc_pp(value, self.star_rating.acc)
        };

        (
            pass_pp(self.star_rating.pass) * pass_acc_multiplier(accuracy, self.predicted_acc()),
            acc,
            tech_pp(accuracy, self.star_rating.tech),
        )
    }

    fn pp(&self, accuracy: f64, is_golf: bool) -> f64 {
        if accuracy <= 0.0 || accuracy > 1.0 || accuracy.is_nan() {
            return 0.0;
        }

        let (pass, base_acc, tech) = self.pp_parts(accuracy, Some(self.stars), is_golf);
        let acc = base_acc * mid_star_top_multiplier(self.stars, accuracy, self.predicted_acc());
        let total = PP_MULTIPLIER * (pass + acc + tech) * low_star_multiplier(self.stars);

        if total.is_finite() {
            total
        } else {
            0.0
        }
    }
}

enum PpCurve<'a> {
    Legacy(&'a StarRating),
    Rebalanced(RebalancedRating<'a>),
}

impl<'a> PpCurve<'a> {
    fn new(star_rating: &'a StarRating) -> Self {
        if star_rating.is_rebalanced() {
            PpCurve::Rebalanced(RebalancedRating::new(star_rating))
        } else {
            PpCurve::Legacy(star_rating)
        }
    }

    fn pp(&self, acc: f64, is_golf: bool) -> f64 {
        match self {
            PpCurve::Legacy(star_rating) => legacy_curve_pp(acc, star_rating, is_golf),
            PpCurve::Rebalanced(rating) => rating.pp(acc, is_golf),
        }
    }

    fn accuracies(&self) -> Vec<f64> {
        match self {
            PpCurve::Legacy(_) => CURVE.iter().map(|(x, _)| *x).collect(),
            PpCurve::Rebalanced(rating) => rating.curve.iter().map(|(x, _)| *x).collect(),
        }
    }
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

    PpCurve::new(&star_rating).pp(if is_golf { 1.0 - acc } else { acc }, is_golf)
}

pub(crate) fn calculate_acc_from_pp(
    pp: f64,
    star_rating: StarRating,
    mode_name: &str,
) -> Option<f64> {
    if pp < 0.0 {
        return None;
    }

    if mode_name == "rhythmgamestandard" {
        return if star_rating.pass == 0.0 {
            None
        } else {
            Some(pp / (star_rating.pass * 55.0))
        };
    }

    let pp_curve = PpCurve::new(&star_rating);
    let accuracies = pp_curve.accuracies();
    if accuracies.len() < 2 {
        return None;
    }

    let max_pp = pp_curve.pp(1.0, false);

    if pp > max_pp {
        return None;
    }

    if pp == max_pp {
        return Some(1.0);
    }

    let mut min_idx = 0;
    let mut max_idx = accuracies.len() - 1;

    let mut iteration = 0;
    while max_idx - min_idx > 1 && iteration < accuracies.len() {
        let middle_idx = (max_idx - min_idx).div_ceil(2) + min_idx;
        if middle_idx > accuracies.len() - 1 {
            return None;
        }

        let middle_pp = pp_curve.pp(accuracies[middle_idx], false);

        (min_idx, max_idx) = if pp > middle_pp {
            (min_idx, middle_idx)
        } else {
            (middle_idx, max_idx)
        };

        iteration += 1;
    }

    let mut min_acc = accuracies[max_idx];
    let mut max_acc = accuracies[min_idx];

    const MAX_ITERATIONS: u32 = 50;
    let mut iteration = 0;

    loop {
        let middle_acc = (max_acc - min_acc) / 2.0 + min_acc;
        let middle_pp = pp_curve.pp(middle_acc, false);

        if format!("{pp:.2}") == format!("{middle_pp:.2}") {
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
        calculate_acc_from_pp, calculate_pp_boundary, calculate_pp_from_acc, calculate_stars,
        calculate_total_pp_from_sorted, get_modified_ratings, Modifier, ModifiersRating,
        SpeedModifierRating, StarRating, WEIGHT_COEFFICIENT,
    };

    fn legacy_star_rating() -> StarRating {
        StarRating {
            pass: 1.0176061,
            tech: 1.0611571,
            acc: 5.0896196,
            ..Default::default()
        }
    }

    // https://api.beatleader.com/leaderboard/1719291
    fn rebalanced_star_rating() -> StarRating {
        StarRating {
            pass: 8.903147,
            tech: 5.277968,
            acc: 12.219628,
            predicted_acc: Some(0.9701281),
            slider_percentage: Some(0.06842573),
            notes: Some(2645),
            note_span: Some(328.6154),
            stars: None,
        }
    }

    fn modifiers_rating() -> ModifiersRating {
        ModifiersRating {
            ss: Some(SpeedModifierRating {
                pass: 7.4410725,
                tech: 4.959645,
                acc: 11.155663,
                predicted_acc: Some(0.9742056),
                slider_percentage: Some(0.06842565),
            }),
            fs: Some(SpeedModifierRating {
                pass: 10.746338,
                tech: 5.638189,
                acc: 13.655256,
                predicted_acc: Some(0.96436447),
                slider_percentage: Some(0.068425745),
            }),
            sf: Some(SpeedModifierRating {
                pass: 14.033539,
                tech: 6.3873434,
                acc: 15.949651,
                predicted_acc: Some(0.95600766),
                slider_percentage: Some(0.06842581),
            }),
            bfs: None,
            bsf: None,
        }
    }

    // values calculated by the PP curve widget on beatleader.com
    const REBALANCED_EXPECTED: &[(f64, f64)] = &[
        (0.7, 262.734),
        (0.8, 323.624),
        (0.9, 395.742),
        (0.96, 557.082),
        (0.975, 655.812),
        (0.99, 1066.264),
    ];

    // https://api.beatleader.com/leaderboard/ff0291 (low stars, low note count, short map)
    fn low_star_rating() -> StarRating {
        StarRating {
            pass: 1.5907664,
            tech: 2.61118,
            acc: 5.458809,
            predicted_acc: Some(0.9887902),
            slider_percentage: Some(0.010989015),
            notes: Some(93),
            note_span: Some(22.19758),
            stars: None,
        }
    }

    // values calculated by the PP curve widget on beatleader.com
    const LOW_STAR_EXPECTED: &[(f64, f64)] = &[
        (0.8, 127.147),
        (0.9, 162.851),
        (0.96, 237.906),
        (0.98, 347.484),
        (0.99, 573.913),
    ];

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
        let pp = calculate_pp_from_acc(0.9714286, legacy_star_rating(), "Standard", false);

        assert_eq!(format!("{:.5}", 179.95542), format!("{:.5}", pp));
    }

    #[test]
    fn it_properly_calculates_pp_from_acc_for_rebalanced_standard_map() {
        for (acc, expected_pp) in REBALANCED_EXPECTED {
            let pp = calculate_pp_from_acc(*acc, rebalanced_star_rating(), "Standard", false);

            assert_eq!(
                format!("{:.3}", expected_pp),
                format!("{:.3}", pp),
                "acc: {acc}"
            );
        }
    }

    #[test]
    fn it_properly_calculates_pp_from_acc_for_rythm_map() {
        let pp =
            calculate_pp_from_acc(0.9714286, legacy_star_rating(), "rhythmgamestandard", false);

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
                ..Default::default()
            },
            "Standard",
            true,
        );

        assert_eq!(format!("{:.5}", 939.23509), format!("{:.5}", pp));
    }

    #[test]
    fn it_returns_zero_pp_for_acc_out_of_range() {
        assert_eq!(
            0.0,
            calculate_pp_from_acc(1.1, rebalanced_star_rating(), "Standard", false)
        );
        assert_eq!(
            0.0,
            calculate_pp_from_acc(0.6, rebalanced_star_rating(), "Standard", true)
        );
    }

    #[test]
    fn it_returns_not_possible_acc_for_pp_lower_than_zero() {
        let acc = calculate_acc_from_pp(-1.0, legacy_star_rating(), "Standard");

        assert!(acc.is_none());

        let acc = calculate_acc_from_pp(-1.0, rebalanced_star_rating(), "Standard");

        assert!(acc.is_none());
    }

    #[test]
    fn it_returns_not_possible_acc_for_pp_higher_than_max() {
        let acc = calculate_acc_from_pp(10000.0, rebalanced_star_rating(), "Standard");

        assert!(acc.is_none());
    }

    #[test]
    fn it_properly_calculates_acc_from_pp_for_standard_map() {
        let acc = calculate_acc_from_pp(179.95542, legacy_star_rating(), "Standard");

        assert!(acc.is_some());
        assert_eq!(format!("{:.5}", 0.9714286), format!("{:.5}", acc.unwrap()));
    }

    #[test]
    fn it_properly_calculates_acc_from_pp_for_rebalanced_standard_map() {
        for (expected_acc, pp) in REBALANCED_EXPECTED {
            let acc = calculate_acc_from_pp(*pp, rebalanced_star_rating(), "Standard");

            assert!(acc.is_some());
            assert_eq!(
                format!("{:.4}", expected_acc),
                format!("{:.4}", acc.unwrap()),
                "pp: {pp}"
            );
        }
    }

    #[test]
    fn it_properly_calculates_acc_from_pp_for_rythm_map() {
        let acc =
            calculate_acc_from_pp(54.3692417990953, legacy_star_rating(), "rhythmgamestandard");

        assert!(acc.is_some());
        assert_eq!(format!("{:.5}", 0.9714286), format!("{:.5}", acc.unwrap()));
    }

    #[test]
    fn it_properly_calculates_stars() {
        // stars from https://api.beatleader.com/leaderboard/1719291
        assert_eq!(
            format!("{:.4}", 10.577722),
            format!("{:.4}", calculate_stars(&rebalanced_star_rating()))
        );

        let star_rating = StarRating {
            pass: 21.964252,
            tech: 5.369696,
            acc: 12.5513315,
            ..Default::default()
        };
        // legacy stars = pp at 96% / 52
        assert_eq!(
            format!(
                "{:.5}",
                calculate_pp_from_acc(0.96, star_rating.clone(), "Standard", false) / 52.0
            ),
            format!("{:.5}", calculate_stars(&star_rating))
        );
    }

    #[test]
    fn it_does_not_modify_ratings_without_modifiers() {
        let star_rating = rebalanced_star_rating();
        let modified = get_modified_ratings(&star_rating, Some(&modifiers_rating()), &[]);

        assert_eq!(star_rating.pass, modified.pass);
        assert_eq!(star_rating.tech, modified.tech);
        assert_eq!(star_rating.acc, modified.acc);
        assert_eq!(star_rating.note_span, modified.note_span);
        assert_eq!(
            format!("{:.4}", 10.577722),
            format!("{:.4}", modified.stars.unwrap())
        );

        for (acc, expected_pp) in REBALANCED_EXPECTED {
            let pp = calculate_pp_from_acc(*acc, modified.clone(), "Standard", false);

            assert_eq!(format!("{:.3}", expected_pp), format!("{:.3}", pp));
        }
    }

    #[test]
    fn it_properly_modifies_ratings_with_speed_modifier() {
        // stars from https://api.beatleader.com/leaderboard/1719291
        for (name, expected_stars, expected_note_span) in [
            ("SS", 9.467636, 328.6154 / 0.85),
            ("FS", 12.119211, 328.6154 / 1.2),
            ("SF", 14.783086, 328.6154 / 1.5),
        ] {
            let modifiers_rating = modifiers_rating();
            let speed_rating = modifiers_rating
                .get(&name.to_lowercase())
                .unwrap()
                .0
                .clone();
            let modified = get_modified_ratings(
                &rebalanced_star_rating(),
                Some(&modifiers_rating),
                &[Modifier { name, value: 0.5 }],
            );

            assert_eq!(speed_rating.pass, modified.pass, "{name}");
            assert_eq!(speed_rating.tech, modified.tech, "{name}");
            assert_eq!(speed_rating.acc, modified.acc, "{name}");
            assert_eq!(speed_rating.predicted_acc, modified.predicted_acc, "{name}");
            assert_eq!(
                speed_rating.slider_percentage, modified.slider_percentage,
                "{name}"
            );
            assert_eq!(Some(expected_note_span), modified.note_span, "{name}");
            assert_eq!(
                format!("{:.4}", expected_stars),
                format!("{:.4}", modified.stars.unwrap()),
                "{name}"
            );
            assert_eq!(
                format!("{:.4}", expected_stars),
                format!("{:.4}", calculate_stars(&modified)),
                "{name}"
            );
        }
    }

    #[test]
    fn it_properly_modifies_ratings_with_multiplier_modifiers() {
        let star_rating = rebalanced_star_rating();
        let modified = get_modified_ratings(
            &star_rating,
            Some(&modifiers_rating()),
            &[
                Modifier {
                    name: "SS",
                    value: -0.3,
                },
                Modifier {
                    name: "HD",
                    value: 0.05,
                },
                Modifier {
                    name: "SMC",
                    value: 0.07,
                },
            ],
        );

        let ss = modifiers_rating().ss.unwrap();
        assert_eq!(
            format!("{:.6}", ss.pass * 1.12),
            format!("{:.6}", modified.pass)
        );
        assert_eq!(
            format!("{:.6}", ss.tech * 1.12),
            format!("{:.6}", modified.tech)
        );
        assert_eq!(
            format!("{:.6}", ss.acc * 1.12),
            format!("{:.6}", modified.acc)
        );
        // stars are calculated from ratings not multiplied by modifiers
        assert_eq!(
            format!("{:.4}", 9.467636),
            format!("{:.4}", modified.stars.unwrap())
        );
        assert!(calculate_stars(&modified) > modified.stars.unwrap());
    }

    #[test]
    fn it_uses_speed_modifier_value_as_multiplier_without_modifiers_rating() {
        let star_rating = rebalanced_star_rating();
        let modified = get_modified_ratings(
            &star_rating,
            None,
            &[Modifier {
                name: "SS",
                value: -0.3,
            }],
        );

        assert_eq!(
            format!("{:.6}", star_rating.pass * 0.7),
            format!("{:.6}", modified.pass)
        );
        assert_eq!(
            format!("{:.6}", star_rating.tech * 0.7),
            format!("{:.6}", modified.tech)
        );
        assert_eq!(
            format!("{:.6}", star_rating.acc * 0.7),
            format!("{:.6}", modified.acc)
        );
        assert_eq!(star_rating.note_span, modified.note_span);
        assert_eq!(
            format!("{:.4}", 10.577722),
            format!("{:.4}", modified.stars.unwrap())
        );
    }

    // https://api.beatleader.com/leaderboard/44c1axxxx91 (map with bfs/bsf ratings)
    fn proof_star_rating() -> StarRating {
        StarRating {
            pass: 5.309791,
            tech: 5.081649,
            acc: 8.853389,
            predicted_acc: Some(0.98059666),
            slider_percentage: Some(-2.3606907e-8),
            notes: Some(1536),
            note_span: Some(246.70174),
            stars: None,
        }
    }

    fn proof_modifiers_rating() -> ModifiersRating {
        ModifiersRating {
            // bfs/bsf have no slider percentage, so map's one is used
            bfs: Some(SpeedModifierRating {
                pass: 6.4082704,
                tech: 5.5619855,
                acc: 10.040533,
                predicted_acc: Some(0.9778708),
                slider_percentage: None,
            }),
            bsf: Some(SpeedModifierRating {
                pass: 8.088186,
                tech: 6.098903,
                acc: 11.661868,
                predicted_acc: Some(0.9733665),
                slider_percentage: None,
            }),
            ..Default::default()
        }
    }

    // https://api.beatleader.com/leaderboard/4452axxxxxxxxxxxxxxx71 (map with MiddleSkip scores)
    fn zenithfall_star_rating() -> StarRating {
        StarRating {
            pass: 8.202506,
            tech: 5.896374,
            acc: 10.540052,
            predicted_acc: Some(0.9759485),
            slider_percentage: Some(0.002613274),
            notes: Some(2544),
            note_span: Some(348.91153),
            stars: None,
        }
    }

    fn zenithfall_modifiers_rating() -> ModifiersRating {
        let rating = |pass, tech, acc, predicted_acc, slider_percentage| {
            Some(SpeedModifierRating {
                pass,
                tech,
                acc,
                predicted_acc: Some(predicted_acc),
                slider_percentage,
            })
        };

        ModifiersRating {
            ss: rating(
                6.9249086,
                5.5265536,
                9.423863,
                0.97891676,
                Some(0.002613251),
            ),
            fs: rating(
                9.8309555,
                6.237442,
                11.997835,
                0.97125316,
                Some(0.0026132774),
            ),
            sf: rating(
                12.720523,
                6.9425135,
                14.115925,
                0.9634089,
                Some(0.0026132667),
            ),
            bfs: rating(9.8309555, 6.237442, 11.67785, 0.9724392, None),
            bsf: rating(12.225521, 6.5589747, 13.065017, 0.966669, None),
        }
    }

    #[test]
    fn it_properly_calculates_pp_from_acc_with_middle_skip() {
        // MS (MiddleSkip) has no value in difficulty.modifierValues, so it doesn't change pp
        let no_mods = get_modified_ratings(
            &zenithfall_star_rating(),
            Some(&zenithfall_modifiers_rating()),
            &[],
        );
        let middle_skip = get_modified_ratings(
            &zenithfall_star_rating(),
            Some(&zenithfall_modifiers_rating()),
            &[Modifier {
                name: "MS",
                value: 0.0,
            }],
        );

        // score from https://api.beatleader.com/leaderboard/4452axxxxxxxxxxxxxxx71
        // (server stores pp as f32)
        let pp = calculate_pp_from_acc(0.9699529, middle_skip, "Standard", false);
        assert!((pp - 551.4529).abs() < 0.001, "got: {pp}");
        assert_eq!(
            calculate_pp_from_acc(0.9699529, no_mods, "Standard", false),
            pp
        );
    }

    #[test]
    fn it_properly_modifies_ratings_with_bfs_and_bsf_modifiers() {
        let star_rating = proof_star_rating();
        let modifiers_rating = proof_modifiers_rating();
        let bfs = modifiers_rating.bfs.clone().unwrap();
        let bsf = modifiers_rating.bsf.clone().unwrap();

        assert_eq!(
            format!("{:.4}", 7.687937),
            format!("{:.4}", calculate_stars(&star_rating))
        );

        for (name, speed_rating, expected_stars, timescale) in
            [("BFS", &bfs, 8.699528, 1.2), ("BSF", &bsf, 10.223622, 1.5)]
        {
            let modified = get_modified_ratings(
                &star_rating,
                Some(&modifiers_rating),
                &[Modifier { name, value: 0.0 }],
            );

            assert_eq!(speed_rating.pass, modified.pass, "{name}");
            assert_eq!(speed_rating.tech, modified.tech, "{name}");
            assert_eq!(speed_rating.acc, modified.acc, "{name}");
            assert_eq!(speed_rating.predicted_acc, modified.predicted_acc, "{name}");
            assert_eq!(
                star_rating.slider_percentage, modified.slider_percentage,
                "{name}"
            );
            assert_eq!(
                star_rating.note_span.map(|span| span / timescale),
                modified.note_span,
                "{name}"
            );
            assert_eq!(
                format!("{:.4}", expected_stars),
                format!("{:.4}", calculate_stars(&modified)),
                "{name}"
            );
        }

        // unlike ss/fs/sf, bfs/bsf values are not excluded from multiplier
        let modified = get_modified_ratings(
            &star_rating,
            Some(&modifiers_rating),
            &[Modifier {
                name: "BFS",
                value: 0.1,
            }],
        );
        assert_eq!(
            format!("{:.6}", bfs.pass * 1.1),
            format!("{:.6}", modified.pass)
        );
    }

    #[test]
    fn it_properly_modifies_legacy_ratings() {
        let modified = get_modified_ratings(
            &legacy_star_rating(),
            Some(&modifiers_rating()),
            &[
                Modifier {
                    name: "fs",
                    value: 0.2,
                },
                Modifier {
                    name: "GN",
                    value: 0.04,
                },
            ],
        );

        let fs = modifiers_rating().fs.unwrap();
        assert_eq!(
            format!("{:.6}", fs.pass * 1.04),
            format!("{:.6}", modified.pass)
        );
        assert_eq!(
            format!("{:.6}", fs.tech * 1.04),
            format!("{:.6}", modified.tech)
        );
        assert_eq!(
            format!("{:.6}", fs.acc * 1.04),
            format!("{:.6}", modified.acc)
        );
        assert!(modified.note_span.is_none());
        assert!(modified.stars.is_none());
    }

    #[test]
    fn it_properly_calculates_pp_from_acc_for_modified_rebalanced_ratings() {
        // values calculated by the PP curve widget on beatleader.com
        // (modifiers, (pass, acc, tech), [(acc, pp)])
        type Case<'a> = (&'a [(&'a str, f64)], (f64, f64, f64), &'a [(f64, f64)]);
        let cases: &[Case] = &[
            (
                &[("SS", -0.3)],
                (7.44, 11.16, 4.96),
                &[(0.9, 356.365), (0.975, 604.333)],
            ),
            (&[("SF", 0.72)], (14.03, 15.95, 6.39), &[(0.9, 541.945)]),
            (&[("FS", 0.4)], (10.75, 13.66, 5.64), &[(0.9, 449.999)]),
            (
                &[("FS", 0.4), ("NB", -0.2), ("NO", -0.2)],
                (6.45, 8.19, 3.38),
                &[(0.9, 267.338)],
            ),
            (
                &[("SC", -0.5)],
                (4.45, 6.11, 2.64),
                &[(0.9, 195.181), (0.975, 323.764)],
            ),
        ];

        for (mods, (pass, acc, tech), expected) in cases {
            let modifiers: Vec<Modifier> = mods
                .iter()
                .map(|(name, value)| Modifier {
                    name,
                    value: *value,
                })
                .collect();
            let modified = get_modified_ratings(
                &rebalanced_star_rating(),
                Some(&modifiers_rating()),
                &modifiers,
            );

            assert_eq!(
                format!("{:.2}", pass),
                format!("{:.2}", modified.pass),
                "{mods:?}"
            );
            assert_eq!(
                format!("{:.2}", acc),
                format!("{:.2}", modified.acc),
                "{mods:?}"
            );
            assert_eq!(
                format!("{:.2}", tech),
                format!("{:.2}", modified.tech),
                "{mods:?}"
            );

            for (expected_acc, expected_pp) in *expected {
                let pp = calculate_pp_from_acc(*expected_acc, modified.clone(), "Standard", false);
                assert_eq!(
                    format!("{:.3}", expected_pp),
                    format!("{:.3}", pp),
                    "{mods:?} acc: {expected_acc}"
                );

                let acc_from_pp = calculate_acc_from_pp(*expected_pp, modified.clone(), "Standard");
                assert_eq!(
                    format!("{:.4}", expected_acc),
                    format!("{:.4}", acc_from_pp.unwrap()),
                    "{mods:?} pp: {expected_pp}"
                );
            }
        }
    }

    #[test]
    fn it_properly_calculates_pp_from_acc_for_low_star_map() {
        // stars from https://api.beatleader.com/leaderboard/ff0291
        assert_eq!(
            format!("{:.4}", 4.517261),
            format!("{:.4}", calculate_stars(&low_star_rating()))
        );

        for (acc, expected_pp) in LOW_STAR_EXPECTED {
            let pp = calculate_pp_from_acc(*acc, low_star_rating(), "Standard", false);
            assert_eq!(
                format!("{:.3}", expected_pp),
                format!("{:.3}", pp),
                "acc: {acc}"
            );

            let acc_from_pp = calculate_acc_from_pp(*expected_pp, low_star_rating(), "Standard");
            assert_eq!(
                format!("{:.4}", acc),
                format!("{:.4}", acc_from_pp.unwrap()),
                "pp: {expected_pp}"
            );
        }
    }

    #[test]
    fn it_properly_calculates_stars_for_modified_rebalanced_ratings() {
        // values shown by the PP curve widget on beatleader.com
        type Case<'a> = (&'a [(&'a str, f64)], f64);
        let cases: &[Case] = &[
            (&[], 10.58),
            (&[("SF", 0.72)], 14.78),
            (&[("FS", 0.4)], 12.12),
            (&[("SS", -0.3)], 9.47),
            (&[("FS", 0.4), ("NB", -0.2), ("NO", -0.2)], 7.33),
            (&[("SC", -0.5)], 5.62),
        ];

        for (mods, expected_stars) in cases {
            let modifiers: Vec<Modifier> = mods
                .iter()
                .map(|(name, value)| Modifier {
                    name,
                    value: *value,
                })
                .collect();
            let modified = get_modified_ratings(
                &rebalanced_star_rating(),
                Some(&modifiers_rating()),
                &modifiers,
            );

            assert_eq!(
                format!("{:.2}", expected_stars),
                format!("{:.2}", calculate_stars(&modified)),
                "{mods:?}"
            );
        }
    }
}
