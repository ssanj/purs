use reqwest;
use crate::{model::{Url, PursError, R, NestedError}, AvatarCacheFile, UserId};
use tokio::{io::{self, AsyncWriteExt}, fs::OpenOptions};
use tokio::fs::File;

pub async fn get_url_data(url: Url) -> R<(Url, Vec<u8>)> {
    println!("downloading data for url: {:?}", url);
    let data =
      reqwest::get(String::from(&url))
      .await
      .map_err(PursError::from)?
      .bytes()
      .await
      .map_err(PursError::from)?;

  Ok((url, data.to_vec()))
}

pub async fn get_or_create_avatar_file(user_id: &UserId, avatar_url: Url, path: String, default_avatar: Url) -> R<Url> {
  let avatar_cache_file = AvatarCacheFile::new(user_id, path);
  match does_cache_file_exist(&avatar_cache_file).await? {
    CacheFileStatus::Exists => Ok(avatar_cache_file.url()),
    CacheFileStatus::DoesNotExist => {
      let (_, url_data) = get_url_data(avatar_url).await?;
      let _ = save_avatar_data(&avatar_cache_file, url_data).await?;
      //try and load the file again but don't fail if it's not found
      match does_cache_file_exist(&avatar_cache_file).await? {
        CacheFileStatus::Exists => Ok(avatar_cache_file.url()),
        CacheFileStatus::DoesNotExist =>  Ok(default_avatar)
      }
    }
  }
}

enum CacheFileStatus {
  Exists,
  DoesNotExist
}

pub async fn does_cache_file_exist(avatar_cache_file: &AvatarCacheFile) -> R<CacheFileStatus> {
  match OpenOptions::new().read(true).open(avatar_cache_file.path()).await {
    Ok(_) => Ok(CacheFileStatus::Exists),
    Err(e) => match e.kind() {
      io::ErrorKind::NotFound => Ok(CacheFileStatus::DoesNotExist),
      _ => Err(PursError::FileError(NestedError::from(e)))
    }
  }
}


pub async fn save_avatar_data(avatar_cache_file: &AvatarCacheFile, avatar_data: Vec<u8>) -> R<()> {
  let mut file =
    File::create(avatar_cache_file.path()).await.map_err(|e| PursError::FileError(NestedError::from(e)))?;
  file.write_all(&avatar_data).await.map_err(|e| PursError::FileError(NestedError::from(e)))?;

  Ok(())
}

