use crate::types::{Folder, Repo};

#[derive(Debug, Clone)]
pub enum TreeNodeKind {
    Folder(Folder),
    Repo(Repo),
    Uncategorized,
}

#[derive(Debug, Clone)]
pub struct TreeNode {
    pub kind: TreeNodeKind,
    pub depth: usize,
    pub expanded: bool,
    pub children_loaded: bool,
    pub children: Vec<TreeNode>,
}

impl TreeNode {
    #[must_use]
    pub fn uncategorized() -> Self {
        Self {
            kind: TreeNodeKind::Uncategorized,
            depth: 0,
            expanded: true,
            children_loaded: true,
            children: Vec::new(),
        }
    }

    #[must_use]
    pub fn folder(folder: Folder, depth: usize) -> Self {
        Self {
            kind: TreeNodeKind::Folder(folder),
            depth,
            expanded: false,
            children_loaded: false,
            children: Vec::new(),
        }
    }

    #[must_use]
    pub fn repo(repo: Repo, depth: usize) -> Self {
        Self {
            kind: TreeNodeKind::Repo(repo),
            depth,
            expanded: false,
            children_loaded: true,
            children: Vec::new(),
        }
    }

    #[must_use]
    pub fn folder_id(&self) -> Option<&str> {
        match &self.kind {
            TreeNodeKind::Folder(f) => Some(&f.id),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_folder(&self) -> bool {
        matches!(self.kind, TreeNodeKind::Folder(_) | TreeNodeKind::Uncategorized)
    }

    #[must_use]
    pub fn name(&self) -> &str {
        match &self.kind {
            TreeNodeKind::Folder(f) => &f.name,
            TreeNodeKind::Repo(r) => &r.name,
            TreeNodeKind::Uncategorized => "[Uncategorized]",
        }
    }

}

#[derive(Debug, Clone)]
pub struct FlatNode {
    pub node: TreeNode,
}

pub fn build_tree(root_folders: &[Folder], repos: &[Repo]) -> Vec<TreeNode> {
    let mut tree = Vec::new();

    let uncategorized_repos: Vec<_> = repos.iter().filter(|r| r.folder_id.is_none()).collect();
    if !uncategorized_repos.is_empty() {
        let mut uncategorized = TreeNode::uncategorized();
        for repo in uncategorized_repos {
            uncategorized.children.push(TreeNode::repo(repo.clone(), 1));
        }
        uncategorized
            .children
            .sort_by_key(|a| a.name().to_lowercase());
        tree.push(uncategorized);
    }

    let mut sorted_folders: Vec<_> = root_folders.to_vec();
    sorted_folders.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    for folder in sorted_folders {
        tree.push(TreeNode::folder(folder, 0));
    }

    tree
}

pub fn load_children_by_folder_id(
    tree: &mut [TreeNode],
    folder_id: &str,
    child_folders: &[Folder],
    child_repos: &[Repo],
) -> bool {
    load_children_by_id_recursive(tree, folder_id, child_folders, child_repos)
}

fn load_children_by_id_recursive(
    tree: &mut [TreeNode],
    folder_id: &str,
    child_folders: &[Folder],
    child_repos: &[Repo],
) -> bool {
    for node in tree {
        if node.folder_id() == Some(folder_id) {
            if !node.children_loaded {
                let depth = node.depth + 1;

                let mut sorted_folders: Vec<_> = child_folders.to_vec();
                sorted_folders.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                for folder in sorted_folders {
                    node.children.push(TreeNode::folder(folder, depth));
                }

                let mut sorted_repos: Vec<_> = child_repos.to_vec();
                sorted_repos.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
                for repo in sorted_repos {
                    node.children.push(TreeNode::repo(repo.clone(), depth));
                }

                node.children_loaded = true;
            }
            return true;
        }

        // Search in children (even if not expanded, we might be preloading)
        if load_children_by_id_recursive(&mut node.children, folder_id, child_folders, child_repos)
        {
            return true;
        }
    }
    false
}

#[must_use]
pub fn flatten_tree(tree: &[TreeNode]) -> Vec<FlatNode> {
    let mut flat = Vec::new();
    for node in tree {
        flatten_node(node, &mut flat);
    }
    flat
}

fn flatten_node(node: &TreeNode, flat: &mut Vec<FlatNode>) {
    flat.push(FlatNode { node: node.clone() });

    if node.expanded {
        for child in &node.children {
            flatten_node(child, flat);
        }
    }
}

pub fn set_all_expanded(tree: &mut [TreeNode], expanded: bool) {
    for node in tree {
        if node.is_folder() {
            node.expanded = expanded;
            set_all_expanded(&mut node.children, expanded);
        }
    }
}

pub fn toggle_expanded_at(tree: &mut [TreeNode], target_index: usize) -> bool {
    let mut current_index = 0;
    toggle_expanded_recursive(tree, target_index, &mut current_index)
}

fn toggle_expanded_recursive(tree: &mut [TreeNode], target_index: usize, current_index: &mut usize) -> bool {
    for node in tree {
        if *current_index == target_index {
            if node.is_folder() {
                node.expanded = !node.expanded;
                return true;
            }
            return false;
        }
        *current_index += 1;

        if node.expanded
            && toggle_expanded_recursive(&mut node.children, target_index, current_index)
        {
            return true;
        }
    }
    false
}

pub fn set_expanded_at(tree: &mut [TreeNode], target_index: usize, expanded: bool) -> bool {
    let mut current_index = 0;
    set_expanded_recursive(tree, target_index, expanded, &mut current_index)
}

fn set_expanded_recursive(
    tree: &mut [TreeNode],
    target_index: usize,
    expanded: bool,
    current_index: &mut usize,
) -> bool {
    for node in tree {
        if *current_index == target_index {
            if node.is_folder() {
                node.expanded = expanded;
                return true;
            }
            return false;
        }
        *current_index += 1;

        if node.expanded
            && set_expanded_recursive(&mut node.children, target_index, expanded, current_index)
        {
            return true;
        }
    }
    false
}

#[must_use]
pub fn get_node_at(tree: &[TreeNode], target_index: usize) -> Option<&TreeNode> {
    let mut current_index = 0;
    get_node_recursive(tree, target_index, &mut current_index)
}

fn get_node_recursive<'a>(tree: &'a [TreeNode], target_index: usize, current_index: &mut usize) -> Option<&'a TreeNode> {
    for node in tree {
        if *current_index == target_index {
            return Some(node);
        }
        *current_index += 1;

        if node.expanded {
            if let Some(found) = get_node_recursive(&node.children, target_index, current_index) {
                return Some(found);
            }
        }
    }
    None
}

#[must_use]
pub fn find_parent_index(tree: &[TreeNode], target_index: usize) -> Option<usize> {
    let flat = flatten_tree(tree);
    let target_node = flat.get(target_index)?;
    let target_depth = target_node.node.depth;

    if target_depth == 0 {
        return None;
    }

    (0..target_index)
        .rev()
        .find(|&i| flat[i].node.depth == target_depth.saturating_sub(1) && flat[i].node.is_folder())
}
