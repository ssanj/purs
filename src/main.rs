use futures::FutureExt;
use futures::future::{join, join_all, try_join_all};
use octocrab::models::Contents;
use octocrab::{self, OctocrabBuilder, Octocrab};
use octocrab::params;
use crate::model::*;
use std::error::Error;
use std::io::{self, BufRead, Write};
use std::path::Path;
use std::process::Command;
use ansi_term::Colour;
use std::fs::File;
extern crate unidiff;
use unidiff::PatchSet;
use std::time::Instant;
use std::collections::HashMap;
use tokio::task::JoinHandle;
use futures::stream::{self, StreamExt};

mod model;

#[tokio::main]
async fn main() -> R<()> {

    let token = std::env::var("GH_ACCESS_TOKEN").expect("Could not find Github Personal Access Token");

    let repo1 = OwnerRepo(Owner("disneystreaming".to_string()), Repo("weaver-test".to_string()));
    let repo2 = OwnerRepo(Owner("scalatest".to_string()), Repo("scalatest".to_string()));

    let config =
        Config {
            working_dir: Path::new("/Users/sanj/ziptemp/prs").to_path_buf(),
            repositories: NonEmptyVec::new(repo1, vec![repo2])
        };

    let octocrab =
        OctocrabBuilder::new()
        .personal_token(token)
        .build()
        .map_err(PursError::from)?;

    let pr_start = Instant::now();
    // Handle resulting errors here, instead of using `?`
    let result = get_prs3(&config, octocrab).await?;
    let time_taken = pr_start.elapsed().as_secs();

    println!("PRs took {} seconds", time_taken);

    // let result = get_dummy_prs();
    let length = result.len();

    for (index, pr) in result.clone().into_iter().enumerate() {
        println!("{:>2} - {}", index + 1, pr);
    }

    match read_user_response("Please select a PR to clone to 'q' to exit", length) {
        Ok(response) => {
            match response {
                UserSelection::Number(selection) => {
                    let selected = result.get(usize::from(selection - 1)).expect("Invalid index");
                    clone_branch(&config, &selected).unwrap()
                },
                UserSelection::Quit => println!("Goodbye!"),
            }
        },
        Err(e) => match e {
            UserInputError::InvalidNumber(invalid) => println!("{} is not a valid number", invalid),
            UserInputError::InvalidSelection { selected, min_selection, max_selection } => println!("{} is not a number between [{} - {}]", selected, min_selection, max_selection),
        }
    }

    Ok(())
}

enum ProgramStatus {
  UserQuit,
  CompletedSuccessfully
}

async fn handle_program(token: String, config: &Config) -> R<ProgramStatus> {
    let octocrab =
        OctocrabBuilder::new()
        .personal_token(token)
        .build()
        .map_err(PursError::from)?;

    let pr_start = Instant::now();
    // Handle resulting errors here, instead of using `?`
    let pull_requests = get_prs3(config, octocrab).await?;
    let time_taken = pr_start.elapsed().as_secs();

    println!("PRs took {} seconds", time_taken);

    // let pull_requests = get_dummy_prs();
    let selection_size = pull_requests.len();

    for (index, pr) in pull_requests.clone().into_iter().enumerate() {
        println!("{:>2} - {}", index + 1, pr);
    }

    let valid_selection = handle_user_selection(selection_size, &pull_requests)?;
    match valid_selection {
      ValidSelection::Quit => Ok(ProgramStatus::UserQuit),
      ValidSelection::Pr(pr) => {
        clone_branch(&config, &pr)?;
        Ok(ProgramStatus::CompletedSuccessfully)
      }
    }

}

enum ValidSelection {
  Quit,
  Pr(PullRequest)
}

fn handle_user_selection(selection_size: usize, selection_options: &Vec<PullRequest>) -> R<ValidSelection> {
  match read_user_response("Please select a PR to clone to 'q' to exit", selection_size) {
        Ok(response) => {
            match response {
                UserSelection::Number(selection) => {
                    let selected = selection_options.get(usize::from(selection - 1)).unwrap();
                    Ok(ValidSelection::Pr(selected.clone()))
                },
                UserSelection::Quit => {
                  //TODO: Print at a higher level
                  println!("Goodbye!");
                  Ok(ValidSelection::Quit)
                },
            }
        },
        Err(e) => Err(PursError::UserError(e))
           // match e {
           //  UserInputError::InvalidNumber(invalid) => {
           //    //TODO: Print at a higher level
           //    println!("{} is not a valid number", invalid);
           //    Err(PursError::UserError(e))
           //  },
           //  UserInputError::InvalidSelection { selected, min_selection, max_selection } => {
           //    //TODO: Print at a higher level
           //    println!("{} is not a number between [{} - {}]", selected, min_selection, max_selection);
           //    Err(PursError::UserError(e))
           //  },
        // }
    }
}


