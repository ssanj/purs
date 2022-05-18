use crate::model::{HomeDirectory, CommandLineArgumentFailure, WorkingDirectory, WorkingDirectoryStatus};
use dirs::home_dir;
use std::fs;

pub fn get_home_dir() -> Result<HomeDirectory, CommandLineArgumentFailure> {
    let error_message = "Could not find home directory";
    let error = CommandLineArgumentFailure::new(error_message);
    home_dir()
      .map(|hr| HomeDirectory::new(hr.as_ref()))
      .ok_or(error)
}

pub fn get_or_create_working_dir(working_dir: &WorkingDirectory) -> Result<WorkingDirectoryStatus, CommandLineArgumentFailure> {
    match fs::metadata(working_dir.to_string()) {
      Ok(dir) =>
          if dir.is_dir() {
              Ok(WorkingDirectoryStatus::Exists)
          } else {
              let error_message = format!("{} is not a directory", working_dir);
              let error = CommandLineArgumentFailure::new(&error_message);
              Err(error)
          },
      Err(e1) => {
          //working_dir is not a directory, try and create it
          let dir_creation =
            fs::create_dir_all(working_dir.working_directory_path())
            .and_then(|_| {
              fs::create_dir_all(working_dir.avatar_cache_dir().to_string())
            });
          match dir_creation {
              Ok(_) =>  Ok(WorkingDirectoryStatus::Created),
              Err(e2) => {
                let error_message = format!("Could not create dir: {}\n\t{}\n\t\t{}", working_dir, e1, e2);
                let error = CommandLineArgumentFailure::new(&error_message);
                Err(error)
              },
          }
      }
    }
}

// ---------------------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests;
