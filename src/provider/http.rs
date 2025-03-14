use super::{ModProvider, ModProvision};
use anyhow::Result;
use async_trait::async_trait;
use reqwest::{header, Client, StatusCode};
use std::{fs, io, path::Path};

#[derive(Default)]
pub struct Http;

#[async_trait]
impl ModProvider for Http {
    async fn install_or_update(
        &self,
        mod_path: &Path,
        identifier: &str,
        tag: Option<&str>,
    ) -> Result<ModProvision> {
        let client = Client::new();
        let mut request = client.get(identifier);

        if let Some(etag) = tag {
            if mod_path.exists() {
                request = request.header(header::IF_NONE_MATCH, etag);
            }
        }

        let response = request.send().await?.error_for_status()?;
        if response.status() == StatusCode::NOT_MODIFIED {
            return Ok(ModProvision::Unchanged);
        }

        let etag = response
            .headers()
            .get(header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(ToOwned::to_owned);

        let content = response.bytes().await?;

        let is_update = mod_path.is_dir();
        extract_archive(io::Cursor::new(content), mod_path)?;

        if is_update {
            Ok(ModProvision::Updated(etag))
        } else {
            Ok(ModProvision::Installed(etag))
        }
    }
}

fn extract_archive(archive: impl io::Read + io::Seek, dest: impl AsRef<Path>) -> Result<()> {
    let tempdir = tempfile::tempdir()?;

    compress_tools::uncompress_archive(
        archive,
        tempdir.path(),
        compress_tools::Ownership::Preserve,
    )?;

    let mut root = tempdir.path().to_owned();
    loop {
        let mut dir_items = fs::read_dir(&root)?;
        let Some(first_item) = dir_items.next() else {
            break;
        };
        let first_item = first_item?;
        if dir_items.next().is_none() && first_item.file_type()?.is_dir() {
            root = first_item.path();
        } else {
            break;
        }
    }

    fs::remove_dir_all(&dest).ok();
    fs::create_dir_all(&dest)?;
    fs::rename(root, dest)?;

    Ok(())
}
