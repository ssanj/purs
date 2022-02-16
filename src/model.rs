use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct PullRequest {
    pub title : String,
    pub pr_number : u64,
    pub ssh_url: Option<String>,
    pub repo_name: Option<String>,
    pub branch_name: String,
    pub head_sha: String,
    pub base_sha: String,
}

impl fmt::Display for PullRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}, [{}] {}", self.title, self.pr_number, if self.ssh_url.is_none()  { "x" } else { "v" })
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
    rest: Vec<T>
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
}

pub struct Config {
    pub working_dir: PathBuf,
    pub repositories: NonEmptyVec<OwnerRepo>,
}
