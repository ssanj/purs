use std::path::PathBuf;

use avatar::get_or_create_avatar_file;
use model::*;

mod avatar;
mod model;
mod tools;

#[tokio::main]
async fn main() {
  // let user_id = UserId::new(3954178);
  let user_id = UserId::new(3426751);
  // let avatar_url = Url::new("https://avatars.githubusercontent.com/u/3954178?v=4".to_owned());
  let avatar_url = Url::new("https://avatars.xgithubusercontent.com/u/3426751?v=4".to_owned());
  let path = "/Users/sanj/.purs/.assets/avatars";
  let avatar_info =
    AvatarInfo::new(
      user_id,
      avatar_url,
      AvatarCacheDirectory::new(PathBuf::from(path))
    );
  let result = get_or_create_avatar_file(&avatar_info);

  match result.await {
    Ok(u) => println!("got url: {:?}", u),
    Err(e) => println!("got error: {}", e)
  }
}
