use futures::FutureExt;
use futures::future::{try_join_all, join_all};
use octocrab::{self, OctocrabBuilder, Octocrab};
use octocrab::params;
use octocrab::models::pulls::ReviewState as GHReviewState;
use crate::model::*;
use crate::user_dir::*;
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::io::{self, Write, ErrorKind};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use ansi_term::Colour;
use std::fs::{File, self};
extern crate unidiff;
use unidiff::PatchSet;
use std::time::Instant;
use futures::stream::{self, StreamExt};
use tui_app::render_tui;
use avatar::get_or_create_avatar_file;
use tools::partition;

mod model;
mod user_dir;
mod console;
mod tui_app;
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

fn cli() -> Result<Config, CommandLineArgumentFailure> {

  const APPVERSION: &str = env!("CARGO_PKG_VERSION");

  let working_dir_help_text = format!("Optional working directory. Defaults to USER_HOME/{}", DEFAULT_WORKING_DIR);

  let script_help: &str =
    "Optional script to run after cloning repository\n\
     Parameters to script:\n\
     param1: checkout directory for the selected PR\n\
     param2: name of the file that has the names of all the changed files\n\
     \n\
     Eg. purs --repo owner/repo --script path/to/your/script
    ";

  let app =
    clap::Command::new("purs")
    .version(APPVERSION)
    .author("Sanj Sahayam")
    .about("List and checkout open Pull Requests on a GitHub repository")
    .arg(
        clap::Arg::new("repo")
            .short('r')
            .long("repo")
            .multiple_occurrences(true)
            .takes_value(true)
            .required(true)
            .help("one or more GitHub repositories to include in the form: <owner>/<repo>"),
    )
    .arg(
        clap::Arg::new("script")
            .short('s')
            .long("script")
            .takes_value(true)
            .next_line_help(true)
            .help(script_help)
    )
    .arg(
        clap::Arg::new("gh_token")
            .takes_value(true)
            .short('t')
            .long("token")
            .env_os(OsStr::new("GH_ACCESS_TOKEN"))
            .hide_env(true)//Don't display value of GH_ACCESS_TOKEN in help text
            .help("GitHub Access Token. Can also be supplied through the GH_ACCESS_TOKEN environment variable")
    )
    .arg(
        clap::Arg::new("working_dir")
            .short('w')
            .long("wd")
            .takes_value(true)
            .help(working_dir_help_text.as_str())
    );

  let matches = app.get_matches();

  if let Some(repos) = matches.values_of("repo") {

    let repositories_result = repos.map(|r| {
      let mut rit = r.split('/').take(2);
      let invalid_format_error = format!("Invalid repository format: {}", r);
      let error = CommandLineArgumentFailure::new(&invalid_format_error);
      let owner = rit.next().ok_or_else(|| error.clone())?;
      let repo = rit.next().ok_or( error)?;

      Ok(OwnerRepo(Owner(owner.to_owned()), Repo(repo.to_owned())))
    }).collect::<Result<Vec<_>, CommandLineArgumentFailure>>();

    let repositories_vec = repositories_result?;
    let no_repositories_supplied_error = "No repositories supplied";
    let repositories =
      NonEmptyVec::from_vec(repositories_vec)
      .ok_or_else(|| CommandLineArgumentFailure::new(no_repositories_supplied_error))?;


    let script_option =
      match matches.value_of("script") {
        Some(script) => {
          let script_path = PathBuf::from_str(script);
          match script_path {
            Ok(valid_script) => ScriptType::Script(ScriptToRun::new(&valid_script)),
            Err(e) => ScriptType::InvalidScript(script.to_string(), NestedError::from(e))
          }
        },
        None => ScriptType::NoScript
      };

    let script = match script_option {
        ScriptType::NoScript => Ok(None),
        ScriptType::Script(script_to_run) => Ok(Some(script_to_run)),
        ScriptType::InvalidScript(script_supplied, e) => {
          let error = format!("Invalid Script supplied: {}. Error:{}", script_supplied, e);
          Err(CommandLineArgumentFailure::new(&error))
        },
    }?;

    let working_dir = match matches.value_of("working_dir") {
      Some(custom_working_dir) => WorkingDirectory::new(Path::new(custom_working_dir)),
      None => {
        let home_dir = get_home_dir()?;
        let working_dir = home_dir.join(DEFAULT_WORKING_DIR);
        WorkingDirectory::new(&working_dir)
      }
    };

    match get_or_create_working_dir(&working_dir)? {
      WorkingDirectoryStatus::Exists => {},
      WorkingDirectoryStatus::Created => println!("created working directory: {}", working_dir),
    }
    let avatar_cache_dir = working_dir.avatar_cache_dir();

    let token =
      matches
      .value_of("gh_token")
      .ok_or_else( || CommandLineArgumentFailure::new("Could not find Github Personal Access Token"))
      .map(GitHubToken::new)?;

    let config =
      Config {
        working_dir,
        avatar_cache_dir,
        repositories,
        token,
        script
      };

    Ok(config)
  } else {
    Err(CommandLineArgumentFailure::new("Invalid command line argument combination, expected at least one repository."))
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

    // let pull_requests = get_dummy_prs();
    // let selection_size = pull_requests.len();

    // for (index, pr) in pull_requests.clone().into_iter().enumerate() {
    //     println!("{:>2} - {}", index + 1, pr);
    // }

    let valid_selection = handle_user_selection_tui(pull_requests.clone())?;
    match valid_selection {
      ValidSelection::Quit => Ok(ProgramStatus::UserQuit),
      ValidSelection::Pr(pr) => {
        let ssh_url = pr.ssh_url.clone();
        let checkout_path = RepoCheckoutPath::new(get_extract_path(config, &pr)?);
        let branch_name = pr.branch_name;

        clone_branch(ssh_url, checkout_path.clone(), branch_name)?;
        write_diff_files(checkout_path.as_ref(), &pr.diffs)?;
        let avatar_hash = get_avatars(&pr.comments, &config.avatar_cache_dir).await?;
        let rendered_comments =
          render_markdown_comments(&octocrab,  &pr.comments).await?;

        write_comment_files(checkout_path.as_ref(), &rendered_comments, avatar_hash)?;
        match &config.script {
          Some(script) => {
            script_to_run(script, &checkout_path)?
          },
          None => {
            println!();
            println!("Checkout path: {}", checkout_path);
            println!("Diff file: {}", DIFF_FILE_LIST);
          }
        }

        Ok(ProgramStatus::CompletedSuccessfully)
      }
    }
}

fn script_to_run(script: &ScriptToRun, checkout_path: &RepoCheckoutPath) -> R<()> {
   let mut command = Command::new(script.to_string());
   command
    .arg(checkout_path.to_string()) //arg1 -> checkout dir
    .arg(DIFF_FILE_LIST); //arg2 -> diff file list

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

// fn handle_user_selection(selection_size: usize, selection_options: &[ValidatedPullRequest]) -> R<ValidSelection> {
//   match read_user_response("Please select a PR to clone to 'q' to exit", selection_size) {
//     Ok(response) => {
//         match response {
//             UserSelection::Number(selection) => {
//                 let selected = selection_options.get(usize::from(selection - 1)).unwrap();
//                 Ok(ValidSelection::Pr(selected.clone()))
//             },
//             UserSelection::Quit => {
//               Ok(ValidSelection::Quit)
//             },
//         }
//     },
//     Err(e) => Err(PursError::UserError(e))
//   }
// }


// fn read_user_response(question: &str, limit: usize) -> Result<UserSelection, UserInputError> {
//   println!("{}", question);
//   let mut buffer = String::new();
//   let stdin = io::stdin();
//   let mut handle = stdin.lock();
//   handle.read_line(&mut buffer).expect("Could not read from input");

//   let line = buffer.lines().next().expect("Could not extract line");

//   match line {
//      "q" | "Q" => Ok(UserSelection::Quit),
//      num =>
//         num.parse::<u8>()
//         .map_err( |_| UserInputError::InvalidNumber(num.to_string()))
//         .and_then(|n| {
//             let input = usize::from(n);
//             if  input == 0 || input > limit {
//                 Err(
//                     UserInputError::InvalidSelection {
//                         selected: n,
//                         min_selection: 1,
//                         max_selection: limit
//                     }
//                 )
//             } else {
//                 Ok(UserSelection::Number(n))
//             }
//         }),
//   }
// }

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



async fn get_pulls(octocrab: Octocrab, owner_repo: OwnerRepo) -> R<octocrab::Page<octocrab::models::pulls::PullRequest>> {
    let OwnerRepo(owner, repo) = owner_repo;
    octocrab
      .pulls(owner.0.to_owned(), repo.0.to_owned())
      .list()
      .state(params::State::Open)
      .sort(params::pulls::Sort::Created)
      .direction(params::Direction::Descending)
      .per_page(20)
      .send()
      .await
      .map_err( PursError::from)
}

type PageHandles = Vec<tokio::task::JoinHandle<Result<(octocrab::Page<octocrab::models::pulls::PullRequest>, OwnerRepo), PursError>>>;

//TODO: Can we break this up into multiple functions?
async fn get_prs3(config: &Config, octocrab: Octocrab) -> R<Vec<PullRequest>> {
    let page_handles:PageHandles  =
      config
      .repositories
      .to_vec()
      .into_iter()
      .map(|owner_repo| {
        tokio::task::spawn(
      get_pulls(
              octocrab.clone(), owner_repo.clone()
            )
            .map(|hr| { hr.map(|h| (h, owner_repo)) }) //write a help function for this
        )
      }).collect::<Vec<_>>();

    let page_results =
      try_join_all(page_handles)
      .await
      .map_err( PursError::from)?;

    let page_repos =
      page_results
      .into_iter()
      //TODO: Do we need to handle the errors of this?
      .map(|rp| rp.unwrap())
      .collect::<Vec<_>>();

    let async_parts = page_repos.iter().map(|(page, OwnerRepo(owner, repo))| {
            page.into_iter().map(|pull| {
                let pr_no = pull.number;
                let reviews_handle = tokio::spawn(get_reviews2(octocrab.clone(), owner.clone(), repo.clone(), pr_no));
                let comments_handle = tokio::spawn(get_comments2(octocrab.clone(), owner.clone(), repo.clone(), pr_no));
                let diffs_handle = tokio::spawn(get_pr_diffs2(octocrab.clone(), owner.clone(), repo.clone(), pr_no));

                AsyncPullRequestParts {
                    owner_repo: OwnerRepo(owner.clone(), repo.clone()),
                    pull: pull.clone(),
                    reviews_handle,
                    comments_handle,
                    diffs_handle
                }
            }).collect::<Vec<_>>()
    });

    let parts = async_parts.flatten().collect::<Vec<_>>();
    let parts_stream = stream::iter(parts);

    let pr_stream =
        parts_stream.then(|AsyncPullRequestParts { owner_repo, pull, reviews_handle, comments_handle, diffs_handle }|{
            async move {
                let res = tokio::try_join!(
                    flatten(reviews_handle),
                    flatten(comments_handle),
                    flatten(diffs_handle)
                );

                match res {
                  Ok((reviews, comments, diffs)) => {

                    let pr_no = pull.number;
                    let title = pull.title.clone().unwrap_or_else(|| "-".to_string());
                    let ssh_url = pull.head.repo.clone().and_then(|r| (r.ssh_url));
                    let head_sha = pull.head.sha;
                    let repo_name = pull.head.repo.clone().and_then(|r| r.full_name);
                    let branch_name = pull.head.ref_field;
                    let base_sha = pull.base.sha;
                    let config_owner_repo = owner_repo;
                    let draft = pull.draft;
                    let created_at = pull.created_at;
                    let updated_at = pull.updated_at;
                    let pr_owner = create_user(pull.user.clone().as_deref());

                    let pr =
                      PullRequest {
                        config_owner_repo,
                        pr_owner,
                        title,
                        pr_number: pr_no,
                        ssh_url,
                        branch_name,
                        head_sha,
                        repo_name,
                        base_sha,
                        reviews,
                        comments,
                        diffs,
                        draft,
                        created_at,
                        updated_at
                      };

                    Ok(pr)
                  },
                  Err(error) => Err(error),
              }
            }
        });


    let results_with_errors: Vec<R<PullRequest>> = pr_stream.collect().await;

    let mut pr_errors: Vec<PursError> = vec![];
    let mut pr_successes: Vec<PullRequest> = vec![];

    //similar to partition
    results_with_errors.into_iter().for_each(|r| match r {
      Ok(pr) => pr_successes.push(pr),
      Err(e) => pr_errors.push(e),
    });

    if pr_errors.is_empty() {
      Ok(pr_successes)
    } else {
      Err(PursError::MultipleErrors(pr_errors))
    }
}

fn create_user(user: Option<&octocrab::models::User>) -> Option<User> {
  user.map(From::from)
}


async fn flatten<T>(handle: tokio::task::JoinHandle<R<T>>) -> R<T> {
    match handle.await {
        Ok(result) => result,
        Err(err) => Err(PursError::from(err)),
    }
}

fn parse_diffs(diff: &str) -> R<PullRequestDiff> {
  let mut patch = PatchSet::new();
  let parse_result = patch.parse(diff).map_err(PursError::from);

  parse_result.map(|_| {
      let diffs = patch.files().iter().map (|p| {
          let file_name =
              // if a file is deleted there is no target file (because it's deleted)
              // if a file is added there is no source file (because it's a new file)
              // if a file is modified there is both a source and target file
              if p.is_removed_file() {
                  parse_only_file_name(&p.source_file)
              } else {
                  parse_only_file_name(&p.target_file)
              };

          let contents = p.to_string();

          GitDiff {
              file_name,
              contents
          }
      }).collect();

      PullRequestDiff(diffs)
  })
}

fn parse_only_file_name(diff_file: &str) -> String {
    let mut file_name = diff_file.to_string();

    // TODO: If this fails the format of the file name is not what we expected
    // Return a specific error later
    let index = file_name.find('/').unwrap() + 1;
    // Remove prefix of a/.. or b/..
    file_name.replace_range(..index, "");
    file_name
}

async fn get_reviews2(octocrab:  Octocrab, owner:  Owner, repo:  Repo, pr_no: u64) -> R<Reviews> {
    let gh_reviews =
        octocrab
        .pulls(owner.0.to_owned(), repo.0.to_owned())
        .list_reviews(pr_no)
        .await?;

   let reviews = gh_reviews.into_iter().map(|r| {

    let user = r.user.login;
    let comment = r.body;
    let state = match r.state {
      Some(GHReviewState::Approved)         => ReviewState::Approved,
      Some(GHReviewState::Pending)          => ReviewState::Pending,
      Some(GHReviewState::ChangesRequested) => ReviewState::ChangesRequested,
      Some(GHReviewState::Commented)        => ReviewState::Commented,
      Some(GHReviewState::Dismissed)        => ReviewState::Dismissed,
      _                                     => ReviewState::Other   //octocrab::models::pulls::ReviewState is non_exhaustive, so we need this wildcard match
    };

    Review {
        user,
        comment,
        state
    }
   }).collect::<Vec<_>>();

    Ok(
      Reviews {
        reviews
      }
    )
}




async fn get_comments2(octocrab: Octocrab, owner: Owner, repo: Repo, pr_no: u64) -> R<Comments> {
    let comments =
        octocrab
        .pulls(owner.0.to_owned(), repo.0.to_owned())
        .list_comments(Some(pr_no))
        .send()
        .await?;

    let comments =
      comments.into_iter().map(|c| {
        let author = User::from_comment(c.clone());
        let file_name = FileName::new(c.path);

        Comment {
          comment_id: CommentId::new(c.id.0),
          diff_hunk: c.diff_hunk,
          body: c.body,
          markdown_body: None, //this will be filled only for the selected PR's comment
          line: c.line.map(LineNumber::new),
          in_reply_to_id: c.in_reply_to_id.map(CommentId::new),
          comment_url: Url::new(c.html_url),
          author,
          file_name
        }
      }).collect();


    Ok(
      Comments {
        comments
      }
    )
}


async fn get_pr_diffs2(octocrab: Octocrab, owner: Owner, repo: Repo, pr_no: u64) -> R<PullRequestDiff> {
    let diff_string =
        octocrab
        .pulls(owner.0.to_owned(), repo.0.to_owned())
        .get_diff(pr_no)
        .await?;

    parse_diffs(&diff_string)
}


pub fn print_error(message: String) {
  let coloured_error = Colour::Red.paint(format!("Error: {}", message));
  println!("{}", coloured_error)
}

pub fn print_info(message: String) {
  let coloured_info = Colour::Green.paint(message);
  println!("{}", coloured_info)
}

//TODO check for unnecessary memory allocations
async fn render_markdown_comments(octocrab: &Octocrab, comments: &Comments) -> R<Comments> {
  let md_start = Instant::now();

  let cs = comments.clone();
  let handles = cs.comments.into_iter().map(|c|{
      tokio::task::spawn({
        render_markdown(octocrab.clone(), c.body.clone()).map(|r| {
          // can we bimap? Why does this work and r.map doesn't because of a move?
         match r {
          Ok(value) => Ok((c, value)),
          Err(e) => Err(e)
         }
        })
      })
  });

  let nested_results_vec = join_all(handles).await;

  let results = nested_results_vec.into_iter().map(|vr| {
    flatten_results(vr, PursError::from)
  });

  let mut comment_map: HashMap<CommentId, Comment> = HashMap::new();

  comments.comments.iter().for_each(|c| {
    let _ = comment_map.insert(c.comment_id.clone(), c.clone());
  });

  results.into_iter().for_each(|r| {
    //We found an update for the markdown body
    //We ignore errors - we try our best for markdown bodies but don't fail
    if let Ok((c, c_updated)) = r {
      let _ = comment_map.insert(c.comment_id.clone(), c.update_markdown_body(c_updated));
    }
  });

  let time_taken = md_start.elapsed().as_millis();
  println!("GH markdown calls took {} ms", time_taken);

  Ok(
    Comments {
      comments: comment_map.into_values().collect()
    }
  )
}

fn flatten_results<T, E, E2, F>(nested_results: Result<Result<T, E>, E2>, f: F) -> Result<T, E>
  where F: FnOnce(E2) -> E
{
  nested_results.map_err(f).and_then(std::convert::identity)
}

async fn render_markdown(octocrab: Octocrab, content: String) -> R<String> {
  octocrab
    .markdown()
    .render_raw(content)
    .await
    .map_err(PursError::from)
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
