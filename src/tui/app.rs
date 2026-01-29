use std::io;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, backend::CrosstermBackend};

use crate::cli::credentials::load_credentials;
use crate::cli::http_client::{ApiClient, NamespaceWithPrimary};
use crate::types::{Folder, Repo};

use super::actions;
use super::tree::{
    TreeNode, TreeNodeKind, build_tree, find_parent_index, flatten_tree, get_node_at,
    load_children_by_folder_id, set_all_expanded, set_expanded_at, toggle_expanded_at,
};
use super::ui;

const STATUS_TIMEOUT: Duration = Duration::from_secs(3);

#[derive(Debug, Clone)]
pub struct StatusMessage {
    pub message: String,
    pub is_error: bool,
    pub created_at: Instant,
}

#[derive(Debug, Clone)]
pub enum InputMode {
    Normal,
    CreatingFolder {
        parent_id: Option<String>,
    },
    RenamingFolder {
        folder_id: String,
    },
    ConfirmDelete {
        folder_id: String,
        name: String,
        has_children: bool,
    },
    MovingItem {
        source_index: usize,
        move_target_index: usize,
        move_targets: Vec<MoveTarget>,
    },
}

#[derive(Debug, Clone)]
pub struct MoveTarget {
    pub folder_id: Option<String>,
    pub tree_index: usize,
}

// Commands sent from UI to worker
enum WorkerCommand {
    Connect {
        initial_workspace: Option<String>,
    },
    RefreshData {
        namespace: String,
    },
    LoadFolderChildren {
        folder_id: String,
        namespace: String,
    },
    CreateFolder {
        name: String,
        parent_id: Option<String>,
        namespace: Option<String>,
    },
    RenameFolder {
        folder_id: String,
        new_name: String,
    },
    DeleteFolder {
        folder_id: String,
        recursive: bool,
    },
    MoveFolder {
        folder_id: String,
        new_parent_id: Option<String>,
    },
    MoveRepo {
        repo_id: String,
        folder_id: Option<String>,
    },
}

// Results sent from worker to UI
enum WorkerResult {
    Connected {
        namespaces: Vec<NamespaceWithPrimary>,
        initial_namespace_idx: usize,
    },
    ConnectionError {
        message: String,
    },
    DataRefreshed {
        folders: Vec<Folder>,
        repos: Vec<Repo>,
    },
    FolderChildrenLoaded {
        folder_id: String,
        child_folders: Vec<Folder>,
    },
    FolderCreated {
        name: String,
    },
    FolderRenamed {
        new_name: String,
    },
    FolderDeleted,
    FolderMoved {
        name: String,
    },
    RepoMoved {
        name: String,
    },
    Error {
        message: String,
    },
}

pub struct App {
    pub running: bool,
    pub mode: InputMode,
    pub namespaces: Vec<NamespaceWithPrimary>,
    pub current_namespace_idx: usize,
    pub folders: Vec<Folder>,
    pub repos: Vec<Repo>,
    pub tree: Vec<TreeNode>,
    pub selected_index: usize,
    pub all_expanded: bool,
    pub input_buffer: String,
    pub status: Option<StatusMessage>,
    pub connecting: bool,
    pub loading: bool,
    pub connection_error: Option<String>,
    pending_folder_loads: Vec<String>,
    cmd_tx: mpsc::Sender<WorkerCommand>,
}

impl App {
    fn new(cmd_tx: mpsc::Sender<WorkerCommand>) -> Self {
        Self {
            running: true,
            mode: InputMode::Normal,
            namespaces: Vec::new(),
            current_namespace_idx: 0,
            folders: Vec::new(),
            repos: Vec::new(),
            tree: Vec::new(),
            selected_index: 0,
            all_expanded: false,
            input_buffer: String::new(),
            status: None,
            connecting: true,
            loading: false,
            connection_error: None,
            pending_folder_loads: Vec::new(),
            cmd_tx,
        }
    }

