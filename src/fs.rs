//! The content tree the shell walks.
//!
//! Content lives in `content/` as plain text and is pulled in at compile time,
//! so adding a section means adding a file and one line to [`ROOT`], never a new
//! command. `ls`, `cat`, `cd`, and tab completion all read this same tree, which
//! is what stops them drifting apart.

/// A named entry in a directory.
pub struct Entry {
    pub name: &'static str,
    pub node: Node,
}

/// Either a directory of further entries, or a file's text.
pub enum Node {
    Dir(&'static [Entry]),
    File(&'static str),
}

impl Node {
    /// Looks up a single name in this node. Files have no children.
    #[must_use]
    pub fn child(&self, name: &str) -> Option<&'static Self> {
        match *self {
            Self::Dir(entries) => entries
                .iter()
                .find(|entry| entry.name == name)
                .map(|entry| &entry.node),
            Self::File(_) => None,
        }
    }

    /// The entries of a directory, or an empty slice for a file.
    #[must_use]
    pub const fn entries(&self) -> &'static [Entry] {
        match *self {
            Self::Dir(entries) => entries,
            Self::File(_) => &[],
        }
    }
}

/// The opening paragraph of `about.txt`, reused verbatim by the login banner and
/// the meta description. Splitting it out here means the intro is written once.
#[must_use]
pub fn intro() -> &'static str {
    const ABOUT: &str = include_str!("../content/about.txt");
    ABOUT.split("\n\n").next().unwrap_or(ABOUT).trim_end()
}

pub const ROOT: Node = Node::Dir(&[
    Entry {
        name: "about.txt",
        node: Node::File(include_str!("../content/about.txt")),
    },
    Entry {
        name: "rust.txt",
        node: Node::File(include_str!("../content/rust.txt")),
    },
    Entry {
        name: "linux.txt",
        node: Node::File(include_str!("../content/linux.txt")),
    },
    Entry {
        name: "infra.txt",
        node: Node::File(include_str!("../content/infra.txt")),
    },
    Entry {
        name: "design.txt",
        node: Node::File(include_str!("../content/design.txt")),
    },
    Entry {
        name: "now.txt",
        node: Node::File(include_str!("../content/now.txt")),
    },
    Entry {
        name: "contact.txt",
        node: Node::File(include_str!("../content/contact.txt")),
    },
    Entry {
        name: "projects",
        node: Node::Dir(&[
            Entry {
                name: "audium",
                node: Node::File(include_str!("../content/projects/audium")),
            },
            Entry {
                name: "swagsh",
                node: Node::File(include_str!("../content/projects/swagsh")),
            },
            Entry {
                name: "carboxyl",
                node: Node::File(include_str!("../content/projects/carboxyl")),
            },
            Entry {
                name: "webpages",
                node: Node::File(include_str!("../content/projects/webpages")),
            },
            Entry {
                name: "niri-takashialpha",
                node: Node::File(include_str!("../content/projects/niri-takashialpha")),
            },
        ]),
    },
]);

/// Resolves a path against a working directory, handling `.`, `..`, a leading
/// `/` or `~`, and rejecting anything that walks past the root.
///
/// Returns the resolved segments, which the caller can turn back into a node
/// with [`node_at`].
#[must_use]
pub fn resolve(cwd: &[&'static str], path: &str) -> Option<Vec<&'static str>> {
    let mut segments = if path.starts_with('/') || path.starts_with('~') {
        Vec::new()
    } else {
        cwd.to_vec()
    };

    for part in path.split('/') {
        match part {
            "" | "." | "~" => {}
            ".." => {
                segments.pop();
            }
            name => {
                // Only names that exist in the tree can enter the path, which is
                // what keeps the returned segments `'static`.
                let here = node_at(&segments)?;
                let entry = here.entries().iter().find(|entry| entry.name == name)?;
                segments.push(entry.name);
            }
        }
    }
    Some(segments)
}

/// Walks resolved segments back to the node they name.
#[must_use]
pub fn node_at(segments: &[&str]) -> Option<&'static Node> {
    let mut node = &ROOT;
    for segment in segments {
        node = node.child(segment)?;
    }
    Some(node)
}

/// Renders a path the way the prompt and `pwd` show it, rooted at `~`.
#[must_use]
pub fn display_path(segments: &[&str]) -> String {
    if segments.is_empty() {
        "~".to_owned()
    } else {
        format!("~/{}", segments.join("/"))
    }
}
