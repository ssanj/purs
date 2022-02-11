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


pub struct CmdOutput {
  pub stdout: Option<String>,
  pub stderr: Option<String>,
}

// impl fmt::Display for CmdOutput {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         writeln!(f, "stdout: {}", self.stdout.as_ref().unwrap_or(&"-".to_string()))?;
//         writeln!(f, "stderr: {}", self.stderr.as_ref().unwrap_or(&"-".to_string()))
//     }
// }



impl CmdOutput {

  pub fn new(stdout: Option<String>, stderr: Option<String>) -> CmdOutput {
    CmdOutput {
      stdout,
      stderr,
    }
  }

  pub fn with_stdout(stdout: Option<String>) -> CmdOutput {
    CmdOutput {
      stdout,
      stderr: None,
    }
  }

  pub fn with_stderr(stderr: Option<String>) -> CmdOutput {
    CmdOutput {
      stdout: None,
      stderr,
    }
  }
}
