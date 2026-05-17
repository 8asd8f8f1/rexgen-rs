use thiserror::Error;

pub(crate) type Result<T> = core::result::Result<T, Error>;
// pub(crate) type Error = Box<dyn std::error::Error>;

#[non_exhaustive]
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error("Io error")]
    Io,

    #[error("rexgex parse error: ")]
    Parse,
}
