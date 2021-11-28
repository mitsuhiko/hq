use clap::Parser;

/// hq is like jq but for HTML data.
#[derive(Parser, Debug)]
pub struct Cli {
    /// A css expression
    #[clap(short = 'f', long = "filter", multiple_occurrences = true)]
    filters: Vec<String>,
    /// An action
    #[clap(short = 'a', long = "action", multiple_occurrences = true)]
    actions: Vec<String>,
}
