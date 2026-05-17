enum QuantifierType {
    Literal { len: u64 },
    Fixed { len: u64 },
    Min { min: u64 },
    MinMax { min: u64, max: u64 },
    QuestionMark,
    Asterisk,
    Plus,
}

pub struct Parsed {}
