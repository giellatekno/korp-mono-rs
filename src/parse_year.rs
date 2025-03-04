// anders: admittedly this went a little over-board....

/// parse the <year> tag of the analysed xml into the
/// (date, datefrom, dateto) fields expected in the korp_mono format
///
/// The string in the year tag can be of 3 forms:
///    YYYY
///    YYYY-YYYY
///    DD.MM.YYYY
///
/// The output is the form (yyyy-mm-dd, yyyymmdd, yyyymmdd).
/// The first is always the first date, with mm-dd being 01-01 if
/// unknown.
/// If a year range is given, the output is
///     (aaaa-01-01, aaaa0101, bbbb0101),
///     where aaaa is the first year, and bbbb is the last year.
pub fn parse_year(year: Option<&str>) -> (String, String, String) {
    macro_rules! zero_output {
        () => {
            (
                "0000-00-00".to_string(),
                "00000000".to_string(),
                "00000000".to_string(),
            )
        };
    }
    #[inline(always)]
    fn is_digit(b: &u8) -> bool {
        const ZERO: u8 = '0' as u8;
        const NINE: u8 = '9' as u8;
        matches!(b, ZERO..=NINE)
    }

    macro_rules! are_digits {
        ($b:expr) => {
            is_digit($b)
        };
        ($b:expr, $($bs:expr),+) => {
            are_digits!($b) &&
            are_digits!($($bs),*)
        };
    }
    const DOT: u8 = '.' as u8;
    const DASH: u8 = '-' as u8;
    match year {
        None => zero_output!(),
        Some(year) => {
            match year.as_bytes() {
                // yyyy
                [a, b, c, d] if are_digits!(a, b, c, d) => {
                    (
                        format!("{year}-01-01"),
                        format!("{year}0101"),
                        format!("{year}0101"),
                    )
                }
                // yyyy-yyyy
                [a, b, c, d, DASH, e, f, g, h] if are_digits!(a, b, c, d, e, f, g, h) => {
                    let year_from = &year[0..4];
                    (
                        format!("{year_from}-01-01"),
                        format!("{year_from}0101"),
                        format!("{}0101", &year[5..9]),
                    )
                }
                // (mm|dd).(mm|dd).yyyy
                [a, b, DOT, c, d, DOT, e, f, g, h] if are_digits!(a, b, c, d, e, f, g, h) => {
                    let dd = &year[0..2];
                    let mm = &year[3..5];
                    let year = &year[6..10];
                    // SAFETY: We know the slices consists only of digits,
                    // so parsing them to u8's is ok
                    let ddu: u8 = unsafe { dd.parse().unwrap_unchecked() };
                    let mmu: u8 = unsafe { mm.parse().unwrap_unchecked() };
                    match (ddu, mmu) {
                        // "00" as day or month is invalid
                        (0, _) | (_, 0) => zero_output!(), 
                        (1..=12, 1..=12) => {
                            // both between 01 and 12, so assume the sane
                            // choice of dd.mm.yyyy (sorry freedom-lovers)
                            (
                                format!("{year}-{mm}-{dd}"),
                                format!("{year}{mm}{dd}"),
                                format!("{year}{mm}{dd}"),
                            )
                        }
                        (1..=12, 1..=31) => {
                            (
                                format!("{year}-{dd}-{mm}"),
                                format!("{year}{dd}{mm}"),
                                format!("{year}{dd}{mm}"),
                            )
                        }
                        (1..=31, 1..=12) => {
                            (
                                format!("{year}-{mm}-{dd}"),
                                format!("{year}{mm}{dd}"),
                                format!("{year}{mm}{dd}"),
                            )
                        }
                        (_, _) => {
                            // invalid date: both over 12, or otherwise
                            zero_output!()
                        }
                    }
                }
                _=> zero_output!(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse_year;

    #[test]
    fn test_none() {
        let (a, b, c) = parse_year(None);
        assert_eq!(a.as_str(), "0000-00-00");
        assert_eq!(b.as_str(), "00000000");
        assert_eq!(c.as_str(), "00000000");
    }

    #[test]
    fn accepted() {
        [
            ("1998", "1998-01-01", "19980101", "19980101"),
            ("1998-2010", "1998-01-01", "19980101", "20100101"),
            ("02.02.2025", "2025-02-02", "20250202", "20250202"),
            ("15.02.2025", "2025-02-15", "20250215", "20250215"),
            ("02.15.2025", "2025-02-15", "20250215", "20250215"),
        ].iter().for_each(|(input, out1, out2, out3)| {
            let out = parse_year(Some(input));
            assert_eq!(
                (out.0.as_str(), out.1.as_str(), out.2.as_str()),
                (*out1, *out2, *out3)
            );
        });
    }

    #[test]
    fn denied() {
        [
            "unknown",
            "999",
            "10000",
            "10001",
            "2000-10000",
            "999-2000",
            "05.32.2000",
            "32.05.2000",
            "06.06.999"
        ].iter().for_each(|input| {
            let out = parse_year(Some(input));
            assert_eq!(
                (out.0.as_str(), out.1.as_str(), out.2.as_str()),
                ("0000-00-00", "00000000", "00000000")
            );
        });
    }
}
