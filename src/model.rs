use std::str::FromStr;

#[derive(Debug, Clone)]
pub(crate) struct Limits {
    pub min_len: usize,
    pub max_len: Option<usize>,
}

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

        let value = digits
            .parse::<usize>()
            .map_err(|_| format!("byte size too large: {input}"))?;
        let unit = normalize_unit(unit);
        let multiplier =
            unit_multiplier(&unit).ok_or_else(|| format!("unrecognized byte unit: {unit}"))?;
        value
            .checked_mul(multiplier)
            .map(ByteSize)
            .ok_or_else(|| format!("byte size too large: {input}"))
    }
}

fn normalize_unit(unit: &str) -> String {
    unit.chars()
        .filter(|ch| ch.is_ascii_alphabetic())
        .flat_map(char::to_lowercase)
        .collect()
}

fn unit_multiplier(unit: &str) -> Option<usize> {
    match unit {
        "" | "b" | "byte" | "bytes" => Some(1),
        "k" | "kb" | "kilo" | "kilos" | "kilobyte" | "kilobytes" => Some(1_000),
        "m" | "mb" | "mega" | "megas" | "megabyte" | "megabytes" => Some(1_000_000),
        "g" | "gb" | "giga" | "gigas" | "gigabyte" | "gigabytes" => Some(1_000_000_000),
        "t" | "tb" | "tera" | "teras" | "terabyte" | "terabytes" => Some(1_000_000_000_000),
        "ki" | "kib" | "kibi" | "kibibyte" | "kibibytes" => Some(1024),
        "mi" | "mib" | "mebi" | "mebibyte" | "mebibytes" => Some(1024usize.pow(2)),
        "gi" | "gib" | "gibi" | "gibibyte" | "gibibytes" => Some(1024usize.pow(3)),
        "ti" | "tib" | "tebi" | "tebibyte" | "tebibytes" => Some(1024usize.pow(4)),
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
        assert_eq!(parse("2 MB"), 2_000_000);
        assert_eq!(parse("3 gigabytes"), 3_000_000_000);
    }

    #[test]
    fn parses_iec_units() {
        assert_eq!(parse("1kib"), 1024);
        assert_eq!(parse("2 MiB"), 2 * 1024 * 1024);
        assert_eq!(parse("3 gibibytes"), 3 * 1024 * 1024 * 1024);
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
    }
}