fn read_user_response(question: &str, limit: usize) -> Result<UserSelection, UserInputError> {
  println!("{}", question);
  let mut buffer = String::new();
  let stdin = io::stdin();
  let mut handle = stdin.lock();
  handle.read_line(&mut buffer).expect("Could not read from input");

  let line = buffer.lines().next().expect("Could not extract line");

  match &line[..] {
     "q" | "Q" => Ok(UserSelection::Quit),
     num =>
        num.parse::<u8>()
        .map_err( |_| UserInputError::InvalidNumber(num.to_string()))
        .and_then(|n| {
            let input = usize::from(n);
            if  input <= 0 || input > limit {
                Err(
                    UserInputError::InvalidSelection {
                        selected: n,
                        min_selection: 1,
                        max_selection: limit
                    }
                )
            } else {
                Ok(UserSelection::Number(n))
            }
        }),
  }
}

// TODO: Matching on the ssh_url and write_diff_files should be done at a higher level - not here.
fn clone_branch(config: &Config, pull: &PullRequest) -> R<()> {
  match &pull.ssh_url {
      Some(ssh_url) => {
          let checkout_path = get_extract_path(&config, &pull);
          print_info(format!("git clone {} -b {} {}", ssh_url, pull.branch_name.as_str(), checkout_path.as_str()));
          let mut command = Command::new("git") ;
            command
            .arg("clone")
            .arg(ssh_url)
            .arg("-b")
            .arg(pull.branch_name.as_str())
            .arg(checkout_path.as_str());

          let output = get_process_output(&mut command);

          match output {
            Ok(CmdOutput::Success) => {
              write_diff_files(checkout_path.as_str(), &pull.diffs)
            },
            Ok(CmdOutput::Failure(exit_code)) => {
                match exit_code {
                    ExitCode::Code(code) => print_error(format!("Git exited with exit code: {}", code)),
                    ExitCode::Terminated => print_error("Git was terminated".to_string()),
                }

                Ok(())
            },
            Err(e) => Err(PursError::GitError(format!("Could not run Git: {}", e))),
          }
      },
      None => Err(PursError::GitError("Can't clone branch without SSH url".to_owned()))
  }
}

// TODO: Do we want the diff file to be configurable?
fn write_diff_files(checkout_path: &str, diffs: &PullRequestDiff) -> R<()> {
  println!("Generating diff files...");
  let file_list_path = Path::new(checkout_path).join("diff_file_list.txt");
  let mut file_list = File::create(&file_list_path) .unwrap();

  diffs.0.iter().for_each(|d| {
      writeln!(file_list, "{}", d.file_name).unwrap(); // TODO: Do we want to wrap this error?

      let diff_file_name = format!("{}.diff", d.file_name);
      let diff_file = Path::new(checkout_path).join(&diff_file_name);

      let mut f = File::create(&diff_file).unwrap(); // TODO: Do we want to wrap this error?
      println!("Creating {}", &diff_file_name);
      let buf: &[u8]= d.contents.as_ref();
      f.write_all(buf).unwrap(); // TODO: Do we want to wrap this error?
  });

  Ok(())
}

fn get_process_output(command: &mut Command) -> R<CmdOutput> {
    let result =
      command
      .status()
      .map_err(|e| PursError::ProcessError(NestedError::from(e)));

    let l = result.map(|r|{
        if r.success() {
            CmdOutput::Success
        } else {
            r.code()
            .map(|c| CmdOutput::Failure(ExitCode::Code(c)))
            .unwrap_or(CmdOutput::Failure(ExitCode::Terminated))
        }
    });

    l
}

// TODO: We can't do anything with "default" here. Replace
fn get_extract_path(config: &Config, pull: &PullRequest) -> String {
    let repo_name = pull.repo_name.clone().unwrap_or("default".to_string());
    let separator = format!("{}", std::path::MAIN_SEPARATOR);
    vec![config.working_dir.to_string_lossy().to_string(), repo_name, pull.branch_name.clone(), pull.head_sha.clone()].join(&separator).to_string()
}

