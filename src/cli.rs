use crate::model::*;
use crate::user_dir::*;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::str::FromStr;

pub fn cli() -> Result<Config, CommandLineArgumentFailure> {

  const APPVERSION: &str = env!("CARGO_PKG_VERSION");

  let working_dir_help_text = format!("Optional working directory. Defaults to USER_HOME/{}", DEFAULT_WORKING_DIR);

  let comments_help_text = "Whether to generate comment files when there are comments. Not included by default.".to_owned();

  let script_help: &str =
    "Optional script to run after cloning repository\n\
     Parameters to script:\n\
     param1: checkout directory for the selected PR\n\
     param2: name of the file that has the names of all the changed files\n\
     \n\
     Eg. purs --repo owner/repo --script path/to/your/script
    ";

  let app =
    clap::Command::new("purs")
    .version(APPVERSION)
    .author("Sanj Sahayam")
    .about("List and checkout open Pull Requests on a GitHub repository")
    .arg(
        clap::Arg::new("repo")
            .short('r')
            .long("repo")
            .multiple_occurrences(true)
            .takes_value(true)
            .required(true)
            .help("one or more GitHub repositories to include in the form: <owner>/<repo>"),
    )
    .arg(
        clap::Arg::new("script")
            .short('s')
            .long("script")
            .takes_value(true)
            .next_line_help(true)
            .help(script_help)
    )
    .arg(
        clap::Arg::new("gh_token")
            .takes_value(true)
            .short('t')
            .long("token")
            .env_os(OsStr::new("GH_ACCESS_TOKEN"))
            .hide_env(true)//Don't display value of GH_ACCESS_TOKEN in help text
            .help("GitHub Access Token. Can also be supplied through the GH_ACCESS_TOKEN environment variable")
    )
    .arg(
        clap::Arg::new("working_dir")
            .short('w')
            .long("wd")
            .takes_value(true)
            .help(working_dir_help_text.as_str())
    )
    .arg(
        clap::Arg::new("comments")
            .short('c')
            .long("comments")
            .help(comments_help_text.as_str())
    );

  let matches = app.get_matches();

  if let Some(repos) = matches.values_of("repo") {

    let repositories_result = repos.map(|r| {
      let mut rit = r.split('/').take(2);
      let invalid_format_error = format!("Invalid repository format: {}", r);
      let error = CommandLineArgumentFailure::new(&invalid_format_error);
      let owner = rit.next().ok_or_else(|| error.clone())?;
      let repo = rit.next().ok_or( error)?;

      Ok(OwnerRepo(Owner(owner.to_owned()), Repo(repo.to_owned())))
    }).collect::<Result<Vec<_>, CommandLineArgumentFailure>>();

    let repositories_vec = repositories_result?;
    let no_repositories_supplied_error = "No repositories supplied";
    let repositories =
      NonEmptyVec::from_vec(repositories_vec)
      .ok_or_else(|| CommandLineArgumentFailure::new(no_repositories_supplied_error))?;


    let script_option =
      match matches.value_of("script") {
        Some(script) => {
          let script_path = PathBuf::from_str(script);
          match script_path {
            Ok(valid_script) => ScriptType::Script(ScriptToRun::new(&valid_script)),
            Err(e) => ScriptType::InvalidScript(script.to_string(), NestedError::from(e))
          }
        },
        None => ScriptType::NoScript
      };

    let script = match script_option {
        ScriptType::NoScript => Ok(None),
        ScriptType::Script(script_to_run) => Ok(Some(script_to_run)),
        ScriptType::InvalidScript(script_supplied, e) => {
          let error = format!("Invalid Script supplied: {}. Error:{}", script_supplied, e);
          Err(CommandLineArgumentFailure::new(&error))
        },
    }?;

    let working_dir = match matches.value_of("working_dir") {
      Some(custom_working_dir) => WorkingDirectory::new(Path::new(custom_working_dir)),
      None => {
        let home_dir = get_home_dir()?;
        let working_dir = home_dir.join(DEFAULT_WORKING_DIR);
        WorkingDirectory::new(&working_dir)
      }
    };

    match get_or_create_working_dir(&working_dir)? {
      WorkingDirectoryStatus::Exists => {},
      WorkingDirectoryStatus::Created => println!("created working directory: {}", working_dir),
    }
    let avatar_cache_dir = working_dir.avatar_cache_dir();

    let token =
      matches
      .value_of("gh_token")
      .ok_or_else( || CommandLineArgumentFailure::new("Could not find Github Personal Access Token"))
      .map(GitHubToken::new)?;


    let include_comments = matches.is_present("comments");

    let config =
      Config {
        working_dir,
        avatar_cache_dir,
        repositories,
        token,
        script,
        include_comments
      };

    Ok(config)
  } else {
    Err(CommandLineArgumentFailure::new("Invalid command line argument combination, expected at least one repository."))
  }
}