    #[must_use]
    pub fn current_namespace(&self) -> Option<&NamespaceWithPrimary> {
        self.namespaces.get(self.current_namespace_idx)
    }

    fn rebuild_tree(&mut self) {
        let was_expanded = self.all_expanded;
        self.tree = build_tree(&self.folders, &self.repos);

        if was_expanded {
            set_all_expanded(&mut self.tree, true);
        }

        let flat_len = flatten_tree(&self.tree).len();
        if self.selected_index >= flat_len {
            self.selected_index = flat_len.saturating_sub(1);
        }
    }

    pub fn set_status(&mut self, message: impl Into<String>) {
        self.status = Some(StatusMessage {
            message: message.into(),
            is_error: false,
            created_at: Instant::now(),
        });
    }

    pub fn set_error(&mut self, message: impl Into<String>) {
        self.status = Some(StatusMessage {
            message: message.into(),
            is_error: true,
            created_at: Instant::now(),
        });
    }

    fn clear_expired_status(&mut self) {
        if let Some(status) = &self.status {
            if status.created_at.elapsed() > STATUS_TIMEOUT {
                self.status = None;
            }
        }
    }

    fn send_command(&mut self, cmd: WorkerCommand) -> bool {
        match self.cmd_tx.send(cmd) {
            Ok(()) => true,
            Err(_) => {
                self.set_error("Worker disconnected");
                false
            }
        }
    }

    fn run(&mut self, result_rx: &mpsc::Receiver<WorkerResult>) -> anyhow::Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let result = self.run_event_loop(&mut terminal, result_rx);

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

