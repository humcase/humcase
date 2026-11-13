//! Calendar day validation.
//!
//! A Humcase due date is a **day** (`YYYY-MM-DD`), not an instant. Validation must therefore
//! check a real calendar date: `2026-02-31` or `2026-04-31` do not exist and must never be
//! accepted (ticket "Due dates: the validator accepts a nonexistent date").

/// True if `value` is a valid calendar day in the strict `YYYY-MM-DD` format.
///
/// Checks the shape (10 characters, dashes at positions 4 and 7, digits elsewhere) **and** the
/// real validity of the date: month 1-12, day within the month, leap years included.
pub fn is_iso_day(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    if !bytes
        .iter()
        .enumerate()
        .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
    {
        return false;
    }

    let year: i32 = value[0..4].parse().unwrap_or(-1);
    let month: u32 = value[5..7].parse().unwrap_or(0);
    let day: u32 = value[8..10].parse().unwrap_or(0);

    let leap_year = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap_year {
                29
            } else {
                28
            }
        }
        _ => return false,
    };

    (1..=days_in_month).contains(&day)
}

#[cfg(test)]
mod tests {
    use super::is_iso_day;

    #[test]
    fn accepts_real_dates() {
        assert!(is_iso_day("2026-12-31"));
        assert!(is_iso_day("2026-01-01"));
        assert!(is_iso_day("2028-02-29"), "2028 is a leap year");
        assert!(
            is_iso_day("2000-02-29"),
            "2000 is a leap year (divisible by 400)"
        );
    }

    #[test]
    fn rejects_nonexistent_dates() {
        assert!(!is_iso_day("2026-02-31"));
        assert!(!is_iso_day("2026-04-31"));
        assert!(!is_iso_day("2026-02-29"), "2026 is not a leap year");
        assert!(
            !is_iso_day("1900-02-29"),
            "1900 is not a leap year (divisible by 100)"
        );
        assert!(!is_iso_day("2026-13-01"));
        assert!(!is_iso_day("2026-00-10"));
        assert!(!is_iso_day("2026-01-00"));
        assert!(!is_iso_day("2026-1-01"));
    }

    #[test]
    fn rejects_invalid_shapes() {
        assert!(!is_iso_day(""));
        assert!(!is_iso_day("31/12/2026"));
        assert!(!is_iso_day("2026-12-31T00:00:00"));
        assert!(!is_iso_day("2026_12_31"));
        assert!(!is_iso_day("202a-12-31"));
    }
}
