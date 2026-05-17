use std::fs::File;
use std::io::BufWriter;
use std::io::Write;
use std::io::{self};
use std::path::PathBuf;

use num_bigint::BigUint;

use crate::calculate::Amount;
use crate::cli;
use crate::corpus::Corpus;
use crate::corpus::GenerationRequest;
use crate::corpus::LengthConstraints;
use crate::error::Error;
use crate::error::Result;
use crate::generate::GenerationOrder;
use crate::model::ByteSize;

pub(crate) fn run() -> Result<()> {
    let m = cli::app().get_matches();
    if let Some(threads) = m.get_one::<usize>("threads") {
        rayon::ThreadPoolBuilder::new()
            .num_threads(*threads)
            .build_global()
            .map_err(|err| {
                Error::Message(format!(
                    "failed to configure thread pool: {err}"
                ))
            })?;
    }

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
    let constraints = LengthConstraints {
        min: m.get_one::<ByteSize>("min-len").map_or(0, |size| size.0),
        max: m.get_one::<ByteSize>("max-len").map(|size| size.0),
    };
    let corpus = Corpus::new(pattern, constraints)?;

    let want_count = m.get_flag("count");
    let want_size = m.get_flag("size");
    let want_generate = m.get_flag("generate");

    if want_generate {
        let request = GenerationRequest {
            limit: m.get_one::<u64>("limit").copied(),
            max_total_bytes: m
                .get_one::<ByteSize>("max-total-bytes")
                .map(|size| BigUint::from(size.0)),
            start_string: m.get_one::<String>("start-string").cloned(),
            stop_string: m.get_one::<String>("stop-string").cloned(),
            reverse_strings: m.get_flag("reverse-strings"),
            order: if m.get_flag("invert-order") {
                GenerationOrder::Inverted
            } else if m.get_flag("ordered")
                || m.get_one::<String>("start-string").is_some()
                || m.get_one::<String>("stop-string").is_some()
            {
                GenerationOrder::Default
            } else {
                GenerationOrder::Unordered
            },
        };
        corpus.validate_generation_request(&request)?;
        let out = m.get_one::<PathBuf>("out").cloned();
        if !m.get_flag("yes")
            && !confirm_generation(
                pattern,
                corpus.constraints(),
                &request,
                out.as_ref(),
            )?
        {
            eprintln!("generation aborted");
            return Ok(());
        }
        generate_output(&corpus, request, out)?;
    }

    if !want_generate || want_count || want_size {
        let stats = corpus.analyze()?;
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
    constraints: &LengthConstraints,
    request: &GenerationRequest,
    out: Option<&PathBuf>,
) -> Result<bool> {
    eprintln!("Generate matching strings?");
    eprintln!("pattern: {pattern}");
    eprintln!("min length: {} bytes", constraints.min);
    if let Some(max_len) = constraints.max {
        eprintln!("max length: {max_len} bytes");
    }
    if let Some(limit) = request.limit {
        eprintln!("string limit: {limit}");
    }
    if let Some(max_total) = &request.max_total_bytes {
        eprintln!("total byte limit: {max_total}");
    }
    if let Some(start) = &request.start_string {
        eprintln!("start string: {start}");
    }
    if let Some(stop) = &request.stop_string {
        eprintln!("stop string: {stop}");
    }
    if request.reverse_strings {
        eprintln!("reverse strings: yes");
    }
    if request.order == GenerationOrder::Inverted {
        eprintln!("order: inverted");
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

fn generate_output(
    corpus: &Corpus,
    request: GenerationRequest,
    out: Option<PathBuf>,
) -> Result<()> {
    match out {
        Some(path) => {
            let mut writer = BufWriter::new(File::create(path)?);
            corpus.generate_file(request, &mut writer)?;
            writer.flush()?;
        }
        None => {
            let mut writer = BufWriter::new(io::stdout());
            corpus.generate_file(request, &mut writer)?;
            writer.flush()?;
        }
    }
    Ok(())
}

fn print_size(amount: &Amount) {
    match amount {
        Amount::Finite(bytes) => {
            println!("{} ({})", bytes, format_binary(bytes))
        }
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
