use std::fmt;
use std::path::PathBuf;
use octocrab;
use tokio::task::JoinHandle;

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
    pub review_count_handle: JoinHandle<octocrab::Result<usize>>,
    pub comment_count_handle: JoinHandle<octocrab::Result<usize>>,
    pub diffs_handle: JoinHandle<octocrab::Result<PullRequestDiff>>
}


pub enum UserInputError {
    InvalidNumber(String),
    InvalidSelection{
        selected: u8,
        min_selection: u8,
        max_selection: usize
    }
}
