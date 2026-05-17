use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;

use num_bigint::BigUint;
use regex_syntax::Parser;

use crate::calculate::{self, Amount};
use crate::cli::app;
use crate::error::{Error, Result};
use crate::generate;
use crate::model::Limits;

pub(crate) fn run() -> Result<()> {
    let m = app().get_matches();
    let pattern = m
        .get_one::<String>("pattern")
        .ok_or_else(|| Error::Message("missing pattern".to_string()))?;
    let hir = Parser::new().parse(pattern)?;
    let limits = Limits {
        min_len: *m.get_one::<usize>("min-len").unwrap_or(&0),
        max_len: m.get_one::<usize>("max-len").copied(),
    };
    if let Some(max) = limits.max_len {
        if limits.min_len > max {
            return Err(Error::Message(
                "--min-len cannot exceed --max-len".to_string(),
            ));
        }
    }

    let want_count = m.get_flag("count");
    let want_size = m.get_flag("size");
    let want_generate = m.get_flag("generate");

    if want_generate {
        let limit = m.get_one::<u64>("limit").copied();
        let max_total = m
            .get_one::<String>("max-total-bytes")
            .map(|s| parse_size(s))
            .transpose()?;
        let out = m.get_one::<PathBuf>("out").cloned();
        let generate_limits = generation_limits(&hir, &limits, limit);
        generate_output(&hir, &generate_limits, limit, max_total, out)?;
    }

    if !want_generate || want_count || want_size {
        let stats = calculate::analyze(&hir, &limits)?;
        if want_count && !want_size {
            println!("{}", stats.count.display());
        } else if want_size && !want_count {
            print_size(&stats.total_bytes);
        } else {
            println!("count: {}", stats.count.display());
            print!("total_bytes: ");
            print_size(&stats.total_bytes);
        }
    }

    Ok(())
}

fn generation_limits(hir: &regex_syntax::hir::Hir, limits: &Limits, limit: Option<u64>) -> Limits {
    if limits.max_len.is_some() || limit.is_none() {
        return limits.clone();
    }
    let min = hir.properties().minimum_len().unwrap_or(0);
    Limits {
        min_len: limits.min_len,
        max_len: Some(min.saturating_add(limit.unwrap() as usize)),
    }
}

fn generate_output(
    hir: &regex_syntax::hir::Hir,
    limits: &Limits,
    limit: Option<u64>,
    max_total: Option<BigUint>,
    out: Option<PathBuf>,
) -> Result<()> {
    let mut writer: Box<dyn Write> = match out {
        Some(path) => Box::new(BufWriter::new(File::create(path)?)),
        None => Box::new(BufWriter::new(io::stdout())),
    };
    let mut emitted = 0u64;
    let mut total = BigUint::from(0u8);

    generate::generate(hir, limits, |s| {
        if limit.is_some_and(|limit| emitted >= limit) {
            return Ok(false);
        }
        let next_total = &total + BigUint::from(s.len());
        if max_total.as_ref().is_some_and(|max| next_total > *max) {
            return Ok(false);
        }
        writeln!(writer, "{s}")?;
        emitted += 1;
        total = next_total;
        Ok(true)
    })?;
    writer.flush()?;
    Ok(())
}

fn print_size(amount: &Amount) {
    match amount {
        Amount::Finite(bytes) => println!("{} ({})", bytes, format_binary(bytes)),
        Amount::Infinite => println!("infinite"),
    }
}

fn format_binary(bytes: &BigUint) -> String {
    let units = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
    let base = BigUint::from(1024u16);
    let mut unit = 0;
    let mut value = bytes.clone();
    while value >= base && unit + 1 < units.len() {
        value /= &base;
        unit += 1;
    }
    format!("{value} {}", units[unit])
}

fn parse_size(input: &str) -> Result<BigUint> {
    let trimmed = input.trim();
    let split_at = trimmed
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(trimmed.len());
    let (digits, suffix) = trimmed.split_at(split_at);
    if digits.is_empty() {
        return Err(Error::Message(format!("invalid size: {input}")));
    }
    let mut value = digits
        .parse::<BigUint>()
        .map_err(|_| Error::Message(format!("invalid size: {input}")))?;
    let multiplier = match suffix.trim().to_ascii_lowercase().as_str() {
        "" | "b" => 1u32,
        "k" | "kb" | "kib" => 1024u32,
        "m" | "mb" | "mib" => 1024u32.pow(2),
        "g" | "gb" | "gib" => 1024u32.pow(3),
        _ => return Err(Error::Message(format!("invalid size unit: {suffix}"))),
    };
    value *= multiplier;
    Ok(value)
}
