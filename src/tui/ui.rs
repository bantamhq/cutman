use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph},
};

use super::app::{App, InputMode};
use super::tree::{FlatNode, TreeNodeKind, flatten_tree};

const COLLAPSED_ICON: &str = "\u{25b6}"; // ▶ (filled)
const EXPANDED_ICON: &str = "\u{25bc}"; // ▼ (filled)
const EMPTY_FOLDER_ICON: &str = "\u{25bd}"; // ▽ (outline, always down)
const REPO_ICON: &str = "\u{25cb}"; // ○

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(5),    // Main tree view
            Constraint::Length(3), // Status bar
            Constraint::Length(1), // Help bar
        ])
        .split(frame.area());

    draw_header(frame, app, chunks[0]);
    draw_tree(frame, app, chunks[1]);
    draw_status(frame, app, chunks[2]);
    draw_help(frame, app, chunks[3]);

    match &app.mode {
        InputMode::CreatingFolder { .. } => draw_input_dialog(frame, app, "New Folder Name"),
        InputMode::RenamingFolder { .. } => draw_input_dialog(frame, app, "Rename Folder"),
        InputMode::ConfirmDelete { name, .. } => draw_confirm_dialog(frame, name),
        InputMode::MovingItem { .. } => draw_move_dialog(frame, app),
        InputMode::Normal => {}
    }
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let (title, style) = if app.connection_error.is_some() {
        (" Not Connected ".to_string(), Style::default().fg(Color::Red))
    } else {
        let namespace_name = app
            .current_namespace()
            .map(|n| n.namespace.name.as_str())
            .unwrap_or("Unknown");

        let is_primary = app.current_namespace().is_some_and(|n| n.is_primary);

        let primary_indicator = if is_primary { " (primary)" } else { "" };
        let workspace_indicator = if app.namespaces.len() > 1 {
            format!(
                " [{}/{}]",
                app.current_namespace_idx + 1,
                app.namespaces.len()
            )
        } else {
            String::new()
        };

        (
            format!(
                " Workspace: {}{}{} ",
                namespace_name, primary_indicator, workspace_indicator
            ),
            Style::default().fg(Color::Cyan),
        )
    };

    let header = Paragraph::new(Line::from(vec![Span::styled(title, style)])).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    frame.render_widget(header, area);
}

fn draw_tree(frame: &mut Frame, app: &App, area: Rect) {
    let tree_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::DarkGray));

    if app.connecting {
        let connecting_text = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "Connecting...",
                Style::default().fg(Color::Yellow),
            )),
        ])
        .block(tree_block)
        .alignment(ratatui::layout::Alignment::Center);

        frame.render_widget(connecting_text, area);
        return;
    }

    if let Some(error) = &app.connection_error {
        let error_text = Paragraph::new(vec![
            Line::from(""),
            Line::from(Span::styled(
                "Connection Error",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(error.as_str(), Style::default().fg(Color::Red))),
            Line::from(""),
            Line::from(Span::styled(
                "Press 'r' to retry",
                Style::default().fg(Color::DarkGray),
            )),
        ])
        .block(tree_block)
        .alignment(ratatui::layout::Alignment::Center);

        frame.render_widget(error_text, area);
        return;
    }

    let flat_nodes = flatten_tree(&app.tree);

    if flat_nodes.is_empty() {
        let (message, hint) = if app.loading {
            ("Loading...", "")
        } else {
            ("No folders or repos", "Press 'n' to create a folder")
        };

        let mut lines = vec![
            Line::from(""),
            Line::from(Span::styled(
                message,
                Style::default().fg(Color::DarkGray),
            )),
        ];
        if !hint.is_empty() {
            lines.push(Line::from(""));
            lines.push(Line::from(Span::styled(
                hint,
                Style::default().fg(Color::DarkGray),
            )));
        }

        let empty_text = Paragraph::new(lines)
            .block(tree_block)
            .alignment(ratatui::layout::Alignment::Center);

        frame.render_widget(empty_text, area);
        return;
    }

    let items: Vec<ListItem> = flat_nodes
        .iter()
        .enumerate()
        .map(|(idx, flat_node)| {
            let is_selected = idx == app.selected_index;
            let is_move_target = matches!(&app.mode, InputMode::MovingItem { move_target_index, .. } if *move_target_index == idx);

            create_tree_item(flat_node, is_selected, is_move_target)
        })
        .collect();

    let list = List::new(items).block(tree_block);

    frame.render_widget(list, area);
}

