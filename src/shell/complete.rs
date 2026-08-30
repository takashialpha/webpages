//! What Tab offers.
//!
//! A command name in the first position, a path anywhere else, and an option
//! after a `-`. All three come out of the same places the shell would look to
//! run the line, so completion cannot offer something that would not work.

use crate::args::Completes;
use crate::commands::COMMANDS;
use crate::fs;

use super::Session;

/// The result of pressing Tab.
pub struct Completion {
    /// The input line, extended as far as is unambiguous.
    pub line: String,
    /// Every match, when there is more than one, for the shell to list.
    pub candidates: Vec<String>,
}

/// Completes the last word: a command name in the first position, a path
/// anywhere else.
pub fn complete(session: &Session, input: &str) -> Completion {
    let (head, word) = input
        .rfind(' ')
        .map_or(("", input), |space| input.split_at(space + 1));

    let candidates = if head.is_empty() {
        // Both, because running a program means typing a path to it. `bi`
        // and `bin/li` are on the way to the same thing.
        COMMANDS
            .iter()
            .map(|command| command.name.to_owned())
            .filter(|name| name.starts_with(word))
            .chain(path_candidates(session, word, Completes::Paths))
            .collect::<Vec<_>>()
    } else if word.starts_with('-') {
        flag_candidates(head, word)
    } else {
        path_candidates(session, word, wanted(head))
    };

    let Some(prefix) = common_prefix(&candidates) else {
        return Completion {
            line: input.to_owned(),
            candidates: Vec::new(),
        };
    };

    // One match is finished, so add what a shell would: a space, unless it is
    // a directory, where you carry on typing.
    let completed = if candidates.len() == 1 {
        let sole = prefix.as_str();
        if sole.ends_with('/') {
            format!("{head}{sole}")
        } else {
            format!("{head}{sole} ")
        }
    } else {
        format!("{head}{prefix}")
    };

    Completion {
        line: completed,
        candidates: if candidates.len() > 1 {
            candidates
        } else {
            Vec::new()
        },
    }
}

/// The options the command at the head of the line takes. Every one takes
/// `-h`.
fn flag_candidates(head: &str, word: &str) -> Vec<String> {
    let Some(name) = head.split_whitespace().next() else {
        return Vec::new();
    };
    let Some(command) = COMMANDS.iter().find(|command| command.name == name) else {
        return Vec::new();
    };

    let mut candidates = vec!["-h".to_owned(), "--help".to_owned()];
    for flag in command.spec.flags {
        candidates.push(format!("-{}", flag.short));
        if let Some(long) = flag.long {
            candidates.push(format!("--{long}"));
        }
    }
    candidates.retain(|candidate| candidate.starts_with(word));
    candidates
}

/// What that command's operands can be.
fn wanted(head: &str) -> Completes {
    head.split_whitespace()
        .next()
        .and_then(|name| COMMANDS.iter().find(|command| command.name == name))
        .map_or(Completes::Paths, |command| command.spec.completes)
}

/// Completions for a partial path, relative to the working directory.
fn path_candidates(session: &Session, word: &str, wanted: Completes) -> Vec<String> {
    if wanted == Completes::Nothing {
        return Vec::new();
    }

    let (dir, stem) = word
        .rfind('/')
        .map_or(("", word), |slash| word.split_at(slash + 1));

    let Some(segments) = fs::resolve(&session.cwd, dir) else {
        return Vec::new();
    };
    let Some(node) = fs::node_at(&segments) else {
        return Vec::new();
    };

    node.entries()
        .iter()
        .filter(|entry| entry.name.starts_with(stem))
        .filter(|entry| wanted != Completes::Dirs || matches!(entry.node, fs::Node::Dir(_)))
        .map(|entry| {
            let suffix = if matches!(entry.node, fs::Node::Dir(_)) {
                "/"
            } else {
                ""
            };
            format!("{dir}{}{suffix}", entry.name)
        })
        .collect()
}

/// The longest prefix shared by every candidate.
fn common_prefix(candidates: &[String]) -> Option<String> {
    let first = candidates.first()?;
    let mut end = first.len();

    for other in &candidates[1..] {
        end = first
            .char_indices()
            .zip(other.char_indices())
            .take_while(|((_, a), (_, b))| a == b)
            .map(|((index, a), _)| index + a.len_utf8())
            .last()
            .unwrap_or(0)
            .min(end);
    }
    first.get(..end).map(ToOwned::to_owned)
}
