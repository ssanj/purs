# Purs


## Design

### More Detail
- Possibly dump all the detailed information to a separate file like pr_details.txt

### Legend

- Add a legend with the options to the Details section when there are no selections:
  - Q to quit
  - Up/Down arrows to select a PR
  - Left arrow to move out of select mode
  - O to open the selected PR in your default browser
  - H to copy Head SHA to the clipboard
  - B to copy base SHA to the clipboard
  - C to copy curl to retrieve PR content
  - U to copy clone url to clipboard

### Modes of operation

- Add a way to checkout the PR without the diffs or comments
- Option to specify a PR number along with the user/repo and skip the TUI and download it directly
- Option to download a branch by hash (obviously without any PR info, comments etc)

### Comments

- Add a way to fold comments
- Add a way to quickly jump to comments in a file
- Add the avatar url to the comment dump

### Logs

#### Clean up

- AvatarCacheDirectory("/Users/sanj/.purs/.assets/.avatars")), AvatarInfo(UserId(3954178), Url("https://
avatars.githubusercontent.com/u/3954178?v=4"), AvatarCacheDirectory("/Users/sanj/.purs/.assets/.avatar
s"))}


#### Add
- Write each run command