// async fn get_prs2(config: &Config, octocrab: &Octocrab) -> octocrab::Result<Vec<PullRequest>> {

//     //Use only the first for now.

//     let OwnerRepo(owner, repo) = config.repositories.head();
//     let page = octocrab
//     .pulls(owner.0.to_owned(), repo.0.to_owned())
//     .list()
//     // Optional Parameters
//     .state(params::State::Open)
//     .sort(params::pulls::Sort::Created)
//     .direction(params::Direction::Descending)
//     .per_page(20)
//     .send()
//     .await?;

//     let mut results = vec![];
//     for pull in page {
//         let title = pull.title.clone().unwrap_or("-".to_string());
//         let pr_no = pull.number;
//         // let diff_url = pull.diff_url.clone().map(|u| u.to_string()).unwrap_or("-".to_string());
//         let ssh_url = pull.head.repo.clone().and_then(|r| (r.ssh_url));
//         let head_sha = pull.head.sha;
//         let repo_name = pull.head.repo.clone().and_then(|r| r.full_name);
//         let branch_name = pull.head.ref_field;
//         let base_sha = pull.base.sha;

//         let review_count_handle = tokio::spawn(get_reviews2(octocrab.clone(), owner.clone(), repo.clone(), pr_no));
//         let comment_count_handle = tokio::spawn(get_comments2(octocrab.clone(), owner.clone(), repo.clone(), pr_no));
//         let diffs_handle = tokio::spawn(get_pr_diffs2(octocrab.clone(), owner.clone(), repo.clone(), pr_no));

//         let res = tokio::try_join!(
//             flatten(review_count_handle),
//             flatten(comment_count_handle),
//             flatten(diffs_handle)
//         );


//         match res  {
//             Ok((review_count, comment_count, diffs)) => {
//                 results.push(
//                     PullRequest {
//                         title,
//                         pr_number: pr_no,
//                         ssh_url,
//                         branch_name,
//                         head_sha,
//                         repo_name,
//                         base_sha,
//                         review_count,
//                         comment_count,
//                         diffs
//                     }
//                 );
//             },
//             Err(e) => println!("Could not retrieve PR: {}/{} #{}, cause: {}", owner.0.to_owned(), repo.0.to_owned(), pr_no, e)
//         }
//     }

//     Ok(results)
// }

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

