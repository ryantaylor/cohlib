use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("data load error: {0}")]
    Load(String),
}
