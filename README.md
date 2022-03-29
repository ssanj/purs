# Purs

Checkout Pull Requests with ease

## Usage

To just list the open pull request in a repository use:

```
purs --repo owner1/repo1
```

NB: You should export your GitHub Personal Token via the `GH_ACCESS_TOKEN` environment variable or supply it to purs via the `-t` option.

This will display a list of up to twenty open pull requests from the repository supplied. You can then choose from the list and the following will happen:

- The PR branch will be cloned into a directory under your PURS_HOME directory. This defaults to `~/.purs` in the following format:

`PURS_HOME/repo_owner/repository/branch_name/branch_HEAD_hash`

- A diff file will be created with names of all the files that have changed from the parent branch. The files changed will created in a Git diff format with the same name as the original file but with an addition `.diff` suffix.

For example if the `README.md` file was updated, the diff file would be named `README.md.diff`

You can also supply multiple repositories along with a script to execute once the chose pull request has been cloned and diffed.

If supplied the script will be supplied with two parameters:
1. The directory the pull request was checked out to
1. The name of the diff file that holds the changed file names

A complete list of options can be found with:

```
purs --help
```

which yields:

```
purs 0.2.0
Sanj Sahayam
List and checkout open Pull Requests on a GitHub repository

USAGE:
    purs [OPTIONS] --repo <repo>

OPTIONS:
    -h, --help
            Print help information

    -r, --repo <repo>
            one or more GitHub repositories to include in the form: <owner>/<repo>

    -s, --script <script>
            Optional script to run after cloning repository
            Parameters to script:
            param1: checkout directory for the selected PR
            param2: name of the file that has the names of all the changed files

            Eg. purs --repo owner/repo --script path/to/your/script

    -t, --token <gh_token>
            GitHub Access Token. Can also be supplied through the GH_ACCESS_TOKEN environment
            variable

    -V, --version
            Print version information

    -w, --wd <working_dir>
            Optional working directory. Defaults to USER_HOME/.purs
```
