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
              let error_message = format!("{} is not a directory", working_dir.to_string());
              let error = CommandLineArgumentFailure::new(&error_message);
              Err(error)
          },
      Err(e1) => {
          //working_dir is not a directory, try and create it
          //use avatar_cache_dir because it's nested under the working dir
          //and creating it will create the working_dir as well.
          //two birds, one stone?
          match fs::create_dir_all(working_dir.avatar_cache_dir().to_string()) {
              Ok(_) =>  Ok(WorkingDirectoryStatus::Created),
              Err(e2) => {
                let error_message = format!("Could not create dir: {}\n\t{}\n\t\t{}", working_dir.to_string(), e1, e2);
                let error = CommandLineArgumentFailure::new(&error_message);
                Err(error)
              },
          }
      }
    }
}
