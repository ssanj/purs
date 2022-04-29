use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{PathBuf, Path};
use std::fmt::{self, Display};
use std::error::Error;
use serde_json::error;
use tokio::task::JoinHandle;
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashMap;
use crate::tools::group_by;


pub type R<T> = Result<T, PursError>;

pub const DEFAULT_WORKING_DIR: &str = ".purs";
pub const DIFF_FILE_LIST: &str = "diff_file_list.txt";

#[derive(Debug, Clone)]
pub struct PullRequest {
    pub config_owner_repo: OwnerRepo,
    pub title : String,
    pub pr_number : u64,
    pub ssh_url: Option<String>,
    pub repo_name: Option<String>,
    pub branch_name: String,
    pub head_sha: String,
    pub base_sha: String,
    pub reviews: Reviews,
    pub comments: Comments,
    pub diffs: PullRequestDiff,
    pub draft: Option<bool>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct ValidatedPullRequest {
    pub config_owner_repo: OwnerRepo,
    pub title : String,
    pub pr_number : u64,
    pub ssh_url: GitRepoSshUrl,
    pub repo_name: Repo,
    pub branch_name: RepoBranchName,
    pub head_sha: String,
    pub base_sha: String,
    pub reviews: Reviews,
    pub comments: Comments,
    pub diffs: PullRequestDiff,
    pub draft: bool,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

impl fmt::Display for ValidatedPullRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let repo_name = &self.config_owner_repo.1.0;
        write!(f, "{}, PR#{} ({}🔍) ({}💬) [{}]", self.title, self.pr_number, self.reviews.count(), self.comments.count(), repo_name)
    }
}


#[derive(Debug, Clone)]
pub struct PullRequestDiff(pub Vec<GitDiff>);

impl fmt::Display for PullRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let repo_name = &self.config_owner_repo.1.0;
        write!(f, "{}, PR#{} ({}🔍) ({}💬) [{}]", self.title, self.pr_number, self.reviews.count(), self.comments.count(), repo_name)
    }
}


#[derive(Debug)]
pub enum UserSelection {
    Number(u8),
    Quit
}


pub enum CmdOutput {
  Success,
  Failure(ExitCode),
}

pub enum ExitCode {
    Code(i32),
    Terminated
}

#[derive(Clone, Debug)]
pub struct Owner(pub String);

impl Display for Owner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug)]
pub struct Repo(pub String);

impl Display for Repo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(f, "{}", self.0)
    }
}

#[derive(Clone, Debug)]
pub struct OwnerRepo(pub Owner, pub Repo);

impl Display for OwnerRepo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(f, "{}/{}", self.0.0, self.1.0)
    }
}

#[derive(Clone, Debug)]
pub struct NonEmptyVec<T> {
    first: T,
    rest: Vec<T>,
}

impl <T: Clone> NonEmptyVec<T> {
    #[allow(dead_code)]
    pub fn one(first: T) -> NonEmptyVec<T> {
        NonEmptyVec {
            first,
            rest: vec![]
        }
    }

    #[allow(dead_code)]
    pub fn new(first: T, rest: Vec<T>) -> NonEmptyVec<T> {
        NonEmptyVec {
            first,
            rest
        }
    }

    #[allow(dead_code)]
    pub fn head(&self) -> T {
        self.first.clone()
    }

    pub fn to_vec(&self) -> Vec<T> {
        let mut v = vec![self.first.clone()];
        v.append(&mut self.rest.clone());
        v
    }

    pub fn from_vec(other: Vec<T>) -> Option<Self> {
      match &other[..] {
        [h, t @ ..] => {
          Some(
            NonEmptyVec {
              first: h.clone(),
              rest: t.to_vec()
            }
          )
        },
        _ => None
      }
    }
}

#[derive(Debug)]
pub struct Config {
    pub working_dir: WorkingDirectory,
    pub repositories: NonEmptyVec<OwnerRepo>,
    pub token: GitHubToken,
    pub script: Option<ScriptToRun>
}

#[derive(Debug, Clone)]
pub struct GitDiff {
    pub file_name: String,
    pub contents: String
}


pub struct AsyncPullRequestParts {
    pub owner_repo: OwnerRepo,
    pub pull: octocrab::models::pulls::PullRequest,
    pub reviews_handle: JoinHandle<R<Reviews>>,
    pub comments_handle: JoinHandle<R<Comments>>,
    pub diffs_handle: JoinHandle<R<PullRequestDiff>>
}