fn create_tree_item(flat_node: &FlatNode, is_selected: bool, is_move_target: bool) -> ListItem<'static> {
    let node = &flat_node.node;
    let indent = "  ".repeat(node.depth);

    let (icon, name) = match &node.kind {
        TreeNodeKind::Uncategorized => {
            let expand_icon = if node.expanded { EXPANDED_ICON } else { COLLAPSED_ICON };
            (format!("{} ", expand_icon), "[Uncategorized]".to_string())
        }
        TreeNodeKind::Folder(f) => {
            // Empty folders (loaded with no children) always show outline down arrow
            let is_empty = node.children_loaded && node.children.is_empty();
            let expand_icon = if is_empty {
                EMPTY_FOLDER_ICON
            } else if node.expanded {
                EXPANDED_ICON
            } else {
                COLLAPSED_ICON
            };
            (format!("{} ", expand_icon), f.name.clone())
        }
        TreeNodeKind::Repo(r) => (format!("  {} ", REPO_ICON), r.name.clone()),
    };

    let style = if is_selected {
        Style::default()
            .bg(Color::Blue)
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else if is_move_target {
        Style::default()
            .bg(Color::Yellow)
            .fg(Color::Black)
    } else {
        Style::default()
    };

    let content = format!("{}{}{}", indent, icon, name);
    ListItem::new(Line::from(Span::styled(content, style)))
}

fn draw_status(frame: &mut Frame, app: &App, area: Rect) {
    let (message, style) = if let Some(status) = &app.status {
        let style = if status.is_error {
            Style::default().fg(Color::Red)
        } else {
            Style::default().fg(Color::Green)
        };
        (status.message.clone(), style)
    } else if app.loading {
        ("Loading...".to_string(), Style::default().fg(Color::Yellow))
    } else {
        ("Ready".to_string(), Style::default().fg(Color::DarkGray))
    };

    let status = Paragraph::new(Line::from(Span::styled(message, style))).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(" Status "),
    );

    frame.render_widget(status, area);
}

fn draw_help(frame: &mut Frame, app: &App, area: Rect) {
    let help_text = if app.connection_error.is_some() {
        "r:retry  q:quit"
    } else {
        match &app.mode {
            InputMode::Normal => {
                "n:new  R:rename  m:move  d:delete  e:expand  Tab:workspace  r:refresh  q:quit"
            }
            InputMode::CreatingFolder { .. } | InputMode::RenamingFolder { .. } => {
                "Enter:confirm  Esc:cancel"
            }
            InputMode::ConfirmDelete { .. } => "y:confirm  n/Esc:cancel",
            InputMode::MovingItem { .. } => "j/k:select target  Enter:confirm  Esc:cancel",
        }
    };

    let help = Paragraph::new(Line::from(Span::styled(
        help_text,
        Style::default().fg(Color::DarkGray),
    )));

    frame.render_widget(help, area);
}

fn draw_input_dialog(frame: &mut Frame, app: &App, title: &str) {
    let area = centered_rect(50, 5, frame.area());

    frame.render_widget(Clear, area);

    let input = &app.input_buffer;
    let dialog = Paragraph::new(Line::from(Span::raw(input.as_str()))).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(format!(" {} ", title)),
    );

    frame.render_widget(dialog, area);

    frame.set_cursor_position((
        area.x + 1 + input.len() as u16,
        area.y + 1,
    ));
}

fn draw_confirm_dialog(frame: &mut Frame, name: &str) {
    let area = centered_rect(50, 5, frame.area());

    frame.render_widget(Clear, area);

    let message = format!("Delete folder \"{}\"? (y/n)", name);
    let dialog = Paragraph::new(Line::from(Span::styled(
        message,
        Style::default().fg(Color::Red),
    )))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Red))
            .title(" Confirm Delete "),
    );

    frame.render_widget(dialog, area);
}

fn draw_move_dialog(frame: &mut Frame, app: &App) {
    let area = centered_rect(60, 15, frame.area());

    frame.render_widget(Clear, area);

    let flat_nodes = flatten_tree(&app.tree);

    let (move_target_index, moving_node_index) = match &app.mode {
        InputMode::MovingItem {
            move_target_index,
            source_index,
            ..
        } => (*move_target_index, *source_index),
        _ => return,
    };

    let items: Vec<ListItem> = std::iter::once(ListItem::new(Line::from(Span::styled(
        "[Root]",
        if move_target_index == usize::MAX {
            Style::default().bg(Color::Yellow).fg(Color::Black)
        } else {
            Style::default()
        },
    ))))
    .chain(flat_nodes.iter().enumerate().filter_map(|(idx, flat_node)| {
        if !flat_node.node.is_folder() || idx == moving_node_index {
            return None;
        }

        let is_target = idx == move_target_index;
        let indent = "  ".repeat(flat_node.node.depth);
        let name = flat_node.node.name();

        let style = if is_target {
            Style::default().bg(Color::Yellow).fg(Color::Black)
        } else {
            Style::default()
        };

        Some(ListItem::new(Line::from(Span::styled(
            format!("{}{}", indent, name),
            style,
        ))))
    }))
    .collect();

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .title(" Move to... "),
    );

    frame.render_widget(list, area);
}

fn centered_rect(percent_x: u16, height: u16, area: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length((area.height.saturating_sub(height)) / 2),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(area);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