        result
    }

    fn run_event_loop(
        &mut self,
        terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
        result_rx: &mpsc::Receiver<WorkerResult>,
    ) -> anyhow::Result<()> {
        while self.running {
            // Process all pending worker results (non-blocking)
            while let Ok(result) = result_rx.try_recv() {
                self.handle_worker_result(result);
            }

            self.clear_expired_status();
            terminal.draw(|frame| ui::draw(frame, self))?;

            // Poll for keyboard events with timeout
            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key_event(key);
                }
            }
        }
        Ok(())
    }

    fn handle_worker_result(&mut self, result: WorkerResult) {
        match result {
            WorkerResult::Connected {
                namespaces,
                initial_namespace_idx,
            } => {
                self.connecting = false;
                self.namespaces = namespaces;
                self.current_namespace_idx = initial_namespace_idx;
                self.connection_error = None;
                // Request data refresh
                if let Some(ns) = self.current_namespace() {
                    let namespace = ns.namespace.name.clone();
                    self.loading = true;
                    self.send_command(WorkerCommand::RefreshData { namespace });
                }
            }
            WorkerResult::ConnectionError { message } => {
                self.connecting = false;
                self.connection_error = Some(message);
            }
            WorkerResult::DataRefreshed { folders, repos } => {
                self.loading = false;
                self.folders = folders.clone();
                self.repos = repos;
                self.rebuild_tree();

                // Preload all root folder children
                if let Some(namespace) = self.current_namespace().map(|n| n.namespace.name.clone())
                {
                    for folder in &folders {
                        if !self.pending_folder_loads.contains(&folder.id) {
                            if self.send_command(WorkerCommand::LoadFolderChildren {
                                folder_id: folder.id.clone(),
                                namespace: namespace.clone(),
                            }) {
                                self.pending_folder_loads.push(folder.id.clone());
                            }
                        }
                    }
                }
            }
            WorkerResult::FolderChildrenLoaded {
                folder_id,
                child_folders,
            } => {
                self.pending_folder_loads.retain(|id| id != &folder_id);

                let child_repos: Vec<_> = self
                    .repos
                    .iter()
                    .filter(|r| r.folder_id.as_deref() == Some(&folder_id))
                    .cloned()
                    .collect();

                load_children_by_folder_id(
                    &mut self.tree,
                    &folder_id,
                    &child_folders,
                    &child_repos,
                );

                // Cascade: preload children of any child folders
                if let Some(namespace) = self.current_namespace().map(|n| n.namespace.name.clone())
                {
                    for child_folder in &child_folders {
                        if !self.pending_folder_loads.contains(&child_folder.id) {
                            if self.send_command(WorkerCommand::LoadFolderChildren {
                                folder_id: child_folder.id.clone(),
                                namespace: namespace.clone(),
                            }) {
                                self.pending_folder_loads.push(child_folder.id.clone());
                            }
                        }
                    }
                }
            }
            WorkerResult::FolderCreated { name } => {
                self.set_status(format!("Created folder '{}'", name));
                self.request_refresh();
            }
            WorkerResult::FolderRenamed { new_name } => {
                self.set_status(format!("Renamed folder to '{}'", new_name));
                self.request_refresh();
            }
            WorkerResult::FolderDeleted => {
                self.set_status("Folder deleted");
                self.request_refresh();
            }
            WorkerResult::FolderMoved { name } => {
                self.set_status(format!("Moved folder '{}'", name));
                self.request_refresh();
            }
            WorkerResult::RepoMoved { name } => {
                self.set_status(format!("Moved repo '{}'", name));
                self.request_refresh();
            }
            WorkerResult::Error { message } => {
                self.set_error(message);
            }
        }
    }

    fn request_refresh(&mut self) {
        if let Some(ns) = self.current_namespace() {
            let namespace = ns.namespace.name.clone();
            self.loading = true;
            self.send_command(WorkerCommand::RefreshData { namespace });
        }
    }

    fn handle_key_event(&mut self, key: KeyEvent) {
        match &self.mode {
            InputMode::Normal => self.handle_normal_key(key),
            InputMode::CreatingFolder { .. } | InputMode::RenamingFolder { .. } => {
                self.handle_input_key(key)
            }
            InputMode::ConfirmDelete { .. } => self.handle_confirm_key(key),
            InputMode::MovingItem { .. } => self.handle_move_key(key),
        }
    }

    fn handle_normal_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.running = false,
            KeyCode::Char('j') | KeyCode::Down => self.move_selection_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_selection_up(),
            KeyCode::Char('l') | KeyCode::Right | KeyCode::Enter => self.expand_selected(),
            KeyCode::Char('h') | KeyCode::Left => self.collapse_or_go_to_parent(),
            KeyCode::Char('e') => self.toggle_expand_all(),
            KeyCode::Char('n') => self.start_create_folder(),
            KeyCode::Char('R') => self.start_rename_folder(),
            KeyCode::Char('m') => self.start_move_item(),
            KeyCode::Char('d') => self.start_delete_folder(),
            KeyCode::Tab => self.next_workspace(),
            KeyCode::Char('r') => self.do_refresh(),
            _ => {}
        }
    }

    fn do_refresh(&mut self) {
        if self.connection_error.is_some() {
            // Retry connection
            self.connecting = true;
            self.connection_error = None;
            self.send_command(WorkerCommand::Connect {
                initial_workspace: None,
            });
        } else {
            self.request_refresh();
        }
    }

    fn handle_input_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = InputMode::Normal;
                self.input_buffer.clear();
            }
            KeyCode::Enter => {
                match self.mode.clone() {
                    InputMode::CreatingFolder { parent_id } => {
                        self.confirm_create_folder(parent_id.as_deref());
                    }
                    InputMode::RenamingFolder { folder_id } => {
                        self.confirm_rename_folder(&folder_id);
                    }
                    _ => {}
                }
                self.mode = InputMode::Normal;
            }
            KeyCode::Backspace => {
                self.input_buffer.pop();
            }
            KeyCode::Char(c) => {
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT {
                    self.input_buffer.push(c);
                }
            }
            _ => {}
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                if let InputMode::ConfirmDelete {
                    folder_id,
                    has_children,
                    ..
                } = self.mode.clone()
                {
                    self.confirm_delete_folder(&folder_id, has_children);
                }
                self.mode = InputMode::Normal;
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                self.mode = InputMode::Normal;
            }
            _ => {}
        }
    }

    fn handle_move_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => {
                self.mode = InputMode::Normal;
            }
            KeyCode::Char('j') | KeyCode::Down => self.navigate_move_target(1),
            KeyCode::Char('k') | KeyCode::Up => self.navigate_move_target(-1),
            KeyCode::Enter => {
                if let InputMode::MovingItem {
                    source_index,
                    move_target_index,
                    move_targets,
                } = &self.mode
                {
                    let target_folder_id = if *move_target_index == usize::MAX {
                        None
                    } else {
                        move_targets
                            .iter()
                            .find(|t| t.tree_index == *move_target_index)
                            .and_then(|t| t.folder_id.clone())
                    };

                    self.confirm_move_item(*source_index, target_folder_id);
                }
                self.mode = InputMode::Normal;
            }
            _ => {}
        }
    }

    fn navigate_move_target(&mut self, delta: i32) {
        let InputMode::MovingItem {
            move_target_index,
            move_targets,
            source_index,
        } = &self.mode
        else {
            return;
        };

        let current_pos = if *move_target_index == usize::MAX {
            0
        } else {
            move_targets
                .iter()
                .position(|t| t.tree_index == *move_target_index)
                .map(|p| p + 1)
                .unwrap_or(0)
        };

        let next_pos = if delta > 0 {
            (current_pos + delta as usize).min(move_targets.len())
        } else {
            current_pos.saturating_sub((-delta) as usize)
        };

        let new_target_index = if next_pos == 0 {
            usize::MAX
        } else {
            move_targets
                .get(next_pos - 1)
                .map(|t| t.tree_index)
                .unwrap_or(usize::MAX)
        };

        self.mode = InputMode::MovingItem {
            source_index: *source_index,
            move_target_index: new_target_index,
            move_targets: move_targets.clone(),
        };
    }

    fn move_selection_down(&mut self) {
        let flat_len = flatten_tree(&self.tree).len();
        if self.selected_index < flat_len.saturating_sub(1) {
            self.selected_index += 1;
            self.preload_selected_folder();
        }
    }

    fn move_selection_up(&mut self) {
        if self.selected_index > 0 {
            self.selected_index -= 1;
            self.preload_selected_folder();
        }
    }

    fn preload_selected_folder(&mut self) {
        let (children_loaded, folder_id) = {
            let Some(node) = get_node_at(&self.tree, self.selected_index) else {
                return;
            };
            if !node.is_folder() {
                return;
            }
            (node.children_loaded, node.folder_id().map(String::from))
        };

        if children_loaded {
            return;
        }

        let Some(folder_id) = folder_id else {
            return;
        };

        if self.pending_folder_loads.contains(&folder_id) {
            return;
        }

        let Some(namespace) = self.current_namespace().map(|n| n.namespace.name.clone()) else {
            return;
        };

        if self.send_command(WorkerCommand::LoadFolderChildren {
            folder_id: folder_id.clone(),
            namespace,
        }) {
            self.pending_folder_loads.push(folder_id);
        }
    }

    fn expand_selected(&mut self) {
        let (is_folder, children_loaded, folder_id) = {
            let Some(node) = get_node_at(&self.tree, self.selected_index) else {
                return;
            };
            (
                node.is_folder(),
                node.children_loaded,
                node.folder_id().map(String::from),
            )
        };

        if !is_folder {
            return;
        }

        // If children not loaded, request load (will expand when result arrives)
        if !children_loaded {
            if let Some(folder_id) = folder_id {
                if !self.pending_folder_loads.contains(&folder_id) {
                    if let Some(namespace) =
                        self.current_namespace().map(|n| n.namespace.name.clone())
                    {
                        if self.send_command(WorkerCommand::LoadFolderChildren {
                            folder_id: folder_id.clone(),
                            namespace,
                        }) {
                            self.pending_folder_loads.push(folder_id);
                        }
                    }
                }
            }
            // Still toggle to show loading state / expand empty
            toggle_expanded_at(&mut self.tree, self.selected_index);
        } else {
            toggle_expanded_at(&mut self.tree, self.selected_index);
        }
    }

    fn collapse_or_go_to_parent(&mut self) {
        if let Some(node) = get_node_at(&self.tree, self.selected_index) {
            if node.is_folder() && node.expanded {
                set_expanded_at(&mut self.tree, self.selected_index, false);
                return;
            }
        }

        if let Some(parent_idx) = find_parent_index(&self.tree, self.selected_index) {
            self.selected_index = parent_idx;
        }
    }

    fn toggle_expand_all(&mut self) {
        self.all_expanded = !self.all_expanded;
        set_all_expanded(&mut self.tree, self.all_expanded);
        if !self.all_expanded {
            self.selected_index = 0;
        }
    }

    fn start_create_folder(&mut self) {
        let parent_id = if let Some(node) = get_node_at(&self.tree, self.selected_index) {
            match &node.kind {
                TreeNodeKind::Folder(f) => Some(f.id.clone()),
                TreeNodeKind::Uncategorized => None,
                TreeNodeKind::Repo(r) => r.folder_id.clone(),
            }
        } else {
            None
        };

        self.input_buffer.clear();
        self.mode = InputMode::CreatingFolder { parent_id };
    }

    fn start_rename_folder(&mut self) {
        if let Some(node) = get_node_at(&self.tree, self.selected_index) {
            if let TreeNodeKind::Folder(f) = &node.kind {
                self.input_buffer = f.name.clone();
                self.mode = InputMode::RenamingFolder {
                    folder_id: f.id.clone(),
                };
            } else {
                self.set_error("Can only rename folders");
            }
        }
    }

    fn start_move_item(&mut self) {
        if let Some(node) = get_node_at(&self.tree, self.selected_index) {
            if matches!(node.kind, TreeNodeKind::Uncategorized) {
                self.set_error("Cannot move uncategorized section");
                return;
            }

            let flat_nodes = flatten_tree(&self.tree);
            let move_targets: Vec<MoveTarget> = flat_nodes
                .iter()
                .enumerate()
                .filter_map(|(idx, flat_node)| {
                    if !flat_node.node.is_folder() || idx == self.selected_index {
                        return None;
                    }

                    if matches!(flat_node.node.kind, TreeNodeKind::Uncategorized) {
                        return None;
                    }

                    let folder_id = match &flat_node.node.kind {
                        TreeNodeKind::Folder(f) => Some(f.id.clone()),
                        _ => None,
                    };

                    Some(MoveTarget {
                        folder_id,
                        tree_index: idx,
                    })
                })
                .collect();

            self.mode = InputMode::MovingItem {
                source_index: self.selected_index,
                move_target_index: usize::MAX,
                move_targets,
            };
        }
    }

    fn start_delete_folder(&mut self) {
        if let Some(node) = get_node_at(&self.tree, self.selected_index) {
            if let TreeNodeKind::Folder(f) = &node.kind {
                let has_children = !node.children.is_empty();
                self.mode = InputMode::ConfirmDelete {
                    folder_id: f.id.clone(),
                    name: f.name.clone(),
                    has_children,
                };
            } else {
                self.set_error("Can only delete folders");
            }
        }
    }

    fn confirm_create_folder(&mut self, parent_id: Option<&str>) {
        let name = self.input_buffer.trim();
        if name.is_empty() {
            self.set_error("Folder name cannot be empty");
            return;
        }

        let namespace = self.current_namespace().map(|n| n.namespace.name.clone());

        self.send_command(WorkerCommand::CreateFolder {
            name: name.to_string(),
            parent_id: parent_id.map(String::from),
            namespace,
        });
        self.input_buffer.clear();
    }

    fn confirm_rename_folder(&mut self, folder_id: &str) {
        let new_name = self.input_buffer.trim();
        if new_name.is_empty() {
            self.set_error("Folder name cannot be empty");
            return;
        }

        self.send_command(WorkerCommand::RenameFolder {
            folder_id: folder_id.to_string(),
            new_name: new_name.to_string(),
        });
        self.input_buffer.clear();
    }

    fn confirm_delete_folder(&mut self, folder_id: &str, has_children: bool) {
        self.send_command(WorkerCommand::DeleteFolder {
            folder_id: folder_id.to_string(),
            recursive: has_children,
        });
    }

    fn confirm_move_item(&mut self, source_index: usize, target_folder_id: Option<String>) {
        let Some(node) = get_node_at(&self.tree, source_index) else {
            self.set_error("Item not found");
            return;
        };

        match &node.kind {
            TreeNodeKind::Folder(f) => {
                self.send_command(WorkerCommand::MoveFolder {
                    folder_id: f.id.clone(),
                    new_parent_id: target_folder_id,
                });
            }
            TreeNodeKind::Repo(r) => {
                self.send_command(WorkerCommand::MoveRepo {
                    repo_id: r.id.clone(),
                    folder_id: target_folder_id,
                });
            }
            TreeNodeKind::Uncategorized => {
                self.set_error("Cannot move uncategorized section");
            }
        }
    }

    fn next_workspace(&mut self) {
        if self.namespaces.len() > 1 {
            self.current_namespace_idx = (self.current_namespace_idx + 1) % self.namespaces.len();
            self.selected_index = 0;
            self.pending_folder_loads.clear();
            self.request_refresh();
        }
    }
}

