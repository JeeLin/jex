//! 交互式依赖树浏览 TUI（`jex tree -i`）
//!
//! 使用 ratatui + crossterm 全屏 TUI，支持：
//! - ↑/↓/j/k 移动光标
//! - →/l 展开节点
//! - ←/h 折叠节点
//! - / 搜索过滤
//! - q/Esc 退出
//! - ? 帮助

use crate::error::{Error, Result};
use crate::tree::{DependencyNode, DependencyTree};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;

// ---------------------------------------------------------------------------
// Flattened tree item (view model)
// ---------------------------------------------------------------------------

/// 扁平化的树节点，用于渲染和导航
#[derive(Debug, Clone)]
struct FlatItem {
    name: String,
    version: String,
    license: Option<String>,
    depth: usize,
    has_children: bool,
    /// index into the original `DependencyNode::children` of the parent,
    /// only meaningful when `depth > 0`
    /// for root children this is the index in root.children;
    /// for deeper nodes this is the index within parent's children
    child_idx: usize,
}

/// Build a flat list of visible items from the dependency tree.
///
/// `expanded` tracks which depth-level indices are expanded.
/// `filter` optionally filters by a case-insensitive substring on name.
fn flatten_tree(
    tree: &DependencyTree,
    expanded: &std::collections::HashSet<usize>,
    filter: &str,
) -> Vec<FlatItem> {
    let mut items = Vec::new();

    // The root itself
    items.push(FlatItem {
        name: tree.root.name.clone(),
        version: tree.root.version.clone(),
        license: tree.root.license.clone(),
        depth: 0,
        has_children: !tree.root.children.is_empty(),
        child_idx: 0,
    });

    if expanded.contains(&0) {
        flatten_children(&tree.root.children, 1, 0, expanded, filter, &mut items);
    }

    items
}

fn flatten_children(
    children: &[DependencyNode],
    depth: usize,
    parent_tree_idx: usize,
    expanded: &std::collections::HashSet<usize>,
    filter: &str,
    items: &mut Vec<FlatItem>,
) {
    for (i, child) in children.iter().enumerate() {
        let tree_idx = parent_tree_idx * 1000 + i; // simple composite index

        let matches = filter.is_empty()
            || child.name.to_lowercase().contains(&filter.to_lowercase())
            || child
                .version
                .to_lowercase()
                .contains(&filter.to_lowercase());

        if !matches && !has_matching_descendant(child, filter) {
            continue;
        }

        items.push(FlatItem {
            name: child.name.clone(),
            version: child.version.clone(),
            license: child.license.clone(),
            depth,
            has_children: !child.children.is_empty(),
            child_idx: i,
        });

        if expanded.contains(&tree_idx) {
            flatten_children(&child.children, depth + 1, tree_idx, expanded, filter, items);
        }
    }
}

fn has_matching_descendant(node: &DependencyNode, filter: &str) -> bool {
    if filter.is_empty() {
        return true;
    }
    let lf = filter.to_lowercase();
    for child in &node.children {
        if child.name.to_lowercase().contains(&lf)
            || child.version.to_lowercase().contains(&lf)
        {
            return true;
        }
        if has_matching_descendant(child, filter) {
            return true;
        }
    }
    false
}

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

struct App<'a> {
    tree: &'a DependencyTree,
    flat: Vec<FlatItem>,
    selected: usize,
    expanded: std::collections::HashSet<usize>,
    filter: String,
    input_mode: InputMode,
    show_help: bool,
}

#[derive(Debug, PartialEq)]
enum InputMode {
    Normal,
    Search,
}

impl<'a> App<'a> {
    fn new(tree: &'a DependencyTree) -> Self {
        let expanded = std::collections::HashSet::from([0usize]); // root expanded by default
        let flat = flatten_tree(tree, &expanded, "");
        Self {
            tree,
            flat,
            selected: 0,
            expanded,
            filter: String::new(),
            input_mode: InputMode::Normal,
            show_help: false,
        }
    }

    fn refresh(&mut self) {
        self.flat = flatten_tree(self.tree, &self.expanded, &self.filter);
        if self.selected >= self.flat.len() {
            self.selected = self.flat.len().saturating_sub(1);
        }
    }

    fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
    }

    fn move_down(&mut self) {
        if self.selected + 1 < self.flat.len() {
            self.selected += 1;
        }
    }

    /// Expand current node (→/l)
    fn expand(&mut self) {
        if self.selected >= self.flat.len() {
            return;
        }
        let item = &self.flat[self.selected];
        if !item.has_children {
            return;
        }
        let tree_idx = self.compute_tree_idx(item);
        self.expanded.insert(tree_idx);
        self.refresh();
    }

    /// Collapse current node (←/h). If already collapsed, move to parent.
    fn collapse(&mut self) {
        if self.selected >= self.flat.len() {
            return;
        }
        let item = &self.flat[self.selected];
        let tree_idx = self.compute_tree_idx(item);
        if item.has_children && self.expanded.contains(&tree_idx) {
            self.expanded.remove(&tree_idx);
            self.refresh();
        } else if item.depth > 0 {
            // Move selection to parent
            let target_depth = item.depth - 1;
            for i in (0..self.selected).rev() {
                if self.flat[i].depth == target_depth {
                    self.selected = i;
                    break;
                }
            }
        }
    }

    /// Compute a stable tree index for expanded-set tracking
    fn compute_tree_idx(&self, item: &FlatItem) -> usize {
        // Walk backwards through flat to reconstruct path
        // For simplicity we use depth * 1000 + child_idx relative to parent
        // Actually, let's walk the flat list to find the right index
        if item.depth == 0 {
            return 0;
        }
        // Find the parent in flat list
        for i in (0..self.selected).rev() {
            if self.flat[i].depth == item.depth - 1 {
                let parent = &self.flat[i];
                if parent.depth == 0 {
                    return item.child_idx;
                }
                // Recurse
                let parent_tree_idx = self.compute_tree_idx(parent);
                return parent_tree_idx * 1000 + item.child_idx;
            }
        }
        0
    }

    /// Expand all visible nodes
    fn expand_all(&mut self) {
        self.expand_all_from(0, &self.tree.root);
        self.refresh();
    }

    fn expand_all_from(&mut self, tree_idx: usize, node: &DependencyNode) {
        if !node.children.is_empty() {
            self.expanded.insert(tree_idx);
        }
        for (i, child) in node.children.iter().enumerate() {
            let child_idx = tree_idx * 1000 + i;
            self.expand_all_from(child_idx, child);
        }
    }

    /// Collapse all nodes
    fn collapse_all(&mut self) {
        self.expanded.clear();
        self.expanded.insert(0); // keep root visible
        self.selected = 0;
        self.refresh();
    }
}

// ---------------------------------------------------------------------------
// TUI rendering
// ---------------------------------------------------------------------------

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),      // main area
            Constraint::Length(1),   // status bar
        ])
        .split(f.area());

    let main_area = chunks[0];
    let status_area = chunks[1];

    if app.flat.is_empty() {
        let msg = if app.filter.is_empty() {
            "依赖树为空"
        } else {
            "没有匹配的依赖"
        };
        let paragraph = Paragraph::new(msg).block(
            Block::default()
                .title("jex tree (交互模式)")
                .borders(Borders::ALL),
        );
        f.render_widget(paragraph, main_area);
    } else {
        let items: Vec<ListItem> = app
            .flat
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let indent = "  ".repeat(item.depth);
                let prefix = if item.has_children {
                    if app.expanded.contains(&app.compute_tree_idx(item)) {
                        "▼ "
                    } else {
                        "▶ "
                    }
                } else {
                    "  "
                };
                let name_style = if i == app.selected {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                };
                let version_style = Style::default().fg(Color::DarkGray);

                let spans = vec![
                    Span::styled(format!("{}{}", indent, prefix), name_style),
                    Span::styled(&item.name, name_style),
                    Span::styled(format!("  v{}", item.version), version_style),
                    item.license
                        .as_ref()
                        .map(|l| Span::styled(format!("  [{}]", l), Style::default().fg(Color::Blue)))
                        .unwrap_or_default(),
                ];
                ListItem::new(Line::from(spans))
            })
            .collect();

        let list = List::new(items).block(
            Block::default()
                .title("jex tree (交互模式)")
                .borders(Borders::ALL),
        );

        let mut state = ListState::default();
        state.select(Some(app.selected));
        f.render_stateful_widget(list, main_area, &mut state);
    }

    // Status bar
    let status = if app.show_help {
        " ↑↓/jk:移动  →/l:展开  ←/h:折叠  /:搜索  a:全部展开  c:全部折叠  q/Esc:退出  ?:帮助 ".to_string()
    } else if app.input_mode == InputMode::Search {
        format!(" /{}", app.filter)
    } else {
        let total = app.flat.len().saturating_sub(1); // exclude root
        format!(
            " {}/{} dependencies | q/Esc:退出  ?:帮助 ",
            app.selected.min(total),
            total
        )
    };

    let status_bar = Paragraph::new(status).style(Style::default().fg(Color::White).bg(Color::DarkGray));
    f.render_widget(status_bar, status_area);

    // Help overlay
    if app.show_help {
        render_help_overlay(f);
    }
}

