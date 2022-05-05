use futures::TryFutureExt;
use reqwest;
use crate::{model::{Url, PursError, R, NestedError, AvatarCacheFile, UserId, CacheFileStatus, FileUrl, CacheFileType, AvatarCreationErrorType, AvatarInfo}};
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

pub async fn get_or_create_avatar_file(avatar_info: &AvatarInfo) -> R<FileUrl> {
  let avatar_url = avatar_info.avatar_url();
  let user_id = &avatar_info.user_id();
  let cache_path = avatar_info.cache_path().clone();
  let avatar_cache_file = AvatarCacheFile::new(user_id, cache_path);
  match does_cache_file_exist(&avatar_cache_file).await? {
    CacheFileStatus::Exists => avatar_cache_file.url(),
    CacheFileStatus::DoesNotExist => {
      let downloaded_file =
        get_url_data(avatar_url.clone())
          .and_then(|url_data|{
            async {
              save_avatar_data(&avatar_cache_file, url_data.1).await
            }
          })
          .await;

      match downloaded_file {
        Ok(_) => {
          //try and load the file again but don't fail if it's not found
          match does_cache_file_exist(&avatar_cache_file).await? {
            CacheFileStatus::Exists => avatar_cache_file.url(),
            CacheFileStatus::DoesNotExist =>
              Err(
                PursError::AvatarCreationError(
                  AvatarCreationErrorType::CouldNotSaveAvatar(
                    //TODO: Add user_id, path and avatar_url into a class and dump it Display
                    format!("Could not save avatar: {}", avatar_info)
                  )
                )
              )
          }
        },
        Err(e) =>
          Err(
            PursError::AvatarCreationError(
              AvatarCreationErrorType::CouldNotDownloadAvatar(
                //TODO: Add user_id, path and avatar_url into a class and dump it in Display
                e.to_string()
              )
            )
          )
      }
    }
  }
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


#[cfg(test)]
mod tests;