#[derive(Debug)]
pub enum UserInputError {
    InvalidNumber(String),
    InvalidSelection{
        selected: u8,
        min_selection: u8,
        max_selection: usize
    }
}

impl Display for UserInputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
          UserInputError::InvalidNumber(number) => {
            write!(f, "User selected an invalid option. Required a number but got: {}", number)
          },
          UserInputError::InvalidSelection{ selected, min_selection, max_selection} => {
            write!(f, "User selected an invalid option: {} which is out of the expected range: [{}-{}]", selected, min_selection, max_selection)
          },
        }
      }
}

#[derive(Debug)]
pub struct NestedError(Box<dyn Error + Send + Sync>);

impl Display for NestedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(f, "{}", self)
    }
}

#[derive(Debug)]
pub enum ScriptErrorType {
  NonZeroResult(String),
  Error(NestedError)
}

impl Display for ScriptErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      match self {
        ScriptErrorType::NonZeroResult(error) => write!(f, "ScriptErrorType.NonZeroResult: {}", error),
        ScriptErrorType::Error(error) => write!(f, "ScriptErrorType.Error: {}", error),
      }
    }
}


#[derive(Debug)]
pub enum PursError {
    Octocrab(NestedError),
    JoinError(NestedError),
    GitError(String),
    DiffParseError(NestedError),
    ProcessError(NestedError), // Maybe add more information about which process was being executed?
    MultipleErrors(Vec<PursError>),
    UserError(UserInputError),
    ScriptExecutionError(ScriptErrorType),
    TUIError(NestedError),
    ReqwestError(NestedError)
}

impl Display for PursError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PursError::Octocrab(error) => write!(f, "PursError.Octocrab: {}", error),
            PursError::JoinError(error) => write!(f, "PursError.JoinError: {}", error),
            PursError::GitError(error) => write!(f, "PursError.GitError: {}", error),
            PursError::ProcessError(error) => write!(f, "PursError.ProcessError: {}", error),
            PursError::MultipleErrors(errors) => write!(f, "PursError.MultipleErrors: {:?}", errors),
            PursError::DiffParseError(error) => write!(f, "PursError.DiffParseError: {}", error),
            PursError::UserError(error) => write!(f, "PursError.UserError: {}", error),
            PursError::ScriptExecutionError(error) => write!(f, "PursError.ScriptExecutionError: {}", error),
            PursError::TUIError(error) => write!(f, "PursError.TUIError: {}", error),
            PursError::ReqwestError(error) => write!(f, "PursError.ReqwestError: {}", error),
        }
    }
}

type DynamicError = Box<dyn std::error::Error + Send + Sync>;

impl <E> From<E> for NestedError
  where E: Into<DynamicError>
{
  fn from(error: E) -> Self {
    NestedError(error.into())
  }
}

impl From<octocrab::Error> for PursError {
  fn from(error: octocrab::Error) -> Self {
    PursError::Octocrab(NestedError::from(error))
  }
}

impl From<unidiff::Error> for PursError {
  fn from(error: unidiff::Error) -> Self {
    PursError::DiffParseError(NestedError::from(error))
  }
}

impl From<tokio::task::JoinError> for PursError {
  fn from(error: tokio::task::JoinError) -> Self {
    PursError::JoinError(NestedError::from(error))
  }
}

impl From<reqwest::Error> for PursError {
  fn from(error: reqwest::Error) -> Self {
      PursError::ReqwestError(NestedError::from(error))
  }
}

pub enum ProgramStatus {
  UserQuit,
  CompletedSuccessfully
}

pub enum ValidSelection {
  Quit,
  Pr(ValidatedPullRequest)
}

#[derive(Debug,Clone)]
pub struct GitRepoSshUrl(String);

impl AsRef<OsStr> for GitRepoSshUrl {
  fn as_ref(&self) -> &OsStr {
    OsStr::new(&self.0)
  }
}

impl GitRepoSshUrl {
  pub fn new(repo: String) -> Self {
    GitRepoSshUrl(repo)
  }
}

impl Display for GitRepoSshUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(f, "{}", self.0)
    }
}


#[derive(Clone)]
pub struct RepoCheckoutPath(String);

impl RepoCheckoutPath {
  pub fn new(path: String) -> Self {
    RepoCheckoutPath(path)
  }
}