fn render_help_overlay(f: &mut Frame) {
    let area = f.area();
    let popup_width = 50.min(area.width - 4);
    let popup_height = 18.min(area.height - 4);
    let x = (area.width - popup_width) / 2;
    let y = (area.height - popup_height) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    f.render_widget(Clear, popup_area);

    let help_text = vec![
        Line::from(Span::styled(
            "  jex tree 交互模式帮助",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  ↑ / k", Style::default().fg(Color::Cyan)),
            Span::raw("      移动到上一个节点"),
        ]),
        Line::from(vec![
            Span::styled("  ↓ / j", Style::default().fg(Color::Cyan)),
            Span::raw("      移动到下一个节点"),
        ]),
        Line::from(vec![
            Span::styled("  → / l", Style::default().fg(Color::Cyan)),
            Span::raw("      展开当前节点"),
        ]),
        Line::from(vec![
            Span::styled("  ← / h", Style::default().fg(Color::Cyan)),
            Span::raw("      折叠当前节点 / 移动到父节点"),
        ]),
        Line::from(vec![
            Span::styled("  /", Style::default().fg(Color::Cyan)),
            Span::raw("        搜索过滤（输入后回车确认，Esc取消）"),
        ]),
        Line::from(vec![
            Span::styled("  a", Style::default().fg(Color::Cyan)),
            Span::raw("        全部展开"),
        ]),
        Line::from(vec![
            Span::styled("  c", Style::default().fg(Color::Cyan)),
            Span::raw("        全部折叠"),
        ]),
        Line::from(vec![
            Span::styled("  g", Style::default().fg(Color::Cyan)),
            Span::raw("        跳到第一个节点"),
        ]),
        Line::from(vec![
            Span::styled("  G", Style::default().fg(Color::Cyan)),
            Span::raw("        跳到最后一个节点"),
        ]),
        Line::from(vec![
            Span::styled("  q / Esc", Style::default().fg(Color::Cyan)),
            Span::raw("     退出"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  按任意键关闭帮助",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let help = Paragraph::new(help_text)
        .block(
            Block::default()
                .title("帮助")
                .borders(Borders::ALL)
                .style(Style::default().fg(Color::White).bg(Color::Black)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(help, popup_area);
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// 运行交互式依赖树浏览 TUI
///
/// 会接管终端（进入 raw mode + alternate screen），退出时恢复。
pub fn run_tree_interactive(tree: &DependencyTree) -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)
        .map_err(|e| Error::new(format!("初始化终端失败: {e}")))?;

    let mut app = App::new(tree);

    let result = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal
        .show_cursor()
        .map_err(|e| Error::new(format!("恢复光标失败: {e}")))?;

    result
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
) -> Result<()> {
    loop {
        terminal
            .draw(|f| ui(f, app))
            .map_err(|e| Error::new(format!("绘制界面失败: {e}")))?;

        if let Event::Key(key) = event::read().map_err(|e| Error::new(format!("读取事件失败: {e}")))? {
            // Only handle key press events (ignore release)
            if key.kind != KeyEventKind::Press {
                continue;
            }

            if app.show_help {
                app.show_help = false;
                continue;
            }

            match app.input_mode {
                InputMode::Normal => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('?') => {
                        app.show_help = true;
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        app.move_up();
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        app.move_down();
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        app.expand();
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        app.collapse();
                    }
                    KeyCode::Char('/') => {
                        app.input_mode = InputMode::Search;
                        app.filter.clear();
                    }
                    KeyCode::Char('a') => {
                        app.expand_all();
                    }
                    KeyCode::Char('c') => {
                        app.collapse_all();
                    }
                    KeyCode::Char('g') => {
                        app.selected = 0;
                    }
                    KeyCode::Char('G') => {
                        app.selected = app.flat.len().saturating_sub(1);
                    }
                    KeyCode::PageUp => {
                        app.selected = app.selected.saturating_sub(10);
                    }
                    KeyCode::PageDown => {
                        app.selected = (app.selected + 10).min(app.flat.len().saturating_sub(1));
                    }
                    _ => {}
                },
                InputMode::Search => match key.code {
                    KeyCode::Esc => {
                        app.input_mode = InputMode::Normal;
                        app.filter.clear();
                        app.refresh();
                    }
                    KeyCode::Enter => {
                        app.input_mode = InputMode::Normal;
                        app.refresh();
                    }
                    KeyCode::Backspace => {
                        app.filter.pop();
                        app.refresh();
                    }
                    KeyCode::Char(c) => {
                        app.filter.push(c);
                        app.refresh();
                        app.selected = 0;
                    }
                    _ => {}
                },
            }
        }
    }
}
