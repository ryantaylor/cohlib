use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("SGA archive error: {0}")]
    Sga(String),
}
