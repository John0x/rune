//! Number labels for ticks: as many decimals as the tick step needs, optionally compact
//! (`1.2k`, `3.4M`).

/// Decimal places needed to write every multiple of `step` exactly: 0 for 1 or 50, 1 for 0.5 or
/// 0.2, 2 for 0.25 or 0.01. At most 15; 0 for a zero or non-finite step.
pub fn precision(step: f64) -> usize {
    let step = step.abs();
    if !(step.is_finite() && step > 0.0) {
        return 0;
    }
    let mut digits = (-step.log10().floor()).max(0.0) as usize;
    while digits < 15 {
        let scaled = step * 10f64.powi(digits as i32);
        if (scaled - scaled.round()).abs() <= 1e-9 * scaled.max(1.0) {
            break;
        }
        digits += 1;
    }
    digits.min(15)
}

/// Writes `value` with the decimals its tick `step` needs: `format_number(2.5, 0.5)` is `2.5`,
/// `format_number(2.0, 0.5)` is `2.0`, `format_number(1200.0, 200.0)` is `1200`.
///
/// Without a usable step (zero or not finite) the value is written with up to six decimals and
/// trailing zeros dropped. Never writes `-0`.
pub fn format_number(value: f64, step: f64) -> String {
    if !value.is_finite() {
        return format!("{value}");
    }
    let s = if usable(step) { format!("{:.*}", precision(step), value) } else { trim_zeros(format!("{value:.6}")) };
    no_negative_zero(s)
}

/// Like [`format_number`], with thousands as `k`, millions as `M`, billions as `G` and trillions
/// as `T`, and trailing zeros dropped: `1500` with a step of 500 is `1.5k`, `2000000` is `2M`.
///
/// The value is first rounded to the step, so `999950` with a step of 100 reads `1M`, not
/// `1000.0k`. Values under a thousand have no suffix.
pub fn format_compact(value: f64, step: f64) -> String {
    const UNITS: [(f64, &str); 4] = [(1e12, "T"), (1e9, "G"), (1e6, "M"), (1e3, "k")];
    if !value.is_finite() {
        return format!("{value}");
    }
    let step = step.abs();
    let value = if usable(step) { (value / step).round() * step } else { value };
    let (unit, suffix) = UNITS.iter().copied().find(|(u, _)| value.abs() >= *u).unwrap_or((1.0, ""));
    let digits = if usable(step) { precision(step / unit) } else { 6 };
    let number = no_negative_zero(trim_zeros(format!("{:.*}", digits, value / unit)));
    format!("{number}{suffix}")
}

fn usable(step: f64) -> bool {
    step.is_finite() && step != 0.0
}

fn trim_zeros(s: String) -> String {
    if s.contains('.') { s.trim_end_matches('0').trim_end_matches('.').to_owned() } else { s }
}

fn no_negative_zero(s: String) -> String {
    match s.strip_prefix('-') {
        Some(rest) if rest.chars().all(|c| c == '0' || c == '.') => rest.to_owned(),
        _ => s,
    }
}

#[cfg(test)]
#[path = "format_tests.rs"]
mod tests;
