use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};
use crate::avatar::{get_url_data, does_cache_file_exist};
use crate::model::{Url, AvatarCacheFile, CacheFileStatus, UserId};

use tempfile::tempdir;
use std::fs::File;
use std::io::Write;


#[tokio::test]
async fn test_get_url_data() {
    // Start a background HTTP server on a random local port
    let mock_server = MockServer::start().await;
    let data = "1234".as_bytes();
    let template =
      ResponseTemplate::new(200)
        .set_body_bytes(data);

    Mock::given(method("GET"))
        .and(path("/u/12345"))
        .respond_with(template)
        // Mounting the mock on the mock server - it's now effective!
        .mount(&mock_server)
        .await;

    let url = Url::new(format!("{}/u/12345?v=4", &mock_server.uri()));
    let result = get_url_data(url).await.unwrap();

    assert_eq!(result.1, data)
}


#[tokio::test]
async fn test_does_cache_file_exist_without_file() {
  let cache_dir = tempdir().unwrap().into_path().to_string_lossy().to_string();
  let avatar_file = AvatarCacheFile::new(&UserId::new(1000), cache_dir);
  let result = does_cache_file_exist(&avatar_file).await;
  assert_eq!(result.unwrap(), CacheFileStatus::DoesNotExist)
}


#[tokio::test]
async fn test_does_cache_file_exist_with_file() {
  let cache_dir = tempdir().unwrap();
  let cache_dir_path = cache_dir.path().to_string_lossy().to_string();

  let avatar_file_path = cache_dir.path().join("1000.png");
  let mut file = File::create(avatar_file_path).expect("Could not create avatar file");
  writeln!(file, "1234").expect("Could not write avatar file");

  let avatar_file = AvatarCacheFile::new(&UserId::new(1000), cache_dir_path);
  let result = does_cache_file_exist(&avatar_file).await;

  drop(file);
  cache_dir.close().expect("Could not close temp dir");

  assert_eq!(result.unwrap(), CacheFileStatus::Exists);
}
