use super::{ModProvider, ModProvision};
use anyhow::Result;
use async_trait::async_trait;
use std::path::Path;

#[derive(Default)]
pub struct Civfanatics(super::Http);

impl Civfanatics {
    pub fn page_url(&self, id: &str) -> String {
        format!("https://forums.civfanatics.com/resources/{id}")
    }

    pub fn download_url(&self, id: &str) -> String {
        format!("https://forums.civfanatics.com/resources/{id}/download")
    }
}

#[async_trait]
impl ModProvider for Civfanatics {
    async fn install_or_update(
        &self,
        mod_path: &Path,
        identifier: &str,
        tag: Option<&str>,
    ) -> Result<ModProvision> {
        let download_url = self.download_url(identifier);
        self.0.install_or_update(mod_path, &download_url, tag).await
    }
}