fn run_worker(
    client: ApiClient,
    cmd_rx: mpsc::Receiver<WorkerCommand>,
    result_tx: mpsc::Sender<WorkerResult>,
    initial_workspace: Option<String>,
) {
    // Initial connection
    let connect_result = do_connect(&client, initial_workspace.as_deref());
    if result_tx.send(connect_result).is_err() {
        return;
    }

    // Process commands
    while let Ok(cmd) = cmd_rx.recv() {
        let result = match cmd {
            WorkerCommand::Connect { initial_workspace } => {
                do_connect(&client, initial_workspace.as_deref())
            }
            WorkerCommand::RefreshData { namespace } => do_refresh_data(&client, &namespace),
            WorkerCommand::LoadFolderChildren {
                folder_id,
                namespace,
            } => do_load_folder_children(&client, &folder_id, &namespace),
            WorkerCommand::CreateFolder {
                name,
                parent_id,
                namespace,
            } => do_create_folder(&client, &name, parent_id.as_deref(), namespace.as_deref()),
            WorkerCommand::RenameFolder { folder_id, new_name } => {
                do_rename_folder(&client, &folder_id, &new_name)
            }
            WorkerCommand::DeleteFolder {
                folder_id,
                recursive,
            } => do_delete_folder(&client, &folder_id, recursive),
            WorkerCommand::MoveFolder {
                folder_id,
                new_parent_id,
            } => do_move_folder(&client, &folder_id, new_parent_id.as_deref()),
            WorkerCommand::MoveRepo { repo_id, folder_id } => {
                do_move_repo(&client, &repo_id, folder_id.as_deref())
            }
        };

        if result_tx.send(result).is_err() {
            break;
        }
    }
}

