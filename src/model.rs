use std::ffi::OsStr;
use std::path::PathBuf;
use std::fmt::{self, Display};
use std::error::Error;
use tokio::task::JoinHandle;

pub type R<T> = Result<T, PursError>;

#[derive(Debug, Clone)]
pub struct PullRequest {
    pub title : String,
    pub pr_number : u64,
    pub ssh_url: Option<String>,
    pub repo_name: Option<String>,
    pub branch_name: String,
    pub head_sha: String,
    pub base_sha: String,
    pub review_count: usize,
    pub comment_count: usize,
    pub diffs: PullRequestDiff
}

#[derive(Debug, Clone)]
pub struct PullRequestDiff(pub Vec<GitDiff>);

impl fmt::Display for PullRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}, PR#{} ({}🔍) ({}💬) [{}]", self.title, self.pr_number, self.review_count, self.comment_count, if self.ssh_url.is_none()  { "x" } else { "v" })
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

#[derive(Clone)]
pub struct Owner(pub String);

#[derive(Clone)]
pub struct Repo(pub String);

#[derive(Clone)]
pub struct OwnerRepo(pub Owner, pub Repo);

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
        let mut v = vec![];
        v.push(self.first.clone());
        v.append(&mut self.rest.clone());
        v
    }
}

pub struct Config {
    pub working_dir: PathBuf,
    pub repositories: NonEmptyVec<OwnerRepo>,
}

#[derive(Debug, Clone)]
pub struct GitDiff {
    pub file_name: String,
    pub contents: String
}


pub struct AsyncPullRequestParts {
    pub pull: octocrab::models::pulls::PullRequest,
    pub review_count_handle: JoinHandle<R<usize>>,
    pub comment_count_handle: JoinHandle<R<usize>>,
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
pub enum PursError {
    Octocrab(NestedError),
    JoinError(NestedError),
    GitError(String),
    PullRequestHasNoRepo(String),
    PullRequestHasNoSSHUrl(String),
    DiffParseError(NestedError),
    ProcessError(NestedError), // Maybe add more information about which process was being executed?
    MultipleErrors(Vec<PursError>),
    UserError(UserInputError)
}

impl Display for PursError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PursError::Octocrab(error) => write!(f, "PursError.Octocrab: {}", error),
            PursError::JoinError(error) => write!(f, "PursError.JoinError: {}", error),
            PursError::GitError(error) => write!(f, "PursError.GitError: {}", error),
            PursError::PullRequestHasNoRepo(error) => write!(f, "PursError.PullRequestHasNoRepo: {}", error),
            PursError::PullRequestHasNoSSHUrl(error) => write!(f, "PursError.PullRequestHasNoSSHUrl: {}", error),
            PursError::ProcessError(error) => write!(f, "PursError.ProcessError: {}", error),
            PursError::MultipleErrors(errors) => write!(f, "PursError.MultipleErrors: {:?}", errors),
            PursError::DiffParseError(error) => write!(f, "PursError.DiffParseError: {}", error),
            PursError::UserError(error) => write!(f, "PursError.UserError: {}", error),
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

pub enum ProgramStatus {
  UserQuit,
  CompletedSuccessfully
}

pub enum ValidSelection {
  Quit,
  Pr(PullRequest)
}

#[derive(Clone)]
pub struct GitRepoSshUrl(String);

impl AsRef<OsStr> for GitRepoSshUrl {
  fn as_ref(&self) -> &OsStr {
    println!("AsRef<OsStr> for GitRepoSshUrl");
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

#[derive(Clone)]
pub struct RepoBranchName(String);


impl Display for RepoBranchName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(f, "{}", self.0)
    }
}

impl AsRef<str> for RepoBranchName {
  fn as_ref(&self) -> &str {
    println!("AsRef<str> for RepoBranchName");
    &self.0
  }
}

impl RepoBranchName {
  pub fn new(branch: String) -> Self {
    RepoBranchName(branch)
  }
}
