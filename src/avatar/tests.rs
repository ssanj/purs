use dirs::cache_dir;
use wiremock::{MockServer, Mock, ResponseTemplate};
use wiremock::matchers::{method, path};
use crate::avatar::{get_url_data, does_cache_file_exist, get_or_create_avatar_file};
use crate::model::{Url, AvatarCacheFile, CacheFileStatus, UserId, AvatarCacheDirectory, AvatarInfo};

use tempfile::tempdir;
use std::fs::File;
use std::io::{Write, Read};


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
  let cache_dir = AvatarCacheDirectory::from(tempdir().unwrap().path());
  let avatar_file = AvatarCacheFile::new(&UserId::new(1000), cache_dir);
  let result = does_cache_file_exist(&avatar_file).await;
  assert_eq!(result.unwrap(), CacheFileStatus::DoesNotExist)
}


#[tokio::test]
async fn test_does_cache_file_exist_with_file() {
  let cache_dir = tempdir().unwrap();
  let cache_dir_path = AvatarCacheDirectory::from(cache_dir.path());

  let avatar_file_path = cache_dir.path().join("1000.png");
  let mut file = File::create(avatar_file_path).expect("Could not create avatar file");
  writeln!(file, "1234").expect("Could not write avatar file");

  let avatar_file = AvatarCacheFile::new(&UserId::new(1000), cache_dir_path);
  let result = does_cache_file_exist(&avatar_file).await;

  drop(file);
  cache_dir.close().expect("Could not close temp dir");

  assert_eq!(result.unwrap(), CacheFileStatus::Exists);
}


#[tokio::test]
async fn test_get_or_create_avatar_file_not_cached() {
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

    let file_name = "12345.png";
    let cache_dir = AvatarCacheDirectory::from(tempdir().unwrap().path());
    let avatar_url = Url::new(format!("{}/u/12345?v=4", &mock_server.uri()));
    let user_id = UserId::new(12345);
    let avatar_info = AvatarInfo::new(user_id, avatar_url, cache_dir.clone());
    let file_url = get_or_create_avatar_file(&avatar_info).await.unwrap();
    let expected_file_url = format!("file://{}/{}", cache_dir.to_string(), file_name);

    assert_eq!(file_url.to_string(), expected_file_url);

    let cache_file_path = expected_file_url.replace("file://", "");
    let mut file = File::open(&cache_file_path).expect(&format!("could not find file: {}", &cache_file_path));
    let mut file_contents = String::new();
    file.read_to_string(&mut file_contents).unwrap();

    assert_eq!(file_contents, std::str::from_utf8(data).unwrap());
}

#[tokio::test]
async fn test_get_or_create_avatar_file_from_cache() {
    let data = "1234".as_bytes();
    let file_name = "12345.png";
    let cache_dir = AvatarCacheDirectory::from(tempdir().unwrap().path());
    let expected_file_url = format!("file://{}/{}", cache_dir, file_name);
    let cache_file_path = format!("{}/{}", cache_dir, file_name);

    let avatar_url = Url::new(format!("/u/12345?v=4"));
    let user_id = UserId::new(12345);
    let avatar_info = AvatarInfo::new(user_id, avatar_url, cache_dir.clone());

    let mut file =  File::create(&cache_file_path).expect(&format!("could not create file: {}", &cache_file_path));
    file.write_all(data).unwrap();


    let file_url = get_or_create_avatar_file(&avatar_info).await.unwrap();

    assert_eq!(file_url.to_string(), expected_file_url);
}
