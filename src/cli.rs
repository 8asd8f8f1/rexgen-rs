use clap::Arg;
use clap::ArgAction;
use clap::Command;
use clap::builder::ValueParser;
use clap::value_parser;

pub(crate) fn CliApp() -> Command {
    Command::new("rexgen")
        .arg(
            Arg::new("pattern")
                .index(1)
                .required(true)
                .action(ArgAction::Set)
                .value_parser(ValueParser::string()),
        )
        .arg(
            Arg::new("generate")
                .short('g')
                .long("generate")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("min-len")
                .long("min-leng")
                .action(ArgAction::Set)
                .value_parser(value_parser!(u64))
                .help("Minimum length of each output string"),
        )
        .arg(
            Arg::new("max-len")
                .long("max-len")
                .action(ArgAction::Set)
                .value_parser(value_parser!(u64))
                .help("Maximum length of each output string"),
        )
        .arg(
            Arg::new("count")
                .short('c')
                .long("count")
                .action(ArgAction::SetTrue)
                .help(
                    "Whether to calculate count and size of output to be generated",
                ),
        )
        .arg(
            Arg::new("confirm")
                .short('y')
                .long("confirm")
                .help(
                    "Whether to ask user for confirmation before generating output",
                )
                .default_missing_value("true")
                .action(ArgAction::SetTrue)
                .default_value("true"),
        )
        .arg(
            Arg::new("invert")
                .short('i')
                .long("invert")
                .action(ArgAction::SetTrue),
        )
}
