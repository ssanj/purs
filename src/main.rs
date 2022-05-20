use futures::FutureExt;
use futures::future::{try_join_all, join_all};
use octocrab::{self, OctocrabBuilder, Octocrab};
use crate::model::*;
use std::collections::{HashMap, HashSet};
use std::io::{self, Write, ErrorKind};
use std::path::Path;
use std::process::Command;
use ansi_term::Colour;
use std::fs::{File, self};

use std::time::Instant;
use tui_app::render_tui;
use avatar::get_or_create_avatar_file;
use tools::partition;
use cli::cli;
use github::{get_prs3, render_markdown_comments};

mod model;
mod cli;
mod user_dir;
mod console;
mod tui_app;
mod github;
mod tools;
mod avatar;

#[tokio::main]
async fn main() {

  match cli() {
    Ok(config) => {
      let program_result = handle_program(&config).await;

      match program_result {
        Ok(ProgramStatus::UserQuit) =>  println!("Goodbye!"),
        Ok(ProgramStatus::CompletedSuccessfully) => println!("Purs completed successfully"),
        Err(purs_error) => println!("Purs Error: {}", purs_error)
      }
    },
    Err(e) => {
      let error = format!("Could not launch purs due to an error in command line arguments. Error: {}", e);
      eprintln!("{}", error)
    }
  }
}


async fn handle_program(config: &Config) -> R<ProgramStatus> {
    //TODO: Move to another function
    let octocrab =
        OctocrabBuilder::new()
        .personal_token(config.token.to_string())
        .build()
        .map_err(PursError::from)?;

    let pr_start = Instant::now();
    let pull_requests_raw: Vec<PullRequest> = get_prs3(config, octocrab.clone()).await?;

    // Remove any invalid PRs without a clonable url    let pull_requests =
    let pull_requests=
      pull_requests_raw
      .into_iter()
      .filter_map(|pr| {
        match (pr.ssh_url, pr.repo_name) {
          (Some(ssh_url), Some(repo_name)) => {
            Some(
              ValidatedPullRequest {
                config_owner_repo: pr.config_owner_repo,
                pr_owner: pr.pr_owner,
                title : pr.title,
                pr_number : pr.pr_number,
                ssh_url: GitRepoSshUrl::new(ssh_url),
                repo_name: Repo(repo_name),
                branch_name: RepoBranchName::new(pr.branch_name),
                head_sha: pr.head_sha,
                base_sha: pr.base_sha,
                reviews: pr.reviews,
                comments: pr.comments,
                diffs: pr.diffs,
                draft: pr.draft.unwrap_or(false),
                created_at: pr.created_at,
                updated_at: pr.updated_at,
              }
            )
          },
          _ => None // Filter out PRs that don't have an ssh url or repo name
        }

      })
      .collect::<Vec<_>>();

    let time_taken = pr_start.elapsed().as_millis();

    println!("GH API calls took {} ms", time_taken);

    let valid_selection = handle_user_selection_tui(pull_requests.clone())?;
    match valid_selection {
      ValidSelection::Quit => Ok(ProgramStatus::UserQuit),
      ValidSelection::Pr(mode, pr ) => {
        let ssh_url = pr.ssh_url.clone();
        let checkout_path = RepoCheckoutPath::new(get_extract_path(config, &pr)?);
        let branch_name = pr.branch_name.clone();

        println!("mode: {}", mode);

        match mode {
          Mode::Review => {
            clone_branch(ssh_url, checkout_path.clone(), branch_name)?;
            write_diff_files(checkout_path.as_ref(), &pr.diffs)?;
            handle_comment_generation(octocrab.clone(), config, (*pr).clone(), checkout_path.clone()).await?;
          },
          Mode::Edit => {
            clone_branch(ssh_url, checkout_path.clone(), branch_name)?;
            handle_comment_generation(octocrab.clone(), config, (*pr).clone(), checkout_path.clone()).await?;
          },
        };

        match &config.script {
          Some(script) => {
            script_to_run(script, &mode, &checkout_path)?
          },
          None => {
            println!();
            println!("Mode: {}", mode);
            println!("Checkout path: {}", checkout_path);
            println!("Diff file: {}", DIFF_FILE_LIST);
          }
        }

        Ok(ProgramStatus::CompletedSuccessfully)
      }
    }
}

async fn handle_comment_generation(octocrab: Octocrab, config: &Config, pr: ValidatedPullRequest, checkout_path: RepoCheckoutPath) -> R<()> {
  if config.include_comments {
    let avatar_hash = get_avatars(&pr.comments, &config.avatar_cache_dir).await?;
    let rendered_comments =
      render_markdown_comments(&octocrab,  &pr.comments).await?;

    write_comment_files(checkout_path.as_ref(), &rendered_comments, avatar_hash)?;
  }

  Ok(())
}

