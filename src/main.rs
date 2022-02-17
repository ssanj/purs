use octocrab::{self, OctocrabBuilder, Octocrab};
use octocrab::params;
use crate::model::*;
use std::io::{self, BufRead};
use std::path::Path;
use std::process::Command;
use ansi_term::Colour;
use std::fs::File;

mod model;

#[tokio::main]
async fn main() -> octocrab::Result<()> {

    let token = std::env::var("GH_ACCESS_TOKEN").expect("Could not find Github Personal Access Token");
    let config =
        Config {
            working_dir: Path::new("/Users/sanj/ziptemp/prs").to_path_buf(),
            repositories: NonEmptyVec::one(OwnerRepo(Owner("XAMPPRocky".to_string()), Repo("octocrab".to_string())))
        };

    let octocrab =
        OctocrabBuilder::new()
        .personal_token(token)
        .build()?;


    let result = get_prs(&config, &octocrab).await?;
    // let result = get_dummy_prs();
    let length = result.len();

    for (index, pr) in result.clone().into_iter().enumerate() {
        println!("{:>2} - {}", index + 1, pr);
    }

    match read_user_response("Please select a PR to clone to 'q' to exit", length) {
        Ok(response) => {
            match response {
                UserSelection::Number(selection) => {
                    let selected = result.get(usize::from(selection - 1)).expect("Invalid index");
                    clone_branch(&config, &selected).unwrap()
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


fn clone_branch(config: &Config, pull: &PullRequest) -> io::Result<()> {
  match &pull.ssh_url {
      Some(ssh_url) => {
          let checkout_path = get_extract_path(&config, &pull);
          print_info(format!("git clone {} -b {} {}", ssh_url, pull.branch_name.as_str(), checkout_path.as_str()));
          let mut command = Command::new("git") ;
            command
            .arg("clone")
            .arg(ssh_url)
            .arg("-b")
            .arg(pull.branch_name.as_str())
            .arg(checkout_path.as_str());

          let output = get_process_output(&mut command);

          match output {
            Ok(CmdOutput::Success) => {
                println!("Generating diff files...");

                let file_list_path = Path::new(checkout_path.as_str()).join("diff_file_list.txt");
                let file_list = File::create(&file_list_path) .unwrap();


                let mut diff_command = Command::new("git");
                diff_command
                 .current_dir(checkout_path.as_str())
                 .stdout(file_list)
                 .arg("diff")
                 .arg("--name-only")
                 .arg(format!("{}..{}", &pull.base_sha, &pull.head_sha));

                let result = diff_command.status();
                result.expect("Could not generate diff file list");
                write_file_out(&file_list_path, checkout_path.as_str(), &pull).expect("Could not write out file list");
                // run_sbt_tests(checkout_path.as_str()).expect("Could not run sbt tests");
                // launch_sbt(checkout_path.as_str()).expect("Could not launch SBT repl");

                Ok(())
            },
            Ok(CmdOutput::Failure(exit_code)) => {
                match exit_code {
                    ExitCode::Code(code) => print_error(format!("Git exited with exit code: {}", code)),
                    ExitCode::Terminated => print_error("Git was terminated".to_string()),
                }

                Ok(())
            },
            Err(e) => {
                eprintln!("Could not run Git: {}", e);
                Ok(())
            },
          }
      },
      None => Err(io::Error::new(io::ErrorKind::Other, "Can't clone branch without SSH url"))
  }
}

fn get_process_output(command: &mut Command) -> io::Result<CmdOutput> {
    let result = command.status();

    let l = result.map(|r|{
        if r.success() {
            CmdOutput::Success
        } else {
            r.code()
            .map(|c| CmdOutput::Failure(ExitCode::Code(c)))
            .unwrap_or(CmdOutput::Failure(ExitCode::Terminated))
        }
    });

    l
}

fn get_extract_path(config: &Config, pull: &PullRequest) -> String {
    let repo_name = pull.repo_name.clone().unwrap_or("default".to_string());
    let separator = format!("{}", std::path::MAIN_SEPARATOR);
    vec![config.working_dir.to_string_lossy().to_string(), repo_name, pull.branch_name.clone(), pull.head_sha.clone()].join(&separator).to_string()
}

async fn get_prs(config: &Config, octocrab: &Octocrab) -> octocrab::Result<Vec<PullRequest>> {

    //Use only the first for now.

    let OwnerRepo(owner, repo) = config.repositories.head();
    let page = octocrab
    .pulls(owner.0, repo.0)
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
            let base_sha = pull.base.sha;

            PullRequest {
                title,
                pr_number: pr_no,
                ssh_url,
                branch_name,
                head_sha,
                repo_name,
                base_sha
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
            base_sha: "base-sha1".to_string(),
        },
        PullRequest {
            title: "TITLE2".to_string(),
            pr_number: 200,
            ssh_url: Some("ssh2".to_string()),
            repo_name: Some("repo2".to_string()),
            branch_name: "branch2".to_string(),
            head_sha: "sha2".to_string(),
            base_sha: "base-sha2".to_string(),
        },
        PullRequest {
            title: "TITLE3".to_string(),
            pr_number: 300,
            ssh_url: Some("ssh3".to_string()),
            repo_name: Some("repo3".to_string()),
            branch_name: "branch3".to_string(),
            head_sha: "sha3".to_string(),
            base_sha: "base-sha3".to_string(),
        }
    ]

}

pub fn print_error(message: String) {
  let coloured_error = Colour::Red.paint(format!("Error: {}", message));
  println!("{}", coloured_error)
}

pub fn print_info(message: String) {
  let coloured_info = Colour::Green.paint(format!("{}", message));
  println!("{}", coloured_info)
}

fn write_file_out<P>(filename: P, working_dir: &str, pull: &PullRequest) -> io::Result<()>
where P: AsRef<Path> + Copy {
    let lines = read_lines(filename).expect(&format!("Could not read lines from {}", filename.as_ref().to_string_lossy()));

    let mut files_to_open = vec![];
    for line_r in lines {
        let file = line_r.expect("Could not read line");
        let path = Path::new(working_dir).join(format!("{}.diff", file));
        let diff_file = File::create(&path).expect(&format!("Could not create file: {}", path.as_path().to_string_lossy()));

        let mut diff_command = Command::new("git");
        diff_command
         .current_dir(working_dir)
         .stdout(diff_file)
         .arg("diff")
         .arg(format!("{}..{}", &pull.base_sha, &pull.head_sha))
         .arg("--")
         .arg(&file);

         diff_command.status().expect(&format!("Could not write out file: {}", path.as_path().to_string_lossy()));
         files_to_open.push(path);
    }

    let mut sublime_command = Command::new("s");
    sublime_command
    .arg(working_dir)
    .arg("-n");

    files_to_open.iter().for_each(|f| {
        sublime_command.arg(f);
    });


    sublime_command.status().expect("Could not launch Sublime Text");
    Ok(())
}

fn read_lines<P>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>>
where P: AsRef<Path>, {
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}

fn run_sbt_tests(working_dir: &str) -> io::Result<()> {
    let mut sbt_command = Command::new("sbt");
    sbt_command
    .current_dir(working_dir)
    .arg("test");

    sbt_command.status().expect("Running SBT tests failed");
    Ok(())
}

fn launch_sbt(working_dir: &str) -> io::Result<()> {
    let mut sbt_command = Command::new("sbt");
    sbt_command
    .current_dir(working_dir)
    .arg("-mem")
    .arg("2048");

    sbt_command.status().expect("Running SBT failed");
    Ok(())
}
