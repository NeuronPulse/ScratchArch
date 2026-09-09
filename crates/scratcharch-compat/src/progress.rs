//! Character progress bars.
//!
//! Every percentage in the compatibility dashboard is rendered as a fixed-width
//! character bar via the single shared helper [`format_progress_bar`] — never
//! re-implemented per CLI call-site. The default is the Unicode block bar; an
//! ASCII variant ([`format_progress_bar_ascii`]) is provided for plain-text
//! terminals / CI logs.
//!
//! ## Cell-count rule
//!
//! The bar is fixed at `width` cells (20 in the dashboard). The filled count is
//! the nearest-percent rounding of `percent * width / 100` (ties round up),
//! clamped so that:
//!
//! * 0%      → `[░░░░░░░░░░░░░░░░░░░░]` (all empty)
//! * 1..99%  → at least one filled and at least one empty cell
//! * 100%    → `[████████████████████]` (all filled)
//!
//! The two guards keep a near-empty run from reading as 0% and a near-full run
//! from reading as 100% — only 0% is empty and only 100% is full.

/// Round `numer / denom` to a whole percentage in `0..=100` (0 when `denom` is
/// 0). Nearest-percent, ties away from zero on the ratio.
pub fn percent(numer: usize, denom: usize) -> u32 {
    if denom == 0 {
        return 0;
    }
    let frac = numer as f64 / denom as f64;
    let p = (frac * 100.0).round();
    p.clamp(0.0, 100.0) as u32
}

/// Filled-cell count for `percent` on a `width`-cell bar (see module docs).
fn filled_cells(percent: u32, width: usize) -> usize {
    if width == 0 {
        return 0;
    }
    let p = percent.min(100);
    if p == 0 {
        return 0;
    }
    if p == 100 {
        return width;
    }
    // Nearest rounding of percent*width/100, ties round up.
    let approx = (p as usize * width + 50) / 100;
    approx.clamp(1, width.saturating_sub(1))
}

/// Build a fixed-`width` character progress bar for `percent`.
///
/// Returns the bracketed bar only (no percentage text), so callers compose the
/// `NN%  [bar]` line themselves. Uses Unicode block glyphs (`█` filled,
/// `░` empty).
///
/// ```
/// use scratcharch_compat::progress::format_progress_bar;
/// assert_eq!(format_progress_bar(0, 20), "[░░░░░░░░░░░░░░░░░░░░]");
/// assert_eq!(format_progress_bar(100, 20), "[████████████████████]");
/// assert_eq!(format_progress_bar(50, 20), "[██████████░░░░░░░░░░]");
/// ```
pub fn format_progress_bar(percent: u32, width: usize) -> String {
    format_progress_bar_with(percent, width, true)
}

/// ASCII variant of [`format_progress_bar`] (`#` filled, `.` empty) for
/// terminals without Unicode support.
pub fn format_progress_bar_ascii(percent: u32, width: usize) -> String {
    format_progress_bar_with(percent, width, false)
}

fn format_progress_bar_with(percent: u32, width: usize, unicode: bool) -> String {
    let (fill_c, empty_c) = if unicode {
        ('█', '░')
    } else {
        ('#', '.')
    };
    let filled = filled_cells(percent, width);
    let mut out = String::with_capacity(width + 2);
    out.push('[');
    for _ in 0..filled {
        out.push(fill_c);
    }
    for _ in filled..width {
        out.push(empty_c);
    }
    out.push(']');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bar_len(bar: &str) -> usize {
        // Brackets plus `width` cells.
        bar.chars().count()
    }

    fn filled_count(bar: &str) -> usize {
        let inner = &bar[1..bar.len() - 1];
        inner.chars().filter(|&c| c == '█').count()
    }

    #[test]
    fn fixed_twenty_cells() {
        for p in [0u32, 1, 50, 94, 99, 100] {
            let bar = format_progress_bar(p, 20);
            assert_eq!(bar_len(&bar), 22, "bar for {p}% must be 20 cells + brackets");
            assert_eq!(bar.chars().next(), Some('['));
            assert_eq!(bar.chars().last(), Some(']'));
        }
    }

    #[test]
    fn extremes_are_unique() {
        assert_eq!(format_progress_bar(0, 20), "[░░░░░░░░░░░░░░░░░░░░]");
        assert_eq!(filled_count(&format_progress_bar(0, 20)), 0);
        assert_eq!(format_progress_bar(100, 20), "[████████████████████]");
        assert_eq!(filled_count(&format_progress_bar(100, 20)), 20);
    }

    #[test]
    fn one_percent_is_not_empty() {
        // A 1% run must never read as 0%: at least one filled cell.
        let bar = format_progress_bar(1, 20);
        assert_eq!(filled_count(&bar), 1);
    }

    #[test]
    fn ninety_nine_percent_is_not_full() {
        // A 99% run must never read as 100%: at least one empty cell remains.
        let bar = format_progress_bar(99, 20);
        assert_eq!(filled_count(&bar), 19);
    }

    #[test]
    fn fifty_percent_is_half() {
        assert_eq!(filled_count(&format_progress_bar(50, 20)), 10);
    }

    #[test]
    fn nearest_rounding() {
        // 94% of 20 = 18.8 -> 19; 84% of 20 = 16.8 -> 17; 91% = 18.2 -> 18.
        assert_eq!(filled_count(&format_progress_bar(94, 20)), 19);
        assert_eq!(filled_count(&format_progress_bar(84, 20)), 17);
        assert_eq!(filled_count(&format_progress_bar(91, 20)), 18);
    }

    #[test]
    fn arbitrary_width_scales() {
        assert_eq!(filled_count(&format_progress_bar(100, 5)), 5);
        assert_eq!(filled_count(&format_progress_bar(0, 5)), 0);
        // 50% of 5 = 2.5 cells; ties round up -> 3.
        assert_eq!(filled_count(&format_progress_bar(50, 5)), 3);
    }

    #[test]
    fn ascii_variant() {
        assert_eq!(format_progress_bar_ascii(0, 20), "[....................]");
        assert_eq!(format_progress_bar_ascii(100, 20), "[####################]");
        assert_eq!(format_progress_bar_ascii(50, 20), "[##########..........]");
    }

    #[test]
    fn percent_rounding() {
        assert_eq!(percent(0, 10), 0);
        assert_eq!(percent(10, 10), 100);
        assert_eq!(percent(0, 0), 0);
        assert_eq!(percent(1, 3), 33); // 33.33 -> 33
        assert_eq!(percent(2, 3), 67); // 66.67 -> 67
        assert_eq!(percent(3, 4), 75);
    }
}
