use octocrab::{self, OctocrabBuilder};
use octocrab::params;

#[tokio::main]
async fn main() -> octocrab::Result<()> {

    let token = std::env::var("GH_ACCESS_TOKEN").expect("Could not find Github Personal Access Token");

    let octocrab =
        OctocrabBuilder::new()
        .personal_token(token)
        .build()?;

    // Returns the first page of all issues.
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

    for (index, pull) in page.into_iter().enumerate() {
        let title = pull.title.clone().unwrap_or("-".to_string());
        let pr_no = pull.number;
        // let diff_url = pull.diff_url.clone().map(|u| u.to_string()).unwrap_or("-".to_string());
        let ssh_url = pull.head.repo.clone().and_then(|r| (r.ssh_url));
        let brach_name = pull.head.ref_field;
        let pull_req =
            PullRequest {
                title,
                pr_number: pr_no,
                ssh_url,
                brach_name
            };

        println!("{:>2} - {}", index, pull_req);
    }

    Ok(())
}

#[derive(Debug)]
struct PullRequest {
    title : String,
    pr_number : u64,
    ssh_url: Option<String>,
    brach_name: String
}

impl std::fmt::Display for PullRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}, [{}] {}", self.title, self.pr_number, if self.ssh_url.is_none()  { "x" } else { "v" })
    }
}




