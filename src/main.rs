use octocrab::{self, OctocrabBuilder, Octocrab};
use crate::model::*;

use std::time::Instant;
use tui_app::render_tui;
use avatar::get_avatars;
use cli::cli;
use github::{get_prs3, render_markdown_comments};
use process::{script_to_run, clone_branch};
use file_tools::get_extract_path;
use file_writer::{write_diff_files, write_comment_files};

mod model;
mod cli;
mod user_dir;
mod console;
mod tui_app;
mod github;
mod tools;
mod file_tools;
mod avatar;
mod log;
mod process;
mod file_writer;

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


fn handle_user_selection_tui(pulls: Vec<ValidatedPullRequest>) -> R<ValidSelection> {
  render_tui(pulls)
}
