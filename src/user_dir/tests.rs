// use crate::avatar::{get_url_data, does_cache_file_exist, get_or_create_avatar_file};
use crate::user_dir::get_or_create_working_dir;
use crate::model::*;
use pretty_assertions::assert_eq;
use tempfile::tempdir;

#[test]
fn returns_existing_working_directory() {
  let temp_working_dir = tempdir().unwrap();
  let working_directory = WorkingDirectory::new(temp_working_dir.path());
  let result = get_or_create_working_dir(&working_directory);

  assert_eq!(result.unwrap(), WorkingDirectoryStatus::Exists);
}

#[test]
fn creates_working_directory_if_not_found() {
  let temp_working_dir = tempdir().unwrap();
  let working_directory = WorkingDirectory::new(temp_working_dir.path());
  temp_working_dir.close().unwrap(); //delete temp directory

  let result = get_or_create_working_dir(&working_directory);

  assert_eq!(result.unwrap(), WorkingDirectoryStatus::Created);
  assert_eq!(working_directory.working_directory_path().exists(), true);
  assert_eq!(working_directory.avatar_cache_dir().cache_path().exists(), true);
}
