use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("image conversion error: {0}")]
    Image(String),
}
