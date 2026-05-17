use clap::{Arg, ArgAction, Command, builder::ValueParser};
use clap_complete::Shell;

use crate::model::ByteSize;

pub(crate) fn app() -> Command {
    Command::new("rexgen")
        .bin_name("rexgen")
        .about("Count and generate strings matched by a regex")
        .long_about(
            "rexgen counts how many UTF-8 strings a Rust regex can match, estimates total corpus \
             size in bytes, and can generate matching strings in deterministic regex traversal order.",
        )
        .subcommand_negates_reqs(true)
        .after_help(
            "Examples:\n  rexgen 'a|bc|[de]'\n  rexgen 'a*' --max-len 1KiB --count\n  \
             rexgen '[ab]{1,3}' --generate --yes --max-total-bytes 4b\n  \
             rexgen '[[:lower:]]{4}' --generate --out words.txt\n  \
             rexgen completions bash\n\nByte units:\n  Raw numbers are bytes. SI units use base 10: kb, MB, gigabytes. \
             IEC units use base 2: KiB, MiB, gibibytes.\n\nGeneration safety:\n  \
             --generate asks for confirmation before writing strings. Use --yes for scripts.",
        )
        .arg(
            Arg::new("pattern")
                .index(1)
                .action(ArgAction::Set)
                .required(true)
                .value_name("PATTERN")
                .help("Rust regex pattern to analyze or generate")
                .long_help(
                    "Rust regex pattern to analyze or generate. The regex is parsed with \
                     regex-syntax and output strings are valid UTF-8.",
                )
                .value_parser(ValueParser::string()),
        )
        .arg_required_else_help(true)
        .arg(
            Arg::new("count")
                .short('c')
                .long("count")
                .help("Print only the match count")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("size")
                .long("size")
                .help("Print only total corpus byte size")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("generate")
                .short('g')
                .long("generate")
                .help("Generate matching strings")
                .long_help(
                    "Generate matching strings, one per line. Without --yes, rexgen asks for \
                     confirmation before writing to stdout or opening --out.",
                )
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("yes")
                .short('y')
                .long("yes")
                .help("Skip generation confirmation prompt")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("limit")
                .long("limit")
                .action(ArgAction::Set)
                .value_name("N")
                .help("Maximum number of strings to generate")
                .value_parser(clap::value_parser!(u64)),
        )
        .arg(
            Arg::new("min-len")
                .short('l')
                .long("min-len")
                .action(ArgAction::Set)
                .value_name("BYTES")
                .help("Minimum UTF-8 byte length to count or generate")
                .value_parser(clap::value_parser!(ByteSize)),
        )
        .arg(
            Arg::new("max-len")
                .short('u')
                .long("max-len")
                .action(ArgAction::Set)
                .value_name("BYTES")
                .help("Maximum UTF-8 byte length to count or generate")
                .value_parser(clap::value_parser!(ByteSize)),
        )
        .arg(
            Arg::new("max-total-bytes")
                .short('m')
                .long("max-total-bytes")
                .action(ArgAction::Set)
                .value_name("BYTES")
                .help("Stop generation before total emitted string bytes exceed this cap")
                .value_parser(clap::value_parser!(ByteSize)),
        )
        .arg(
            Arg::new("out")
                .short('o')
                .long("out")
                .action(ArgAction::Set)
                .value_name("PATH")
                .help("Write generated strings to a file instead of stdout")
                .value_parser(ValueParser::path_buf()),
        )
        .subcommand(
            Command::new("completions")
                .about("Generate shell completion script")
                .long_about(
                    "Generate a shell completion script and print it to stdout. Redirect the \
                     output to the location expected by your shell or package manager.",
                )
                .arg(
                    Arg::new("shell")
                        .index(1)
                        .required(true)
                        .value_name("SHELL")
                        .help("Shell to generate completions for")
                        .value_parser(clap::value_parser!(Shell)),
                ),
        )
}
