use octocrab::{self, OctocrabBuilder, Octocrab};
use octocrab::params;
use crate::model::*;
use std::io::{self, BufRead};
use std::path::Path;
use std::process::Command;
use ansi_term::Colour;


mod model;

#[tokio::main]
async fn main() -> octocrab::Result<()> {

    let token = std::env::var("GH_ACCESS_TOKEN").expect("Could not find Github Personal Access Token");
    let config = Config{ working_dir: Path::new("/Users/sanj/ziptemp/prs") };

    let octocrab =
        OctocrabBuilder::new()
        .personal_token(token)
        .build()?;


    let result = get_prs(&octocrab).await?;
    // let result = get_dummy_prs();
    let length = result.len();

    for (index, pr) in result.clone().into_iter().enumerate() {
        println!("{:>2} - {}", index + 1, pr);
    }

    match read_user_response("Please select a PR to clone to 'q' to exit", length) {
        Ok(response) => {
            println!("You said: {:?}", response);
            match response {
                UserSelection::Number(selection) => {
                    let selected = result.get(usize::from(selection - 1)).expect("Invalid index");
                    println!("git clone {:?} -b {}", &selected.ssh_url, &selected.branch_name);
                    println!("sha: {}", &selected.head_sha);
                    match clone_branch(&config, &selected) {
                      Ok(CmdOutput(Some(stdout))) => println!("{}", stdout),
                      Ok(CmdOutput(None)) => {},
                      Err(e) => print_error(e.to_string())
                    }
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

enum UserInputError {
    InvalidNumber(String),
    InvalidSelection{
        selected: u8,
        min_selection: u8,
        max_selection: usize
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

struct CmdOutput(Option<String>);


fn clone_branch(config: &Config, pull: &PullRequest) -> io::Result<CmdOutput> {
  match &pull.ssh_url {
      Some(ssh_url) => {
          let checkout_path = get_extract_path(&config, &pull);

          println!("checkout path: {}", checkout_path);
          let mut command = Command::new("git") ;
            command
            .arg("clone")
            .arg(ssh_url)
            .arg("-b")
            // .arg(pull.branch_name.as_str())
            .arg(checkout_path.as_str());

          let result = command.output();

          match command.status() {
             Ok(exit_code) => {
              if exit_code.success() {
                let output = result.map(|r| String::from_utf8_lossy(&r.stdout).to_string());
                Ok(CmdOutput(output.ok()))
              } else {
                let command_error=
                  result
                  .map(|r| {
                    String::from_utf8_lossy(&r.stderr).to_string()
                  });

                let error_message = format!("Failed executing clone: {}", command_error.unwrap_or("<Could not retrieve stderr>".to_string()));
                Err(io::Error::new(io::ErrorKind::Other, error_message))
              }
            },
             Err(e) => {
              let error_message = format!("Failed executing clone: {}", e);
              Err(io::Error::new(io::ErrorKind::Other, error_message))
            }
          }

      },
      None => Err(io::Error::new(io::ErrorKind::Other, "Can't clone branch without SSH url"))
  }
}

struct Config <'a> {
    working_dir: &'a Path
}

fn get_extract_path(config: &Config, pull: &PullRequest) -> String {
    let repo_name = pull.repo_name.clone().unwrap_or("default".to_string());
    let separator = format!("{}", std::path::MAIN_SEPARATOR);
    vec![config.working_dir.to_string_lossy().to_string(), repo_name, pull.branch_name.clone(), pull.head_sha.clone()].join(&separator).to_string()
}

async fn get_prs(octocrab: &Octocrab) -> octocrab::Result<Vec<PullRequest>> {

    let page = octocrab
    .pulls("XAMPPRocky", "octocrab")
    .list()
    // Optional Parameters
    .state(params::State::Open)
    .sort(params::pulls::Sort::Created)
    .direction(params::Direction::Descending)
    .per_page(50)
    .send()
    .await?;

    let results =
        page.into_iter().map (|pull| {
            let title = pull.title.clone().unwrap_or("-".to_string());
            let pr_no = pull.number;
            // let diff_url = pull.diff_url.clone().map(|u| u.to_string()).unwrap_or("-".to_string());
            let ssh_url = pull.head.repo.clone().and_then(|r| (r.ssh_url));
            let head_sha = pull.head.sha;
            let repo_name = pull.head.repo.clone().and_then(|r| r.full_name);
            let branch_name = pull.head.ref_field;

            PullRequest {
                title,
                pr_number: pr_no,
                ssh_url,
                branch_name,
                head_sha,
                repo_name
            }
        })
        .collect::<Vec<PullRequest>>();

    Ok(results)
}

fn get_dummy_prs() -> Vec<PullRequest> {
    vec![
        PullRequest {
            title: "TITLE1".to_string(),
            pr_number: 100,
            ssh_url: Some("ssh1".to_string()),
            repo_name: Some("repo1".to_string()),
            branch_name: "branch1".to_string(),
            head_sha: "sha1".to_string(),
        },
        PullRequest {
            title: "TITLE2".to_string(),
            pr_number: 200,
            ssh_url: Some("ssh2".to_string()),
            repo_name: Some("repo2".to_string()),
            branch_name: "branch2".to_string(),
            head_sha: "sha2".to_string(),
        },
        PullRequest {
            title: "TITLE3".to_string(),
            pr_number: 300,
            ssh_url: Some("ssh3".to_string()),
            repo_name: Some("repo3".to_string()),
            branch_name: "branch3".to_string(),
            head_sha: "sha3".to_string(),
        }
    ]

}

pub fn print_error(message: String) {
  let coloured_error = Colour::Red.paint(format!("Error: {}", message));
  println!("{}", coloured_error)
}

