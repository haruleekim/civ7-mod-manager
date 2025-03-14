use crate::ModSpec;
use anyhow::Result;
use async_trait::async_trait;
use std::{path::Path, str::FromStr};
use url::Url;

mod http;
pub use http::Http;

mod civfanatics;
pub use civfanatics::Civfanatics;

#[derive(Debug, Clone)]
pub enum ModProvision {
    Installed(Option<String>),
    Updated(Option<String>),
    Unchanged,
}

#[async_trait]
pub trait ModProvider: Send {
    async fn install_or_update(
        &self,
        mod_path: &Path,
        identifier: &str,
        tag: Option<&str>,
    ) -> Result<ModProvision>;
}

impl ModSpec {
    pub fn provider(&self) -> Box<dyn ModProvider> {
        match self.source.as_str() {
            "http" => Box::new(Http::default()),
            "civfanatics" => Box::new(Civfanatics::default()),
            _ => panic!("Unsupported source: {}", self.source),
        }
    }

    pub fn resolve_dirname(&self) -> Result<String> {
        match self.source.as_str() {
            "civfanatics" => self
                .identifier
                .split('.')
                .next()
                .map(String::from)
                .ok_or_else(|| anyhow::anyhow!("Invalid identifier")),
            _ => anyhow::bail!("Could not resolve dirname"),
        }
    }
}

impl FromStr for ModSpec {
    type Err = anyhow::Error;

    fn from_str(string: &str) -> Result<Self, Self::Err> {
        let Ok(url) = Url::parse(string) else {
            if string
                .split('.')
                .skip(1)
                .next()
                .and_then(|num| num.parse::<u64>().ok())
                .is_none()
            {
                return Err(anyhow::anyhow!("Invalid Mod ID"));
            }

            return Ok(ModSpec {
                source: "civfanatics".to_string(),
                identifier: string.to_string(),
                tag: None,
            });
        };

        let spec = match url.scheme() {
            "http" | "https" => ModSpec {
                source: "http".to_string(),
                identifier: url.to_string(),
                tag: None,
            },
            "civfanatics" => ModSpec {
                source: "civfanatics".to_string(),
                identifier: format!("{}{}", url.host_str().unwrap_or_default(), url.path()),
                tag: None,
            },
            scheme => anyhow::bail!("Unsupported scheme: {scheme:?}"),
        };

        Ok(spec)
    }
}
