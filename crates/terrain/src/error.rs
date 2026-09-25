use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("terrain parse error: {0}")]
    Parse(String),
    #[error(transparent)]
    Scenario(#[from] scenario::Error),
    #[error(transparent)]
    Xml(#[from] quick_xml::Error),
}
