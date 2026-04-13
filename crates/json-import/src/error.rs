use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("JSON import error: {0}")]
    JsonImport(String),
}
