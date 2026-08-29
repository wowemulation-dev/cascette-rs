//! Build script for cascette-agent.
//!
//! Sets `CASCETTE_BUILD_DATE` so the `/version` endpoint can report
//! when this binary was compiled.

fn main() {
    // Emit build date as RFC 3339 (UTC, date only).
    // Using a simple approach that works without external crates.
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Convert epoch seconds to a YYYY-MM-DD string.
    // 86400 seconds per day, epoch starts 1970-01-01.
    let days = now / 86400;
    let (year, month, day) = epoch_days_to_date(days);

    let date_str = format!("{year:04}-{month:02}-{day:02}");
    println!("cargo:rustc-env=CASCETTE_BUILD_DATE={date_str}");

    // Only rerun if build.rs itself changes (not on every source change).
    println!("cargo:rerun-if-changed=build.rs");
}

/// Convert days since Unix epoch to (year, month, day).
fn epoch_days_to_date(days: u64) -> (u64, u64, u64) {
    // Civil calendar algorithm from Howard Hinnant.
    let z = days + 719_468;
    let era = z / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
