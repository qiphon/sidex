//! Git blame — per-line attribution.

use std::path::Path;

use serde::Serialize;

use crate::cmd::run_git;
use crate::error::{GitError, GitResult};

/// A single line of blame output.
#[derive(Debug, Clone, Serialize)]
pub struct BlameLine {
    pub commit_hash: String,
    pub author: String,
    pub date: String,
    pub line_number: usize,
    pub line_text: String,
}

/// Run `git blame` on a file and parse the output.
pub fn blame(repo_root: &Path, path: &Path) -> GitResult<Vec<BlameLine>> {
    let path_str = path.to_string_lossy();
    let output = run_git(repo_root, &["blame", "--porcelain", "--", &path_str])?;
    parse_porcelain_blame(&output)
}

/// Parse `git blame --porcelain` output.
fn parse_porcelain_blame(output: &str) -> GitResult<Vec<BlameLine>> {
    let mut lines_out = Vec::new();
    let mut current_hash = String::new();
    let mut current_line_number: usize = 0;
    let mut current_author = String::new();
    let mut current_date = String::new();

    for line in output.lines() {
        if line.starts_with('\t') {
            // Content line — this is the actual source line.
            lines_out.push(BlameLine {
                commit_hash: current_hash.clone(),
                author: current_author.clone(),
                date: current_date.clone(),
                line_number: current_line_number,
                line_text: line.strip_prefix('\t').unwrap_or(line).to_string(),
            });
        } else if let Some(rest) = line.strip_prefix("author ") {
            current_author = rest.to_string();
        } else if let Some(rest) = line.strip_prefix("author-time ") {
            current_date = rest.to_string();
        } else {
            // Could be a commit header: "<hash> <orig_line> <final_line> [<num_lines>]"
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 3
                && parts[0].len() >= 7
                && parts[0].chars().all(|c| c.is_ascii_hexdigit())
            {
                current_hash = parts[0].to_string();
                current_line_number = parts[2]
                    .parse()
                    .map_err(|_| GitError::Parse(format!("invalid line number: {}", parts[2])))?;
            }
        }
    }

    Ok(lines_out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_porcelain_blame_works() {
        let input = "\
abc1234567890 1 1 2\n\
author Alice\n\
author-mail <alice@example.com>\n\
author-time 1700000000\n\
author-tz +0000\n\
committer Alice\n\
committer-mail <alice@example.com>\n\
committer-time 1700000000\n\
committer-tz +0000\n\
summary initial\n\
filename hello.rs\n\
\tfn main() {}\n\
abc1234567890 2 2\n\
\t// end\n";

        let lines = parse_porcelain_blame(input).unwrap();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].line_number, 1);
        assert_eq!(lines[0].author, "Alice");
        assert_eq!(lines[0].line_text, "fn main() {}");
        assert_eq!(lines[1].line_number, 2);
        assert_eq!(lines[1].line_text, "// end");
    }
}
