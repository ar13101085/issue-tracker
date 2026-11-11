//! `it`: command line client for the issue tracker, modelled on `gh issue`.

mod client;
mod cmd;
mod config;
mod output;

use clap::{Args, Parser, Subcommand};

#[derive(Parser)]
#[command(name = "it", version, about = "Work with issues from the command line", arg_required_else_help = true)]
pub struct Cli {
    /// Server address, e.g. https://issues.example.com
    #[arg(long, global = true, env = "IT_HOST")]
    pub host: Option<String>,
    /// Repository as owner/name
    #[arg(short = 'R', long, global = true, env = "IT_REPO")]
    pub repo: Option<String>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Sign in, sign out and show who you are
    #[command(subcommand)]
    Auth(AuthCmd),
    /// Work with repositories
    #[command(subcommand)]
    Repo(RepoCmd),
    /// Work with issues
    #[command(subcommand)]
    Issue(IssueCmd),
    /// Work with labels
    #[command(subcommand)]
    Label(LabelCmd),
    /// Work with milestones
    #[command(subcommand)]
    Milestone(MilestoneCmd),
    /// Work with projects
    #[command(subcommand)]
    Project(ProjectCmd),
    /// Read your notifications
    #[command(subcommand)]
    Notification(NotificationCmd),
}

#[derive(Subcommand)]
pub enum AuthCmd {
    /// Sign in to a server and remember the token
    Login {
        /// Use this access token instead of asking for a password
        #[arg(long)]
        token: Option<String>,
        /// Username or email (asked for when omitted)
        #[arg(long, short)]
        username: Option<String>,
    },
    /// Show the server and account in use
    Status,
    /// Forget the token for this server and revoke it
    Logout,
}

#[derive(Subcommand)]
pub enum RepoCmd {
    /// List repositories you own or collaborate on
    List,
    /// Show one repository
    View {
        #[arg(long)]
        json: bool,
        /// Open it in the browser
        #[arg(long, short)]
        web: bool,
    },
    /// Create a repository under your account
    Create {
        name: String,
        #[arg(long, short, default_value = "")]
        description: String,
        #[arg(long)]
        private: bool,
    },
    /// Remember a repository so -R can be left out
    SetDefault { repo: String },
}

#[derive(Args)]
pub struct BodyArgs {
    /// Text of the body
    #[arg(long, short)]
    pub body: Option<String>,
    /// Read the body from a file ("-" for standard input)
    #[arg(long, short = 'F', conflicts_with = "body")]
    pub body_file: Option<String>,
}

