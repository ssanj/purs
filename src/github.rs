use futures::FutureExt;
use futures::future::{try_join_all, join_all};
use octocrab::{self, Octocrab};
use octocrab::params;
use octocrab::models::pulls::ReviewState as GHReviewState;
use crate::model::*;
use unidiff::PatchSet;
use futures::stream::{self, StreamExt};
use crate::tools::partition;
use std::time::Instant;
use std::collections::HashMap;

type PageHandles = Vec<tokio::task::JoinHandle<Result<(octocrab::Page<octocrab::models::pulls::PullRequest>, OwnerRepo), PursError>>>;

pub async fn get_prs3(config: &Config, octocrab: Octocrab) -> R<Vec<PullRequest>> {
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

    //TODO: Replace with partition
   let (pr_successes, pr_errors, ) = partition(results_with_errors);

    if pr_errors.is_empty() {
      Ok(pr_successes)
    } else {
      Err(PursError::MultipleErrors(pr_errors))
    }
}

//TODO check for unnecessary memory allocations
pub async fn render_markdown_comments(octocrab: &Octocrab, comments: &Comments) -> R<Comments> {
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

fn create_user(user: Option<&octocrab::models::User>) -> Option<User> {
  user.map(From::from)
}

async fn flatten<T>(handle: tokio::task::JoinHandle<R<T>>) -> R<T> {
    match handle.await {
        Ok(result) => result,
        Err(err) => Err(PursError::from(err)),
    }
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


async fn render_markdown(octocrab: Octocrab, content: String) -> R<String> {
  octocrab
    .markdown()
    .render_raw(content)
    .await
    .map_err(PursError::from)
}

fn flatten_results<T, E, E2, F>(nested_results: Result<Result<T, E>, E2>, f: F) -> Result<T, E>
  where F: FnOnce(E2) -> E
{
  nested_results.map_err(f).and_then(std::convert::identity)
}
