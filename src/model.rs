use std::fmt;

#[derive(Debug, Clone)]
pub struct PullRequest {
    pub title : String,
    pub pr_number : u64,
    pub ssh_url: Option<String>,
    pub repo_name: Option<String>,
    pub branch_name: String,
    pub head_sha: String
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
