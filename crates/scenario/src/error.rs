use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("scenario parse error: {0}")]
    Scenario(String),
    #[error(transparent)]
    Sga(#[from] sga::Error),
}
