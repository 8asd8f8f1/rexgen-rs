#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("Io error: {0}")]
    // Io(#[from] std::io::Error),
    Io(&'static str),

    #[error("Parse error: {0}")]
    Parse(&'static str),

    #[error("{0}")]
    Message(String),
}

pub(crate) type Result<T> = core::result::Result<T, Error>;
