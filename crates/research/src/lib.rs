// research crate
pub fn init() {}

#[derive(Clone, Debug, Default)]
pub struct Experiment;

impl Experiment {
    pub fn from_toml(_path: impl AsRef<std::path::Path>) -> Self {
        Self
    }
}
