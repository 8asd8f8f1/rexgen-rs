use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::PathBuf;

use num_bigint::BigUint;
use regex_syntax::Parser;

use crate::calculate::{self, Amount};
use crate::cli;
use crate::error::{Error, Result};
use crate::generate;
use crate::model::{ByteSize, Limits};

pub(crate) fn run() -> Result<()> {
    let m = cli::app().get_matches();
    if let Some(("completions", sub_m)) = m.subcommand() {
        let shell = *sub_m
            .get_one::<clap_complete::Shell>("shell")
            .ok_or_else(|| Error::Message("missing shell".to_string()))?;
        let mut cmd = cli::app();
        clap_complete::generate(shell, &mut cmd, "rexgen", &mut io::stdout());
        return Ok(());
    }

    let pattern = m
        .get_one::<String>("pattern")
        .ok_or_else(|| Error::Message("missing pattern".to_string()))?;
    let hir = Parser::new().parse(pattern)?;
    let limits = Limits {
        min_len: m.get_one::<ByteSize>("min-len").map_or(0, |size| size.0),
        max_len: m.get_one::<ByteSize>("max-len").map(|size| size.0),
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
            .get_one::<ByteSize>("max-total-bytes")
            .map(|size| BigUint::from(size.0));
        let out = m.get_one::<PathBuf>("out").cloned();
        let generate_limits = generation_limits(&hir, &limits, limit);
        if !m.get_flag("yes")
            && !confirm_generation(pattern, &generate_limits, limit, &max_total, out.as_ref())?
        {
            eprintln!("generation aborted");
            return Ok(());
        }
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

fn confirm_generation(
    pattern: &str,
    limits: &Limits,
    limit: Option<u64>,
    max_total: &Option<BigUint>,
    out: Option<&PathBuf>,
) -> Result<bool> {
    eprintln!("Generate matching strings?");
    eprintln!("pattern: {pattern}");
    eprintln!("min length: {} bytes", limits.min_len);
    if let Some(max_len) = limits.max_len {
        eprintln!("max length: {max_len} bytes");
    }
    if let Some(limit) = limit {
        eprintln!("string limit: {limit}");
    }
    if let Some(max_total) = max_total {
        eprintln!("total byte limit: {max_total}");
    }
    if let Some(out) = out {
        eprintln!("output: {}", out.display());
    } else {
        eprintln!("output: stdout");
    }
    eprint!("Continue? [y/N] ");
    io::stderr().flush()?;

    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
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
