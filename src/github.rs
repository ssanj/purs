use futures::FutureExt;
use futures::future::{try_join_all, join_all};
use octocrab::{self, Octocrab, Page};
use octocrab::params;
use octocrab::models::pulls::ReviewState as GHReviewState;
use crate::model::*;
use unidiff::PatchSet;
use futures::stream::{self, StreamExt};
use crate::tools::partition;
use std::time::Instant;
use std::collections::HashMap;
use async_trait::async_trait;

type PageHandles = Vec<tokio::task::JoinHandle<Result<(octocrab::Page<octocrab::models::pulls::PullRequest>, OwnerRepo), PursError>>>;
type PullRequestsAndOwner = R<Vec<(octocrab::Page<octocrab::models::pulls::PullRequest>, OwnerRepo)>>;

#[async_trait]
pub trait GitHubT {
  async fn get_prs(&'static self, config: &Config) -> R<Vec<PullRequest>>;
  async fn get_reviews(&self, owner_repo: OwnerRepo, pr_no: u64) -> R<Reviews>;
  async fn get_pulls(&self, owner_repo: OwnerRepo) -> R<PullRequest>;
  async fn get_diffs(&self, owner_repo: OwnerRepo, pr_no: u64) -> R<DiffString>;
  // async fn get_diffs(&self, owner: Owner, repo: Repo, pr_no: u64) -> R<PullRequestDiff>;
  async fn get_comments(&self, owner_repo: OwnerRepo, pr_no: u64) -> R<Comments>;
  async fn render_markdown(&self, content: String) -> R<Markdown>;
}

struct OctocrabGitHub {
  api: Octocrab
}

impl OctocrabGitHub {
  async fn get_pull_requests_with_owner(&self, config: &Config) -> PullRequestsAndOwner {
    let page_handles:PageHandles  =
      config
      .repositories
      .to_vec()
      .into_iter()
      .map(|owner_repo| {
        tokio::task::spawn(
      get_pulls(
              self.api.clone(), owner_repo.clone()
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

    Ok(page_repos)
  }

  fn enhance_pull_request(&'static self, page_repos: &Vec<(Page<octocrab::models::pulls::PullRequest>, OwnerRepo)>) -> Vec<AsyncPullRequestParts> {
    let async_parts = page_repos.iter().map(|(page, owner_repo)| {
            page.into_iter().map(|pull| {
                let pr_no = pull.number;
                let reviews_handle = tokio::spawn(self.get_reviews(owner_repo.clone(), pr_no));
                let comments_handle = tokio::spawn(self.clone().get_comments(owner_repo.clone(), pr_no));
                let diff_string_handle = tokio::spawn(self.clone().get_diffs(owner_repo.clone(), pr_no));

                AsyncPullRequestParts {
                    owner_repo: owner_repo.clone(),
                    pull: pull.clone(),
                    reviews_handle,
                    comments_handle,
                    diffs_handle: None, //TODO: Fix
                    diff_string_handle: Some(diff_string_handle)
                }
            }).collect::<Vec<_>>()
    });

    let parts: Vec<AsyncPullRequestParts> = async_parts.flatten().collect::<Vec<_>>();
    parts
  }
}

#[async_trait]
impl GitHubT for OctocrabGitHub {
  async fn get_reviews(&self, owner_repo: OwnerRepo, pr_no: u64) -> R<Reviews> {
    let octocrab = self.api.clone();

    let gh_reviews =
        octocrab
        .pulls(owner_repo.0.0.to_owned(), owner_repo.1.0.to_owned())
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

  async fn get_comments(&self, owner_repo: OwnerRepo, pr_no: u64) -> R<Comments> {
    let octocrab = self.api.clone();

    let comments =
        octocrab
        .pulls(owner_repo.owner().to_string(), owner_repo.repo().to_string())
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

  async fn get_diffs(&self, owner_repo: OwnerRepo, pr_no: u64) -> R<DiffString> {
    let octocrab = self.api.clone();

    let diff_string =
      octocrab
      .pulls(owner_repo.owner().to_string(), owner_repo.repo().to_string())
      .get_diff(pr_no)
      .await?;

    Ok(DiffString::new(diff_string))
  }

  async fn render_markdown(&self, content: String) -> R<Markdown> {
    let octocrab = self.api.clone();

    octocrab
      .markdown()
      .render_raw(content)
      .await
      .map_err(PursError::from)
      .map(Markdown::new)

  }

  async fn get_prs(&'static self, config: &Config) -> R<Vec<PullRequest>> {
    let page_repos = self.get_pull_requests_with_owner(config).await?;
    let parts = self.enhance_pull_request(&page_repos);
    let parts_stream = stream::iter(parts);

    let pr_stream =
        parts_stream.then(|AsyncPullRequestParts { owner_repo, pull, reviews_handle, comments_handle, diffs_handle, diff_string_handle }|{
            async move {
                let res = tokio::try_join!(
                    flatten(reviews_handle),
                    flatten(comments_handle),
                    flatten(diff_string_handle.unwrap()) //TODO: Fix
                );

                match res {
                  Ok((reviews, comments, diff_string)) => {

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
                    let diffs =  parse_diffs(diff_string)?;

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

  async fn get_pulls(&self, owner_repo: OwnerRepo) -> R<PullRequest> {todo!() }
}

//-------------------------------------------------------------------------------------------

async fn get_pull_requests_with_owner(config: &Config, octocrab: &Octocrab) -> PullRequestsAndOwner {
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

    Ok(page_repos)
}

fn enhance_pull_request(page_repos: &Vec<(Page<octocrab::models::pulls::PullRequest>, OwnerRepo)>, octocrab: &Octocrab) -> Vec<AsyncPullRequestParts> {
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
                    diffs_handle: Some(diffs_handle),
                    diff_string_handle: None

                }
            }).collect::<Vec<_>>()
    });

    let parts: Vec<AsyncPullRequestParts> = async_parts.flatten().collect::<Vec<_>>();
    parts
}

pub async fn get_prs3(config: &Config, octocrab: Octocrab) -> R<Vec<PullRequest>> {
    let page_repos = get_pull_requests_with_owner(config, &octocrab).await?;
    let parts = enhance_pull_request(&page_repos, &octocrab);
    let parts_stream = stream::iter(parts);

    let pr_stream =
        parts_stream.then(|AsyncPullRequestParts { owner_repo, pull, reviews_handle, comments_handle, diffs_handle, diff_string_handle }|{
            async move {
                let res = tokio::try_join!(
                    flatten(reviews_handle),
                    flatten(comments_handle),
                    flatten(diffs_handle.unwrap()) //TODO: Fix
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

    parse_diffs(DiffString::new(diff_string))
}

//Nothing to do with Octocrab
fn parse_diffs(diff_string: DiffString) -> R<PullRequestDiff> {
  let mut patch = PatchSet::new();
  let parse_result = patch.parse(diff_string.to_string()).map_err(PursError::from);

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

      PullRequestDiff::new(diffs)
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
