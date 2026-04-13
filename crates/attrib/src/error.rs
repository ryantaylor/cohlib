use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("attribute parse error: {0}")]
    Attrib(String),
    #[error(transparent)]
    Sga(#[from] sga::Error),
}