fn script_to_run(script: &ScriptToRun, mode: &Mode, checkout_path: &RepoCheckoutPath) -> R<()> {
  let mut command = Command::new(script.to_string());
  command
    .arg(checkout_path.to_string()) //arg1 -> checkout dir
    .arg(mode.short_string()); //arg2 -> mode

   if let Mode::Review = mode {
      command.arg(DIFF_FILE_LIST); //arg3 -> diff file list
   };

   match command.status() {
    Ok(exit_status) => {
      if exit_status.success() {
        Ok(())
      } else {
        Err(
          PursError::ScriptExecutionError(ScriptErrorType::NonZeroResult(exit_status.to_string()))
        )
      }
    },
    Err(error) =>
      Err(
          PursError::ScriptExecutionError(ScriptErrorType::Error(NestedError::from(error)))
      )
  }
}

fn handle_user_selection_tui(pulls: Vec<ValidatedPullRequest>) -> R<ValidSelection> {
  render_tui(pulls)
}


fn clone_branch(ssh_url: GitRepoSshUrl, checkout_path: RepoCheckoutPath, branch_name: RepoBranchName) -> R<()> {
    print_info(format!("git clone {} -b {} {}", ssh_url, branch_name, checkout_path));
    let mut command = Command::new("git") ;
      command
      .arg("clone")
      .arg(ssh_url)
      .arg("-b")
      .arg(branch_name.as_ref())
      .arg(checkout_path.as_ref());

    let git_clone_result = get_process_output(&mut command);

    let _ = match git_clone_result {
      Ok(CmdOutput::Success) => {}, //Success will be returned at the end of the function
      Ok(CmdOutput::Failure(exit_code)) => {
          match exit_code {
              ExitCode::Code(code) => return Err(PursError::GitError(format!("Git exited with exit code: {}", code))),
              ExitCode::Terminated => return Err(PursError::GitError("Git was terminated".to_string())),
          }
      },
      Err(e2) => {
        let e1 = PursError::GitError("Error running Git".to_string());
        return Err(PursError::MultipleErrors(vec![e1, e2]))
      },
    };

    Ok(())
}

// TODO: Do we want the diff file to be configurable?
fn write_diff_files(checkout_path: &str, diffs: &PullRequestDiff) -> R<()> {
  println!("Generating diff files...");

  let write_start = Instant::now();

  let file_list_path = Path::new(checkout_path).join(DIFF_FILE_LIST);
  // TODO: Do we want to wrap this error?
  let mut file_list = File::create(&file_list_path) .unwrap();

  diffs.0.iter().for_each(|d| {
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

fn to_file_error(error_message: &str, error: io::Error) -> PursError {
    PursError::FileError(error_message.to_owned(), NestedError::from(error))
}

fn create_file_and_path(file: &Path) -> R<File> {
  let file_creation_result = File::create(file);
  match file_creation_result {
    Ok(f) => Ok(f),
    Err(e) => try_create_parent_directories(file, e)
  }
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

fn write_comment_files(checkout_path: &str, comments: &Comments, avatar_hash: HashMap<Url, FileUrl>) -> R<()> {
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

fn get_process_output(command: &mut Command) -> R<CmdOutput> {
    let result =
      command
      .status()
      .map_err(|e| PursError::ProcessError(NestedError::from(e)));

    result.map(|r|{
        if r.success() {
            CmdOutput::Success
        } else {
            r.code()
            .map(|c| CmdOutput::Failure(ExitCode::Code(c)))
            .unwrap_or(CmdOutput::Failure(ExitCode::Terminated))
        }
    })

}

fn get_extract_path(config: &Config, pull: &ValidatedPullRequest) -> R<String> {
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


pub fn print_error(message: String) {
  let coloured_error = Colour::Red.paint(format!("Error: {}", message));
  println!("{}", coloured_error)
}

pub fn print_info(message: String) {
  let coloured_info = Colour::Green.paint(message);
  println!("{}", coloured_info)
}


async fn get_avatars(comments: &Comments, avatar_cache_directory: &AvatarCacheDirectory) -> R<HashMap<Url, FileUrl>> {
  let mut unique_gravatar_urls: HashSet<AvatarInfo> = HashSet::new();
  comments.comments.iter().for_each(|c| {
    let avatar =
      AvatarInfo::new(
        c.author.clone().user_id(),
        c.author.clone().gravatar_url(),
        avatar_cache_directory.clone()
      );

    unique_gravatar_urls.insert(avatar);
  });

  let url_data_handles = unique_gravatar_urls.into_iter().map(|u| {
    tokio::task::spawn(get_avatar_from_cache(u))
  });

  let url_data_results_with_errors =
    try_join_all(url_data_handles)
    .await
    .map_err(PursError::from)?;

  let (url_data_results, errors) =
    partition(url_data_results_with_errors);

  if !errors.is_empty() {
    log_errors("get_avatars got the following errors:", errors)
  }

  Ok(url_data_results.into_iter().collect())
}

fn log_errors(message: &str, errors: Vec<PursError>) {
  println!("{}", message);
  errors.into_iter().for_each(|e| {
    eprintln!("  {}", e)
  })
}

async fn get_avatar_from_cache(avatar_info: AvatarInfo) -> R<(Url, FileUrl)> {
  get_or_create_avatar_file(
    &avatar_info
  )
  .await
  .map(|file_url|{
    (avatar_info.avatar_url(), file_url)
  })
}