fn do_connect(client: &ApiClient, initial_workspace: Option<&str>) -> WorkerResult {
    match actions::fetch_namespaces(client) {
        Ok(namespaces) => {
            if namespaces.is_empty() {
                return WorkerResult::ConnectionError {
                    message: "No accessible workspaces found".to_string(),
                };
            }

            let initial_namespace_idx = if let Some(workspace) = initial_workspace {
                match namespaces.iter().position(|n| n.namespace.name == workspace) {
                    Some(idx) => idx,
                    None => {
                        return WorkerResult::ConnectionError {
                            message: format!("Workspace '{}' not found", workspace),
                        };
                    }
                }
            } else {
                namespaces.iter().position(|n| n.is_primary).unwrap_or(0)
            };

            WorkerResult::Connected {
                namespaces,
                initial_namespace_idx,
            }
        }
        Err(e) => WorkerResult::ConnectionError {
            message: format!("Failed to connect: {}", e),
        },
    }
}

fn do_refresh_data(client: &ApiClient, namespace: &str) -> WorkerResult {
    let folders = match actions::fetch_root_folders(client, namespace) {
        Ok(f) => f,
        Err(e) => {
            return WorkerResult::Error {
                message: format!("Failed to fetch folders: {}", e),
            }
        }
    };

    let repos = match actions::fetch_repos(client, namespace) {
        Ok(r) => r,
        Err(e) => {
            return WorkerResult::Error {
                message: format!("Failed to fetch repos: {}", e),
            }
        }
    };

    WorkerResult::DataRefreshed { folders, repos }
}