impl Display for RepoCheckoutPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(f, "{}", self.0)
    }
}

impl AsRef<str> for RepoCheckoutPath {
  fn as_ref(&self) -> &str {
    &self.0
  }
}

#[derive(Debug,Clone)]
pub struct RepoBranchName(String);


impl Display for RepoBranchName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(f, "{}", self.0)
    }
}

impl AsRef<str> for RepoBranchName {
  fn as_ref(&self) -> &str {
    &self.0
  }
}

impl RepoBranchName {
  pub fn new(branch: String) -> Self {
    RepoBranchName(branch)
  }
}


#[derive(Debug)]
pub enum ScriptType {
  NoScript,
  Script(ScriptToRun),
  InvalidScript(String, NestedError)
}

#[derive(Debug)]
pub struct ScriptToRun(PathBuf);

impl ScriptToRun {
  pub fn new(path: &Path) -> Self {
    ScriptToRun(path.to_path_buf())
  }
}

impl Display for ScriptToRun {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(f, "{}", self.0.to_string_lossy())
    }
}

#[derive(Debug)]
pub struct WorkingDirectory(PathBuf);

impl WorkingDirectory {
  pub fn new(working_dir: &Path) -> Self {
    WorkingDirectory(working_dir.to_path_buf())
  }
}

impl Display for WorkingDirectory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(f, "{}", self.0.to_string_lossy())
    }
}

#[derive(Debug)]
pub struct HomeDirectory(PathBuf);

impl HomeDirectory {
  pub fn new(home_dir: &Path) -> Self {
    HomeDirectory(home_dir.to_path_buf())
  }

  pub fn join(&self, arg: &str) -> PathBuf {
    self.0.join(arg)
  }
}

impl Display for HomeDirectory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(f, "{}", self.0.to_string_lossy())
    }
}

#[derive(Debug)]
pub struct GitHubToken(String);

impl GitHubToken {
  pub fn new(token: &str) -> Self {
    GitHubToken(token.to_string())
  }
}

impl Display for GitHubToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(f, "{}", self.0)
    }
}

#[derive(Clone)]
pub struct CommandLineArgumentFailure(String);

impl Display for CommandLineArgumentFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(f, "{}", self.0)
    }
}

impl CommandLineArgumentFailure {
  pub fn new(error: &str) -> Self {
    CommandLineArgumentFailure(error.to_string())
  }
}

#[derive(Debug)]
pub enum WorkingDirectoryStatus {
  Exists,
  Created
}

#[derive(Debug, Clone)]
pub enum ReviewState {
    Approved,
    Pending,
    ChangesRequested,
    Commented,
    Dismissed,
    Other
}

#[derive(Debug, Clone)]
pub struct Review {
  pub user: String,
  pub comment: Option<String>,
  pub state: ReviewState
}

#[derive(Debug, Clone)]
pub struct Reviews {
  pub reviews: Vec<Review>
}

impl Reviews {
  pub fn count(&self) -> usize {
    self.reviews.len()
  }