#[derive(Subcommand)]
pub enum IssueCmd {
    /// List issues
    #[command(visible_alias = "ls")]
    List {
        /// open, closed or all
        #[arg(long, short, default_value = "open")]
        state: String,
        /// Only issues with this label (repeatable)
        #[arg(long, short)]
        label: Vec<String>,
        /// Only issues assigned to this user ("@me" for yourself)
        #[arg(long, short)]
        assignee: Option<String>,
        /// Only issues opened by this user
        #[arg(long, short = 'A')]
        author: Option<String>,
        /// Only issues in this milestone (title)
        #[arg(long, short)]
        milestone: Option<String>,
        /// Only issues of this type: Bug, Feature or Task
        #[arg(long = "type", short = 't')]
        kind: Option<String>,
        /// Search text, or any query the web search box accepts
        #[arg(long, short = 'S')]
        search: Option<String>,
        /// Most issues to show
        #[arg(long, short = 'L', default_value_t = 30)]
        limit: u64,
        #[arg(long)]
        json: bool,
        /// Open the list in the browser
        #[arg(long, short)]
        web: bool,
    },
    /// Show an issue
    View {
        number: i64,
        /// Also show the comments
        #[arg(long, short)]
        comments: bool,
        #[arg(long)]
        json: bool,
        #[arg(long, short)]
        web: bool,
    },
    /// Open a new issue
    #[command(visible_alias = "new")]
    Create {
        #[arg(long, short)]
        title: Option<String>,
        #[command(flatten)]
        body: BodyArgs,
        #[arg(long, short)]
        label: Vec<String>,
        #[arg(long, short)]
        assignee: Vec<String>,
        /// Milestone title
        #[arg(long, short)]
        milestone: Option<String>,
        #[arg(long = "type", short = 'T')]
        kind: Option<String>,
        /// Open the new issue page in the browser instead
        #[arg(long, short)]
        web: bool,
    },
    /// Change an issue
    Edit {
        number: i64,
        #[arg(long, short)]
        title: Option<String>,
        #[command(flatten)]
        body: BodyArgs,
        #[arg(long)]
        add_label: Vec<String>,
        #[arg(long)]
        remove_label: Vec<String>,
        #[arg(long)]
        add_assignee: Vec<String>,
        #[arg(long)]
        remove_assignee: Vec<String>,
        /// Milestone title
        #[arg(long, short, conflicts_with = "remove_milestone")]
        milestone: Option<String>,
        #[arg(long)]
        remove_milestone: bool,
        /// Bug, Feature, Task or "none"
        #[arg(long = "type")]
        kind: Option<String>,
    },
    /// Close an issue
    Close {
        number: i64,
        /// completed, "not planned" or duplicate
        #[arg(long, short, default_value = "completed")]
        reason: String,
        /// Leave a closing comment
        #[arg(long, short)]
        comment: Option<String>,
    },
    /// Reopen an issue
    Reopen {
        number: i64,
        #[arg(long, short)]
        comment: Option<String>,
    },
    /// Add a comment
    Comment {
        number: i64,
        #[command(flatten)]
        body: BodyArgs,
    },
    /// Delete an issue for good
    Delete {
        number: i64,
        /// Do not ask for confirmation
        #[arg(long)]
        yes: bool,
    },
    /// Pin an issue to the top of the list
    Pin { number: i64 },
    /// Unpin an issue
    Unpin { number: i64 },
    /// Mark an issue as blocked by another issue
    Block {
        number: i64,
        /// The issue that has to be finished first
        #[arg(long)]
        by: i64,
    },
    /// Remove a "blocked by" relationship
    Unblock {
        number: i64,
        /// The issue that no longer blocks it
        #[arg(long)]
        by: i64,
    },
    /// Lock the conversation
    Lock {
        number: i64,
        /// off-topic, "too heated", resolved or spam
        #[arg(long, short)]
        reason: Option<String>,
    },
    /// Unlock the conversation
    Unlock { number: i64 },
    /// Move an issue to another repository
    Transfer {
        number: i64,
        /// Destination as owner/name
        destination: String,
    },
    /// Issues assigned to you and opened by you
    Status,
}

#[derive(Subcommand)]
pub enum LabelCmd {
    /// List labels
    List {
        #[arg(long)]
        json: bool,
    },
    /// Create a label
    Create {
        name: String,
        /// Six hex digits; a default is used when omitted
        #[arg(long, short, default_value = "ededed")]
        color: String,
        #[arg(long, short, default_value = "")]
        description: String,
    },
    /// Rename or recolor a label
    Edit {
        name: String,
        #[arg(long, short)]
        name_new: Option<String>,
        #[arg(long, short)]
        color: Option<String>,
        #[arg(long, short)]
        description: Option<String>,
    },
    /// Delete a label
    Delete {
        name: String,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
pub enum MilestoneCmd {
    /// List milestones
    List {
        /// open, closed or all
        #[arg(long, short, default_value = "open")]
        state: String,
        #[arg(long)]
        json: bool,
    },
    /// Create a milestone
    Create {
        title: String,
        #[arg(long, short, default_value = "")]
        description: String,
        /// Due date as YYYY-MM-DD
        #[arg(long)]
        due: Option<String>,
    },
    /// Close a milestone
    Close { number: i64 },
    /// Reopen a milestone
    Reopen { number: i64 },
    /// Delete a milestone
    Delete {
        number: i64,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Subcommand)]
pub enum ProjectCmd {
    /// List projects
    List {
        #[arg(long)]
        json: bool,
    },
    /// Show a project's items by status
    View {
        number: i64,
        #[arg(long)]
        json: bool,
        #[arg(long, short)]
        web: bool,
    },
    /// Add an issue to a project
    ItemAdd { number: i64, issue: i64 },
    /// Move an issue to a status column ("none" clears it)
    ItemStatus { number: i64, issue: i64, status: String },
}

#[derive(Subcommand)]
pub enum NotificationCmd {
    /// List your notifications
    List {
        /// inbox, unread or done
        #[arg(long, short, default_value = "inbox")]
        filter: String,
        #[arg(long)]
        json: bool,
    },
    /// Mark everything as read
    Read,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    if let Err(message) = cmd::run(cli).await {
        eprintln!("{} {message}", output::red("error:"));
        std::process::exit(1);
    }
}