fn do_load_folder_children(client: &ApiClient, folder_id: &str, namespace: &str) -> WorkerResult {
    match actions::fetch_folder_children(client, namespace, folder_id) {
        Ok(child_folders) => WorkerResult::FolderChildrenLoaded {
            folder_id: folder_id.to_string(),
            child_folders,
        },
        Err(e) => WorkerResult::Error {
            message: format!("Failed to load folder contents: {}", e),
        },
    }
}

fn do_create_folder(
    client: &ApiClient,
    name: &str,
    parent_id: Option<&str>,
    namespace: Option<&str>,
) -> WorkerResult {
    match actions::create_folder(client, name, parent_id, namespace) {
        Ok(_) => WorkerResult::FolderCreated {
            name: name.to_string(),
        },
        Err(e) => WorkerResult::Error {
            message: format!("Failed to create folder: {}", e),
        },
    }
}

fn do_rename_folder(client: &ApiClient, folder_id: &str, new_name: &str) -> WorkerResult {
    match actions::rename_folder(client, folder_id, new_name) {
        Ok(_) => WorkerResult::FolderRenamed {
            new_name: new_name.to_string(),
        },
        Err(e) => WorkerResult::Error {
            message: format!("Failed to rename folder: {}", e),
        },
    }
}

fn do_delete_folder(client: &ApiClient, folder_id: &str, recursive: bool) -> WorkerResult {
    match actions::delete_folder(client, folder_id, recursive) {
        Ok(()) => WorkerResult::FolderDeleted,
        Err(e) => WorkerResult::Error {
            message: format!("Failed to delete folder: {}", e),
        },
    }
}

