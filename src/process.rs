use crate::model::*;
use crate::log::*;
use std::process::Command;


pub fn script_to_run(script: &ScriptToRun, mode: &Mode, checkout_path: &RepoCheckoutPath) -> R<()> {
  let mut command = Command::new(script.to_string());
  command
    .arg(checkout_path.to_string()) //arg1 -> checkout dir
    .arg(mode.short_string()); //arg2 -> mode

   if let Mode::Review = mode {
      command.arg(DIFF_FILE_LIST); //arg3 -> diff file list
   };

   match command.status() {
    Ok(exit_status) => {
      if exit_status.success() {
        Ok(())
      } else {
        Err(
          PursError::ScriptExecutionError(ScriptErrorType::NonZeroResult(exit_status.to_string()))
        )
      }
    },
    Err(error) =>
      Err(
          PursError::ScriptExecutionError(ScriptErrorType::Error(NestedError::from(error)))
      )
  }
}

pub fn clone_branch(ssh_url: GitRepoSshUrl, checkout_path: RepoCheckoutPath, branch_name: RepoBranchName) -> R<()> {
    print_info(format!("git clone {} -b {} {}", ssh_url, branch_name, checkout_path));
    let mut command = Command::new("git") ;
      command
      .arg("clone")
      .arg(ssh_url)
      .arg("-b")
      .arg(branch_name.as_ref())
      .arg(checkout_path.as_ref());

    let git_clone_result = get_process_output(&mut command);

    let _ = match git_clone_result {
      Ok(CmdOutput::Success) => {}, //Success will be returned at the end of the function
      Ok(CmdOutput::Failure(exit_code)) => {
          match exit_code {
              ExitCode::Code(code) => return Err(PursError::GitError(format!("Git exited with exit code: {}", code))),
              ExitCode::Terminated => return Err(PursError::GitError("Git was terminated".to_string())),
          }
      },
      Err(e2) => {
        let e1 = PursError::GitError("Error running Git".to_string());
        return Err(PursError::MultipleErrors(vec![e1, e2]))
      },
    };

    Ok(())
}

fn get_process_output(command: &mut Command) -> R<CmdOutput> {
    let result =
      command
      .status()
      .map_err(|e| PursError::ProcessError(NestedError::from(e)));

    result.map(|r|{
        if r.success() {
            CmdOutput::Success
        } else {
            r.code()
            .map(|c| CmdOutput::Failure(ExitCode::Code(c)))
            .unwrap_or(CmdOutput::Failure(ExitCode::Terminated))
        }
    })
}
