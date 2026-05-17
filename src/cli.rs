use clap::{Arg, ArgAction, Command, builder::ValueParser};

pub(crate) fn App() -> Command {
    Command::new("rexgen")
        .arg(
            Arg::new("pattern")
                .short('p')
                .long("pattern")
                .action(ArgAction::Set)
                .required(true)
                .value_parser(ValueParser::string()),
        )
        .arg_required_else_help(true)
        .arg(
            Arg::new("count")
                .short('c')
                .long("count")
                .action(ArgAction::SetTrue),
        )
        .arg(
            Arg::new("generate")
                .short('g')
                .long("generate")
                .action(ArgAction::SetTrue)
                .conflicts_with("count"),
        )
}
