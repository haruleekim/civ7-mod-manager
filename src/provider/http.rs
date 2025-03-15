use super::{ModProvider, ModProvision};
use anyhow::{Result, anyhow};
use async_trait::async_trait;
use reqwest::{Client, StatusCode, header};
use std::{fs, io::Cursor, path::Path};

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
        extract_archive(content.as_ref(), mod_path)?;

        if is_update {
            Ok(ModProvision::Updated(etag))
        } else {
            Ok(ModProvision::Installed(etag))
        }
    }
}

fn extract_archive(archive: &[u8], dest: impl AsRef<Path>) -> Result<()> {
    let tempdir = tempfile::tempdir()?;

    compress_tools::uncompress_archive(
        Cursor::new(archive),
        tempdir.path(),
        compress_tools::Ownership::Ignore,
    )
    .or_else(|_| {
        let archive_file_parent_dir = tempfile::tempdir()?;
        let archive_file_path = archive_file_parent_dir.path().join("archive.rar");
        fs::write(&archive_file_path, archive)?;
        let mut archive = unrar::Archive::new(&archive_file_path).open_for_processing()?;
        while let Some(header) = archive.read_header()? {
            archive = if header.entry().is_file() {
                header.extract_with_base(tempdir.path())?
            } else {
                header.skip()?
            };
        }
        anyhow::Ok(())
    })
    .map_err(|_| anyhow!("Failed to extract archive"))?;

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

    let mut files = vec![root.clone()];
    while let Some(file) = files.pop() {
        let mut permissions = fs::metadata(&file)?.permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&file, permissions)?;
        if file.is_dir() {
            for entry in fs::read_dir(file)? {
                files.push(entry?.path());
            }
        }
    }

    fs::remove_dir_all(&dest).ok();
    fs::create_dir_all(&dest)?;
    fs::rename(root, dest)?;

    Ok(())
}
