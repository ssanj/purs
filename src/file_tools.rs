use std::io::{self, ErrorKind};
use std::path::Path;
use std::fs::{File, self};
use crate::model::*;


//TODO: Move this to model
pub fn to_file_error(error_message: &str, error: io::Error) -> PursError {
    PursError::FileError(error_message.to_owned(), NestedError::from(error))
}

pub fn create_file_and_path(file: &Path) -> R<File> {
  let file_creation_result = File::create(file);
  match file_creation_result {
    Ok(f) => Ok(f),
    Err(e) => try_create_parent_directories(file, e)
  }
}

pub fn get_extract_path(config: &Config, pull: &ValidatedPullRequest) -> R<String> {
    let repo_name = pull.repo_name.clone();
    let branch_name = pull.branch_name.clone();
    let separator = format!("{}", std::path::MAIN_SEPARATOR);
    let extraction_path =
      vec![
        config.working_dir.to_string(),
        repo_name.to_string(),
        branch_name.to_string(),
        pull.pr_number.to_string(),
        pull.head_sha.clone()
      ].join(&separator);

    Ok(extraction_path)
}


fn try_create_parent_directories(file: &Path, e: io::Error) -> R<File> {
  match e.kind() {
    ErrorKind::NotFound => {
      match file.parent() {
        Some(parent_dir) => {
            fs::create_dir_all(parent_dir)
              .and_then(|_| File::create(file))
              .map_err(|e| {
            let error_message = format!("Could not created file: {}", get_file_name(file));
            to_file_error(&error_message, e)
          })
        },
        None => {
          return Err(
            to_file_error(
              &format!("Could not create file because it does not have a parent directory: {}", get_file_name(file)),
              e
            ))
        }
      }
    },
    _ => {
      return Err(
        to_file_error(
          &format!("Could not create file: {}", get_file_name(file)),
          e
      ))
    }
  }
}

fn get_file_name(file_path: &Path) -> String {
  file_path.to_string_lossy().to_string()
}
