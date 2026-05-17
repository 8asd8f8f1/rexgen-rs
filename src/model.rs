use std::str::FromStr;

use byte_unit::Byte;
use byte_unit::Unit;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ByteSize(pub usize);

impl FromStr for ByteSize {
    type Err = String;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Err("byte size cannot be empty".to_string());
        }

        let split_at = trimmed
            .find(|ch: char| !ch.is_ascii_digit())
            .unwrap_or(trimmed.len());
        let (digits, unit) = trimmed.split_at(split_at);
        if digits.is_empty() {
            return Err(format!("invalid byte size: {input}"));
        }

        if !unit
            .chars()
            .all(|ch| ch.is_ascii_alphabetic() || ch.is_ascii_whitespace())
        {
            return Err(format!("invalid byte size: {input}"));
        }

        let value = digits
            .parse::<u128>()
            .map_err(|_| format!("byte size too large: {input}"))?;
        let normalized_unit = normalize_unit(unit);
        let unit = parse_unit(&normalized_unit).ok_or_else(|| {
            format!("unrecognized byte unit: {normalized_unit}")
        })?;
        let bytes = Byte::from_u128_with_unit(value, unit)
            .ok_or_else(|| format!("byte size too large: {input}"))?
            .as_u128();
        usize::try_from(bytes)
            .map(ByteSize)
            .map_err(|_| format!("byte size too large: {input}"))
    }
}

fn normalize_unit(unit: &str) -> String {
    unit.chars()
        .filter(|ch| ch.is_ascii_alphabetic())
        .flat_map(char::to_lowercase)
        .collect()
}

fn parse_unit(unit: &str) -> Option<Unit> {
    match unit {
        "" | "b" | "byte" | "bytes" => Some(Unit::B),
        "k" | "kb" | "kilo" | "kilos" | "kilobyte" | "kilobytes" => {
            Some(Unit::KB)
        }
        "m" | "mb" | "mega" | "megas" | "megabyte" | "megabytes" => {
            Some(Unit::MB)
        }
        "g" | "gb" | "giga" | "gigas" | "gigabyte" | "gigabytes" => {
            Some(Unit::GB)
        }
        "t" | "tb" | "tera" | "teras" | "terabyte" | "terabytes" => {
            Some(Unit::TB)
        }
        "ki" | "kib" | "kibi" | "kibibyte" | "kibibytes" => Some(Unit::KiB),
        "mi" | "mib" | "mebi" | "mebibyte" | "mebibytes" => Some(Unit::MiB),
        "gi" | "gib" | "gibi" | "gibibyte" | "gibibytes" => Some(Unit::GiB),
        "ti" | "tib" | "tebi" | "tebibyte" | "tebibytes" => Some(Unit::TiB),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::ByteSize;

    fn parse(input: &str) -> usize {
        input.parse::<ByteSize>().unwrap().0
    }

    #[test]
    fn parses_bytes() {
        assert_eq!(parse("10"), 10);
        assert_eq!(parse("10b"), 10);
        assert_eq!(parse("10 bytes"), 10);
    }

    #[test]
    fn parses_si_units() {
        assert_eq!(parse("1kb"), 1_000);
        assert_eq!(parse("1 kilo"), 1_000);
        assert_eq!(parse("1 kilobyte"), 1_000);
        assert_eq!(parse("1 kilos"), 1_000);
        assert_eq!(parse("2 MB"), 2_000_000);
        assert_eq!(parse("3 gigabytes"), 3_000_000_000);
    }

    #[test]
    fn parses_iec_units() {
        assert_eq!(parse("1kib"), 1024);
        assert_eq!(parse("1 kibi"), 1024);
        assert_eq!(parse("2 MiB"), 2 * 1024 * 1024);
        assert_eq!(parse("2 mebi"), 2 * 1024 * 1024);
        assert_eq!(parse("2 mebibyte"), 2 * 1024 * 1024);
        assert_eq!(parse("3 gibibytes"), 3 * 1024 * 1024 * 1024);
    }

    #[test]
    fn rejects_fractional_and_signed_inputs() {
        assert!("1.5MB".parse::<ByteSize>().is_err());
        assert!(".5MB".parse::<ByteSize>().is_err());
        assert!("-1MB".parse::<ByteSize>().is_err());
    }

    #[test]
    fn rejects_unknown_units() {
        let err = "1qq".parse::<ByteSize>().unwrap_err();
        assert!(err.contains("unrecognized byte unit"));
    }

    #[test]
    fn rejects_overflow() {
        let err = "999999999999999999999999999999999999999999999999999999999999999999999999tb"
            .parse::<ByteSize>()
            .unwrap_err();
        assert!(err.contains("byte size too large"));

        let err = "99999999999999999999tb".parse::<ByteSize>().unwrap_err();
        assert!(err.contains("byte size too large"));
    }
}
