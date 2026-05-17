pub(crate) type Result<T> = core::result::Result<T, Error>;

#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("regex parse error: {0}")]
    Parse(#[from] regex_syntax::Error),

    #[error("regex compile error: {0}")]
    Regex(#[from] regex::Error),

    #[error("unsupported regex feature: {0}")]
    Unsupported(&'static str),

    #[error("{0}")]
    Message(String),
}
