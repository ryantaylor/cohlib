use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("locale parse error: {0}")]
    Locale(String),
    #[error(transparent)]
    Sga(#[from] sga::Error),
}
