use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("replay parse error: {0}")]
    Replay(String),
}