  pub fn reviewer_names(&self) -> HashSet<String> {
    self.reviews.iter().map(|r| r.user.clone()).collect()
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CommentId(u64);

impl CommentId {
  pub fn new(comment_id: u64) -> Self {
    CommentId(comment_id)
  }
}

#[derive(Debug, Clone)]
pub struct LineNumber(u64);

impl LineNumber {
  pub fn new(line_no: u64) -> Self {
    LineNumber(line_no)
  }
}

#[derive(Debug, Clone)]
pub struct Markdown(String);

impl Markdown {
  pub fn new(body: String) -> Self {
    Markdown(body)
  }
}


#[derive(Debug, Clone)]
pub struct Comment {
  pub comment_id: CommentId,
  pub diff_hunk: String,
  pub body: String,
  pub markdown_body: Option<Markdown>,
  pub author: User,
  pub comment_url: Url,
  pub line: Option<LineNumber>,
  pub file_name: FileName,
  pub in_reply_to_id: Option<CommentId>
}

impl Comment {
  pub fn update_markdown_body(self, markdown_body: String) -> Self {
    Comment {
      markdown_body: Some(Markdown(markdown_body)),
      ..self
    }
  }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct FileName(String);

impl FileName {
  pub fn new(file_name: String) -> Self {
    FileName(file_name)
  }
}


#[derive(Debug, Clone)]
pub struct Comments {
  pub comments: Vec<Comment>
}

impl Comments {
  pub fn count(&self) -> usize {
    self.comments.len()
  }

  pub fn is_empty(&self) -> bool {
    self.comments.is_empty()
  }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Url(String);

impl Url {
  pub fn new(url: String) -> Self {
    Url(url)
  }
}

#[derive(Debug, Clone)]
pub struct Base64Encoded(String);

impl Base64Encoded {
  pub fn new(value: String) -> Self {
    Base64Encoded(value)
  }
}

#[derive(Debug, Clone)]
pub struct User {
  name: String,
  gravatar: Url,
}

impl User {
  pub fn new(name: String, gravatar: Url) -> Self {
    User {
      name,
      gravatar,
    }
  }

  pub fn gravatar_url(self) -> Url {
    self.gravatar
  }
}



impl From<url::Url> for Url {
  fn from(url: url::Url) -> Self {
      Url::new(url.into())
  }
}

impl From<&Url> for String {
  fn from(url: &Url) -> Self {
      url.0.clone()
  }
}



#[derive(Serialize, Debug, PartialEq)]
pub struct CommentJson {
  pub user_name: String,
  pub user_icon: String,
  pub link: String,
  pub line: u64,
  pub body: String,
  pub file_name: String,
}

#[derive(Serialize, Debug, PartialEq)]
pub struct LineCommentsJson {
  pub line: u64,
  pub file_name: String,
  pub file_line_comments: Vec<CommentJson>
}

#[derive(Serialize, Debug, PartialEq)]
pub struct FileCommentsJson {
  pub file_name: String,
  pub file_comments: Vec<LineCommentsJson>
}

impl CommentJson {
  pub fn grouped_by_line(comments: Comments) -> Vec<FileCommentsJson> {
    let comments_with_lines = comments.comments.into_iter().filter_map(|c|{
        c.line.map(|cl|{
          CommentJson {
            user_name: c.author.name,
            user_icon: c.author.gravatar.0,
            link: c.comment_url.0,
            line: cl.0,
            body: c.body.clone(),
            file_name: c.file_name.0
          }
        })
    }).collect::<Vec<_>>();

  let file_comments: HashMap<String, Vec<CommentJson>> =
    group_by(comments_with_lines, |v| v.file_name.clone());

  file_comments
    .into_iter()
    .map(|(file_name, comments_in_file)| {
      let lined_comment_json: HashMap<u64, Vec<CommentJson>> =
        group_by(comments_in_file, |c| c.line);

      let line_comments_json: Vec<LineCommentsJson> =
        lined_comment_json
          .into_iter()
          .map(|(line, comment_json)| {
              LineCommentsJson {
                line,
                file_name: file_name.clone(),
                file_line_comments: comment_json
              }
          }).collect();

      FileCommentsJson {
        file_name: file_name.clone(),
        file_comments: line_comments_json
      }
    }).collect()
  }


  pub fn grouped_by_line_2(comments: Comments, avatars: HashMap<Url, Base64Encoded>) -> Vec<FileCommentsJson> {
    let comments_with_lines = comments.comments.into_iter().filter_map(|c|{
        c.line.map(|cl|{
          let x = avatars.get(&c.author.gravatar);
          CommentJson {
            user_name: c.author.name,
            user_icon: x.clone().map(|s| s.clone().0).unwrap_or("-".to_owned()), //TODO: Have a better default
            link: c.comment_url.0,
            line: cl.0,
            body: c.body.clone(),
            file_name: c.file_name.0
          }
        })
    }).collect::<Vec<_>>();

  let file_comments: HashMap<String, Vec<CommentJson>> =
    group_by(comments_with_lines, |v| v.file_name.clone());

  file_comments
    .into_iter()
    .map(|(file_name, comments_in_file)| {
      let lined_comment_json: HashMap<u64, Vec<CommentJson>> =
        group_by(comments_in_file, |c| c.line);

      let line_comments_json: Vec<LineCommentsJson> =
        lined_comment_json
          .into_iter()
          .map(|(line, comment_json)| {
              LineCommentsJson {
                line,
                file_name: file_name.clone(),
                file_line_comments: comment_json
              }
          }).collect();

      FileCommentsJson {
        file_name: file_name.clone(),
        file_comments: line_comments_json
      }
    }).collect()
  }
}

#[cfg(test)]
mod tests;
