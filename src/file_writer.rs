use crate::model::*;
use crate::file_tools::{create_file_and_path, to_file_error};

use std::collections::HashMap;
use std::io::Write;
use std::path::Path;
use std::fs::File;
use std::time::Instant;


// TODO: Do we want the diff file to be configurable?
pub fn write_diff_files(checkout_path: &str, pr_diffs: &PullRequestDiff) -> R<()> {
  println!("Generating diff files...");

  let write_start = Instant::now();

  let file_list_path = Path::new(checkout_path).join(DIFF_FILE_LIST);
  // TODO: Do we want to wrap this error?
  let mut file_list = File::create(&file_list_path) .unwrap();

  pr_diffs.diffs().iter().for_each(|d| {
      writeln!(file_list, "{}.diff", d.file_name).unwrap(); // TODO: Do we want to wrap this error?

      let diff_file_name = format!("{}.diff", d.file_name);
      let diff_file = Path::new(checkout_path).join(&diff_file_name);

      let mut f = create_file_and_path(&diff_file).unwrap();

      println!("Creating {}", &diff_file_name);
      let buf: &[u8]= d.contents.as_ref();
      f.write_all(buf)
        .map_err(|e| {
          to_file_error(
            &format!(
              "Could not write diff_file contents: \n{}",
              std::str::from_utf8(buf)
                .unwrap_or("<Could not retrieve content due to a UTF8 decoding error>")
                ), e)
          })
        .unwrap();
  });

  let time_taken = write_start.elapsed().as_millis();
  println!("Writing diff files took {} ms", time_taken);

  Ok(())
}


pub fn write_comment_files(checkout_path: &str, comments: &Comments, avatar_hash: HashMap<Url, FileUrl>) -> R<()> {
  if !comments.is_empty() {
    println!("Generating comment files...");

    let write_start = Instant::now();

    let file_comments_json = CommentJson::grouped_by_line_2(comments.clone(), avatar_hash);

    file_comments_json.into_iter().for_each(|file_comments_json|{
      let comment_file_name = format!("{}.comment", file_comments_json.file_name);
      let comment_file = Path::new(checkout_path).join(&comment_file_name);

      match serde_json::to_string_pretty(&file_comments_json) {
        Ok(contents) => {
          let mut cf = File::create(&comment_file).unwrap(); // TODO: Do we want to wrap this error?
          println!("Creating {}", &comment_file_name);
          let buf: &[u8]= contents.as_ref();
          cf.write_all(buf).unwrap(); // TODO: Do we want to wrap this error?
        },
        Err(error) => eprintln!("Could not created comment file {}: {}", comment_file.to_string_lossy(), error)
      }
    });

    let time_taken = write_start.elapsed().as_millis();
    println!("Writing comment files took {} ms", time_taken);
  }

  Ok(())
}
