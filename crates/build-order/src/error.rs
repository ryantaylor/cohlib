use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("build order error: {0}")]
    BuildOrder(String),
}
