use serde::{Deserialize, Serialize};
use std::fmt;

mod manager;
pub use manager::*;

pub mod provider;
pub use provider::{ModProvider, ModProvision};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModSpec {
    pub source: String,
    pub identifier: String,
    pub tag: Option<String>,
}

impl fmt::Display for ModSpec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}:{}", self.source, self.identifier)
    }
}
