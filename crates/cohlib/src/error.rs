use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Replay(#[from] replay::Error),
    #[error(transparent)]
    Data(#[from] data::Error),
    #[error(transparent)]
    BuildOrder(#[from] build_order::Error),
}
