/// Parse the `<year>` tag of the analysed xml into the (date, datefrom, dateto)
/// fields expected in the korp_mono format.
///
/// Three forms of `<year>` strings are recognized: `"YYYY"`, `"YYYY-YYYY"`, and
/// `"AA.BB.YYYY"`. In the latter form, which of `AA` and `BB` is the month is not
/// neccesarily known, but it will be parsed as expected when it is unambigous, or
/// sanely when not (that is, if both `AA` and `BB` are between 1 and 12, it is
/// recognized as `DD.MM.YYYY`.
///
/// The output form is (`YYYY-MM-DD`, `YYYYMMDD`, `YYYYMMDD`). The first is always
/// the first date, with `MM-DD` being `01-01` if unknown. If a year range is given,
/// the output is (`AAAA-01-01`, `AAAA0101`, `BBBB0101`), where `AAAA` is the first year,
/// and `BBBB` is the last year.
pub fn parse_year(year: Option<&str>) -> (String, String, String) {
    fn output(year: &str, month: &str, day: &str) -> (String, String, String) {
        (
            format!("{year}-{month}-{day}"),
            format!("{year}{month}{day}"),
            format!("{year}{month}{day}"),
        )
    }

    fn zero_output() -> (String, String, String) {
        output("0000", "00", "00")
    }

    let Some(year) = year else {
        return zero_output();
    };

    const DOT: u8 = b'.';
    const DASH: u8 = b'-';

    macro_rules! d {
        () => {
            b'0'..=b'9'
        };
    }

    match year.as_bytes() {
        // yyyy
        [d!(), d!(), d!(), d!()] => output(year, "01", "01"),
        // yyyy-yyyy
        [d!(), d!(), d!(), d!(), DASH, d!(), d!(), d!(), d!()] => {
            let year_from = &year[0..4];
            (
                format!("{year_from}-01-01"),
                format!("{year_from}0101"),
                format!("{}0101", &year[5..9]),
            )
        }
        // (mm|dd).(mm|dd).yyyy
        [d!(), d!(), DOT, d!(), d!(), DOT, d!(), d!(), d!(), d!()] => {
            let dd = &year[0..2];
            let mm = &year[3..5];
            let year = &year[6..10];
            // SAFETY: Slices consists only of digits, so parsing them as u8 is ok
            let ddu: u8 = unsafe { dd.parse().unwrap_unchecked() };
            let mmu: u8 = unsafe { mm.parse().unwrap_unchecked() };
            match (ddu, mmu) {
                (0, _) | (_, 0) => zero_output(), // 0 as day or month is invalid
                (1..=12, 1..=12) => output(year, mm, dd), // both in 01..=12, assume sane choice of dd.mm.yyyy
                (1..=12, 1..=31) => output(year, dd, mm),
                (1..=31, 1..=12) => output(year, mm, dd),
                (_, _) => zero_output(), // invalid: both over 12
            }
        }
        // unrecognized format
        _ => zero_output(),
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
        ]
        .iter()
        .for_each(|(input, out1, out2, out3)| {
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
            "",
            "?",
            "unknown",
            "999",
            "10000",
            "10001",
            "2000-10000",
            "999-2000",
            "05.32.2000",
            "32.05.2000",
            "32.32.2015",
            "06.06.999",
        ]
        .iter()
        .for_each(|input| {
            let out = parse_year(Some(input));
            assert_eq!(
                (out.0.as_str(), out.1.as_str(), out.2.as_str()),
                ("0000-00-00", "00000000", "00000000")
            );
        });
    }
}