fn do_move_folder(
    client: &ApiClient,
    folder_id: &str,
    new_parent_id: Option<&str>,
) -> WorkerResult {
    match actions::move_folder(client, folder_id, new_parent_id) {
        Ok(folder) => WorkerResult::FolderMoved { name: folder.name },
        Err(e) => WorkerResult::Error {
            message: format!("Failed to move folder: {}", e),
        },
    }
}

fn do_move_repo(client: &ApiClient, repo_id: &str, folder_id: Option<&str>) -> WorkerResult {
    match actions::move_repo(client, repo_id, folder_id) {
        Ok(()) => WorkerResult::RepoMoved {
            name: repo_id.to_string(),
        },
        Err(e) => WorkerResult::Error {
            message: format!("Failed to move repo: {}", e),
        },
    }
}

pub fn run_manage(workspace: Option<String>) -> anyhow::Result<()> {
    let credentials = load_credentials()?;
    let client = ApiClient::new(&credentials)?;

    let (cmd_tx, cmd_rx) = mpsc::channel::<WorkerCommand>();
    let (result_tx, result_rx) = mpsc::channel::<WorkerResult>();

    // Spawn worker thread
    let worker_workspace = workspace.clone();
    let worker_handle = std::thread::spawn(move || {
        run_worker(client, cmd_rx, result_tx, worker_workspace);
    });

    let mut app = App::new(cmd_tx.clone());
    let result = app.run(&result_rx);

    // Signal worker to shutdown (will cause recv to return Err) and wait
    drop(cmd_tx);
    let _ = worker_handle.join();

    result
}
