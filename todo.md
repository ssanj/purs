# Purs


## Features

- Write each run command
- Only show info on reviews and comments if they exist. That makes it easier to find.


## Design

- Add a legend with the options to the Details section when there are no selections:
  - Q to quit
  - Up/Down arrows to select a PR
  - Left arrow to move out of select mode
  - O to open the selected PR in your default browser
  - H to copy Head SHA to the clipboard
  - B to copy base SHA to the clipboard
  - C to copy curl to retrieve PR content
  - U to copy clone url to clipboard
- Add a way to checkout the PR without the diffs or comments
  There seem to be two use cases here:
  - Check out the PR to review it (default -> Enter)
  - Check out the PR to address review comments. (We don't want diffs and comments in this view)
- Add a way to fold comments
- Add a way to quickly jump to comments in a file
- Covert the markdown content of comments to html via GH
