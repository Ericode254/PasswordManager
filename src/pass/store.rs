use std::path::{Path, PathBuf};

/// A node in the password store tree (directory or .gpg entry).
#[derive(Debug, Clone)]
pub struct StoreNode {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub children: Vec<StoreNode>,
    pub expanded: bool,
}

/// A flattened, visible entry ready for list rendering.
#[derive(Debug, Clone)]
pub struct FlatEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
    pub depth: usize,
    pub expanded: bool,
    #[allow(dead_code)]
    pub has_children: bool,
}

/// Returns the password store directory, respecting `$PASSWORD_STORE_DIR`.
pub fn get_store_dir() -> PathBuf {
    std::env::var("PASSWORD_STORE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .expect("Could not determine home directory")
                .join(".password-store")
        })
}

/// Scans the password store directory and returns a sorted tree.
pub fn scan_store(store_dir: &Path) -> Vec<StoreNode> {
    if !store_dir.exists() {
        return Vec::new();
    }
    let mut nodes = Vec::new();
    scan_dir(store_dir, store_dir, &mut nodes);
    sort_nodes(&mut nodes);
    nodes
}

fn scan_dir(base: &Path, dir: &Path, nodes: &mut Vec<StoreNode>) {
    let mut entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(_) => return,
    };
    entries.sort_by_key(|e| e.file_name());

    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();

        // Skip hidden entries (.git, .gpg-id, etc.)
        if name.starts_with('.') {
            continue;
        }

        if path.is_dir() {
            let mut children = Vec::new();
            scan_dir(base, &path, &mut children);
            sort_nodes(&mut children);

            // Only include directories that contain at least one entry
            if !children.is_empty() {
                let rel = path.strip_prefix(base).unwrap_or(&path);
                nodes.push(StoreNode {
                    name,
                    path: rel.to_string_lossy().to_string(),
                    is_dir: true,
                    children,
                    expanded: true,
                });
            }
        } else if path.extension().and_then(|s| s.to_str()) == Some("gpg") {
            let rel = path.strip_prefix(base).unwrap_or(&path);
            let mut pass_path = rel.to_string_lossy().to_string();
            if pass_path.ends_with(".gpg") {
                pass_path.truncate(pass_path.len() - 4);
            }
            let display = name.strip_suffix(".gpg").unwrap_or(&name).to_string();
            nodes.push(StoreNode {
                name: display,
                path: pass_path,
                is_dir: false,
                children: Vec::new(),
                expanded: false,
            });
        }
    }
}

/// Directories first, then alphabetical (case-insensitive).
fn sort_nodes(nodes: &mut [StoreNode]) {
    nodes.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
}

/// Flattens the tree into a list of visible entries respecting expand/collapse state.
pub fn flatten_tree(nodes: &[StoreNode], depth: usize, out: &mut Vec<FlatEntry>) {
    for node in nodes {
        out.push(FlatEntry {
            name: node.name.clone(),
            path: node.path.clone(),
            is_dir: node.is_dir,
            depth,
            expanded: node.expanded,
            has_children: !node.children.is_empty(),
        });
        if node.is_dir && node.expanded {
            flatten_tree(&node.children, depth + 1, out);
        }
    }
}

/// Flattens the entire tree, ignoring expand/collapse state. Used for search, so
/// matches inside collapsed directories are still found.
pub fn flatten_tree_all(nodes: &[StoreNode], depth: usize, out: &mut Vec<FlatEntry>) {
    for node in nodes {
        out.push(FlatEntry {
            name: node.name.clone(),
            path: node.path.clone(),
            is_dir: node.is_dir,
            depth,
            expanded: node.expanded,
            has_children: !node.children.is_empty(),
        });
        if node.is_dir {
            flatten_tree_all(&node.children, depth + 1, out);
        }
    }
}

/// Toggles the expand state of the node at `path` in the tree. Returns `true` if found.
pub fn toggle_node(nodes: &mut [StoreNode], path: &str) -> bool {
    for node in nodes.iter_mut() {
        if node.path == path && node.is_dir {
            node.expanded = !node.expanded;
            return true;
        }
        if node.is_dir && toggle_node(&mut node.children, path) {
            return true;
        }
    }
    false
}

pub fn collapsed_paths(nodes: &[StoreNode]) -> Vec<String> {
    let mut paths = Vec::new();
    for node in nodes {
        if node.is_dir && !node.expanded {
            paths.push(node.path.clone());
        }
        paths.extend(collapsed_paths(&node.children));
    }
    paths
}

pub fn reveal_path(nodes: &mut [StoreNode], path: &str) {
    for node in nodes {
        if node.is_dir && path.starts_with(&format!("{}/", node.path)) {
            node.expanded = true;
            reveal_path(&mut node.children, path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_flatten_and_toggle() {
        let mut nodes = vec![
            StoreNode {
                name: "Work".into(),
                path: "Work".into(),
                is_dir: true,
                children: vec![StoreNode {
                    name: "email".into(),
                    path: "Work/email".into(),
                    is_dir: false,
                    children: vec![],
                    expanded: false,
                }],
                expanded: true,
            },
            StoreNode {
                name: "personal".into(),
                path: "personal".into(),
                is_dir: false,
                children: vec![],
                expanded: false,
            },
        ];

        let mut visible = Vec::new();
        flatten_tree(&nodes, 0, &mut visible);
        assert_eq!(visible.len(), 3);
        assert_eq!(visible[0].name, "Work");
        assert_eq!(visible[0].depth, 0);
        assert_eq!(visible[1].name, "email");
        assert_eq!(visible[1].depth, 1);
        assert_eq!(visible[2].name, "personal");
        assert_eq!(visible[2].depth, 0);

        // Toggle Work to collapsed
        let toggled = toggle_node(&mut nodes, "Work");
        assert!(toggled);
        assert!(!nodes[0].expanded);

        visible.clear();
        flatten_tree(&nodes, 0, &mut visible);
        assert_eq!(visible.len(), 2);
        assert_eq!(visible[0].name, "Work");
        assert_eq!(visible[1].name, "personal");
    }

    #[test]
    fn test_sort_nodes_order() {
        let mut nodes = vec![
            StoreNode {
                name: "zebra".into(),
                path: "zebra".into(),
                is_dir: false,
                children: vec![],
                expanded: false,
            },
            StoreNode {
                name: "alpha".into(),
                path: "alpha".into(),
                is_dir: true,
                children: vec![],
                expanded: false,
            },
            StoreNode {
                name: "beta".into(),
                path: "beta".into(),
                is_dir: false,
                children: vec![],
                expanded: false,
            },
        ];

        sort_nodes(&mut nodes);
        // Directories come first, then alphabetical
        assert_eq!(nodes[0].name, "alpha");
        assert!(nodes[0].is_dir);
        assert_eq!(nodes[1].name, "beta");
        assert!(!nodes[1].is_dir);
        assert_eq!(nodes[2].name, "zebra");
        assert!(!nodes[2].is_dir);
    }
}