async fn get_prs3(config: &Config, octocrab: Octocrab) -> R<Vec<PullRequest>> {

    let page_handles =
      config
      .repositories
      .to_vec()
      .into_iter()
      .map(|owner_repo| {
        let handle=
            tokio::task::spawn(
          get_pulls(
                  octocrab.clone(), owner_repo.clone()
                )
                .map(|hr| { hr.map(|h| (h, owner_repo)) }) //write a help function for this
            );
            handle
      }).collect::<Vec<_>>();

    let page_results =
      try_join_all(page_handles)
      .await
      .map_err( PursError::from)?;

    let page_repos =
      page_results
      .into_iter()
      .map(|rp| rp.unwrap())
      .collect::<Vec<_>>();

    let async_parts = page_repos.into_iter().map(|(page, OwnerRepo(owner, repo))| {
            page.into_iter().map(|pull| {
                let pr_no = pull.number;
                let review_count_handle = tokio::spawn(get_reviews2(octocrab.clone(), owner.clone(), repo.clone(), pr_no));
                let comment_count_handle = tokio::spawn(get_comments2(octocrab.clone(), owner.clone(), repo.clone(), pr_no));
                let diffs_handle = tokio::spawn(get_pr_diffs2(octocrab.clone(), owner.clone(), repo.clone(), pr_no));

                AsyncPullRequestParts {
                    pull,
                    review_count_handle,
                    comment_count_handle,
                    diffs_handle
                }
            }).collect::<Vec<_>>()
    }).collect::<Vec<_>>();

    let parts = async_parts.into_iter().flatten().collect::<Vec<_>>();
    let parts_stream = stream::iter(parts);

    let pr_stream =
        parts_stream.then(|AsyncPullRequestParts { pull, review_count_handle, comment_count_handle, diffs_handle }|{
            async move {
                let res = tokio::try_join!(
                    flatten(review_count_handle),
                    flatten(comment_count_handle),
                    flatten(diffs_handle)
                );

                match res {
                  Ok((review_count, comment_count, diffs)) => {

                    let pr_no = pull.number;
                    let title = pull.title.clone().unwrap_or("-".to_string());
                    let ssh_url = pull.head.repo.clone().and_then(|r| (r.ssh_url));
                    let head_sha = pull.head.sha;
                    let repo_name = pull.head.repo.clone().and_then(|r| r.full_name);
                    let branch_name = pull.head.ref_field;
                    let base_sha = pull.base.sha;

                    let pr =
                      PullRequest {
                        title,
                        pr_number: pr_no,
                        ssh_url,
                        branch_name,
                        head_sha,
                        repo_name,
                        base_sha,
                        review_count,
                        comment_count,
                        diffs
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

// async fn get_prs(config: &Config, octocrab: &Octocrab) -> octocrab::Result<Vec<PullRequest>> {

//     //Use only the first for now.

//     let OwnerRepo(owner, repo) = config.repositories.head();
//     let page = octocrab
//     .pulls(owner.0.to_owned(), repo.0.to_owned())
//     .list()
//     // Optional Parameters
//     .state(params::State::Open)
//     .sort(params::pulls::Sort::Created)
//     .direction(params::Direction::Descending)
//     .per_page(20)
//     .send()
//     .await?;

//     let mut results = vec![];
//     for pull in page {
//         let title = pull.title.clone().unwrap_or("-".to_string());
//         let pr_no = pull.number;
//         // let diff_url = pull.diff_url.clone().map(|u| u.to_string()).unwrap_or("-".to_string());
//         let ssh_url = pull.head.repo.clone().and_then(|r| (r.ssh_url));
//         let head_sha = pull.head.sha;
//         let repo_name = pull.head.repo.clone().and_then(|r| r.full_name);
//         let branch_name = pull.head.ref_field;
//         let base_sha = pull.base.sha;

//         let review_count = get_reviews(octocrab, &owner, &repo, pr_no).await?;
//         let comment_count = get_comments(octocrab, &owner, &repo, pr_no).await?;
//         let diffs = get_pr_diffs(octocrab, &owner, &repo, pr_no).await?;

//         results.push(
//             PullRequest {
//                 title,
//                 pr_number: pr_no,
//                 ssh_url,
//                 branch_name,
//                 head_sha,
//                 repo_name,
//                 base_sha,
//                 review_count,
//                 comment_count,
//                 diffs
//             }
//         )
//     }

//     Ok(results)
// }

async fn flatten<T>(handle: tokio::task::JoinHandle<R<T>>) -> R<T> {
    match handle.await {
        Ok(result) => result,
        Err(err) => Err(PursError::from(err)),
    }
}


//TODO: Return Result with an error if the diff can't be parsed.
fn parse_diffs(diff: &str) -> R<PullRequestDiff> {
  let mut patch = PatchSet::new();
  let parse_result = patch.parse(diff).map_err(PursError::from);

  parse_result.map(|_| {
      let diffs = patch.files().into_iter().map (|p| {
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

// async fn get_reviews(octocrab: &Octocrab, owner: &Owner, repo: &Repo, pr_no: u64) -> octocrab::Result<usize> {
//     let reviews =
//         octocrab
//         .pulls(owner.0.to_owned(), repo.0.to_owned())
//         .list_reviews(pr_no).await?;

//     Ok(reviews.into_iter().count())
// }

async fn get_reviews2(octocrab:  Octocrab, owner:  Owner, repo:  Repo, pr_no: u64) -> R<usize> {
    let reviews =
        octocrab
        .pulls(owner.0.to_owned(), repo.0.to_owned())
        .list_reviews(pr_no)
        .await?;

    Ok(reviews.into_iter().count())
}

// async fn get_comments(octocrab: &Octocrab, owner: &Owner, repo: &Repo, pr_no: u64) -> octocrab::Result<usize> {
//     let comments =
//         octocrab
//         .pulls(owner.0.to_owned(), repo.0.to_owned())
//         .list_comments(Some(pr_no))
//         .send()
//         .await?;

//     Ok(comments.into_iter().count())
// }

async fn get_comments2(octocrab: Octocrab, owner: Owner, repo: Repo, pr_no: u64) -> R<usize> {
    let comments =
        octocrab
        .pulls(owner.0.to_owned(), repo.0.to_owned())
        .list_comments(Some(pr_no))
        .send()
        .await?;

    Ok(comments.into_iter().count())
}

// async fn get_pr_diffs(octocrab: &Octocrab, owner: &Owner, repo: &Repo, pr_no: u64) -> octocrab::Result<PullRequestDiff> {
//     let diff_string =
//         octocrab
//         .pulls(owner.0.to_owned(), repo.0.to_owned())
//         .get_diff(pr_no)
//         .await?;

//     Ok(parse_diffs(&diff_string))
// }

async fn get_pr_diffs2(octocrab: Octocrab, owner: Owner, repo: Repo, pr_no: u64) -> R<PullRequestDiff> {
    let diff_string =
        octocrab
        .pulls(owner.0.to_owned(), repo.0.to_owned())
        .get_diff(pr_no)
        .await?;

    parse_diffs(&diff_string)
}



// fn get_dummy_prs() -> octocrab::Result<Vec<PullRequest>> {
//     vec![
//         PullRequest {
//             title: "TITLE1".to_string(),
//             pr_number: 100,
//             ssh_url: Some("ssh1".to_string()),
//             repo_name: Some("repo1".to_string()),
//             branch_name: "branch1".to_string(),
//             head_sha: "sha1".to_string(),
//             base_sha: "base-sha1".to_string(),
//         },
//         PullRequest {
//             title: "TITLE2".to_string(),
//             pr_number: 200,
//             ssh_url: Some("ssh2".to_string()),
//             repo_name: Some("repo2".to_string()),
//             branch_name: "branch2".to_string(),
//             head_sha: "sha2".to_string(),
//             base_sha: "base-sha2".to_string(),
//         },
//         PullRequest {
//             title: "TITLE3".to_string(),
//             pr_number: 300,
//             ssh_url: Some("ssh3".to_string()),
//             repo_name: Some("repo3".to_string()),
//             branch_name: "branch3".to_string(),
//             head_sha: "sha3".to_string(),
//             base_sha: "base-sha3".to_string(),
//         }
//     ]

// }

pub fn print_error(message: String) {
  let coloured_error = Colour::Red.paint(format!("Error: {}", message));
  println!("{}", coloured_error)
}

pub fn print_info(message: String) {
  let coloured_info = Colour::Green.paint(format!("{}", message));
  println!("{}", coloured_info)
}

// fn write_file_out<P>(filename: P, working_dir: &str, pull: &PullRequest) -> io::Result<()>
// where P: AsRef<Path> + Copy {
//     let lines = read_lines(filename).expect(&format!("Could not read lines from {}", filename.as_ref().to_string_lossy()));

//     let mut files_to_open = vec![];
//     for line_r in lines {
//         let file = line_r.expect("Could not read line");
//         let path = Path::new(working_dir).join(format!("{}.diff", file));
//         let diff_file = File::create(&path).expect(&format!("Could not create file: {}", path.as_path().to_string_lossy()));

//         let mut diff_command = Command::new("git");
//         diff_command
//          .current_dir(working_dir)
//          .stdout(diff_file)
//          .arg("diff")
//          .arg(format!("{}..{}", &pull.base_sha, &pull.head_sha))
//          .arg("--")
//          .arg(&file);

//          diff_command.status().expect(&format!("Could not write out file: {}", path.as_path().to_string_lossy()));
//          files_to_open.push(path);
//     }

//     let mut sublime_command = Command::new("s");
//     sublime_command
//     .arg(working_dir)
//     .arg("-n");

//     files_to_open.iter().for_each(|f| {
//         sublime_command.arg(f);
//     });


//     sublime_command.status().expect("Could not launch Sublime Text");
//     Ok(())
// }

// TODO: Have an external script specified, which is given the working directory of the checkout.
// With that and the contents of the diff_files.txt file it should be able to figure out
// Anything it needs.
// fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
// where P: AsRef<Path>, {
//     let file = File::open(filename)?;
//     Ok(io::BufReader::new(file).lines())
// }

// fn run_sbt_tests(working_dir: &str) -> io::Result<()> {
//     let mut sbt_command = Command::new("sbt");
//     sbt_command
//     .current_dir(working_dir)
//     .arg("test");

//     sbt_command.status().expect("Running SBT tests failed");
//     Ok(())
// }

// fn launch_sbt(working_dir: &str) -> io::Result<()> {
//     let mut sbt_command = Command::new("sbt");
//     sbt_command
//     .current_dir(working_dir)
//     .arg("-mem")
//     .arg("2048");

//     sbt_command.status().expect("Running SBT failed");
//     Ok(())
// }
