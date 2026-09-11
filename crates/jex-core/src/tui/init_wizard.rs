//! 交互式项目初始化向导（`jex init -i`）
//!
//! 使用 ratatui + crossterm 全屏 TUI，5 步向导：
//! 1. 项目名称输入
//! 2. Java 版本选择
//! 3. 构建工具选择
//! 4. 常用依赖选择（多选）
//! 5. 配置预览 + 确认
//!
//! 快捷键：
//! - ←/→ 切换步骤
//! - ↑/↓ 在步骤内导航
//! - Space 选择/取消（checkbox）
//! - Enter 确认（最终步骤）
//! - q/Esc 取消

use crate::error::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use std::io;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

const TOTAL_STEPS: usize = 5;

const JAVA_VERSIONS: &[(&str, &str)] = &[
    ("8", "Java 8 (LTS)"),
    ("11", "Java 11 (LTS)"),
    ("17", "Java 17 (LTS)"),
    ("21", "Java 21 (LTS, recommended)"),
];

const BUILD_TOOLS: &[(&str, &str)] = &[
    ("javac", "Pure javac (no build tool)"),
    ("maven", "Apache Maven"),
    ("gradle", "Gradle"),
];

struct DepEntry {
    name: &'static str,
    group: &'static str,
    artifact: &'static str,
    default_version: &'static str,
}

const COMMON_DEPS: &[DepEntry] = &[
    DepEntry {
        name: "JUnit 5",
        group: "org.junit.jupiter",
        artifact: "junit-jupiter-api",
        default_version: "5.10.2",
    },
    DepEntry {
        name: "Gson",
        group: "com.google.code.gson",
        artifact: "gson",
        default_version: "2.11.0",
    },
    DepEntry {
        name: "Lombok",
        group: "org.projectlombok",
        artifact: "lombok",
        default_version: "1.18.32",
    },
    DepEntry {
        name: "SLF4J API",
        group: "org.slf4j",
        artifact: "slf4j-api",
        default_version: "2.0.13",
    },
    DepEntry {
        name: "Log4j Core",
        group: "org.apache.logging.log4j",
        artifact: "log4j-core",
        default_version: "2.23.1",
    },
    DepEntry {
        name: "Apache Commons Lang",
        group: "org.apache.commons",
        artifact: "commons-lang3",
        default_version: "3.14.0",
    },
    DepEntry {
        name: "Jackson Databind",
        group: "com.fasterxml.jackson.core",
        artifact: "jackson-databind",
        default_version: "2.17.1",
    },
];

// ---------------------------------------------------------------------------
// Step definitions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Step {
    Name = 0,
    JavaVersion = 1,
    BuildTool = 2,
    Dependencies = 3,
    Preview = 4,
}

impl Step {
    fn from_index(i: usize) -> Self {
        match i {
            0 => Step::Name,
            1 => Step::JavaVersion,
            2 => Step::BuildTool,
            3 => Step::Dependencies,
            4 => Step::Preview,
            _ => Step::Preview,
        }
    }
    fn title(self) -> &'static str {
        match self {
            Step::Name => "Project Name",
            Step::JavaVersion => "Java Version",
            Step::BuildTool => "Build Tool",
            Step::Dependencies => "Dependencies",
            Step::Preview => "Preview & Confirm",
        }
    }
}

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

struct App {
    current_step: usize,
    step_cursor: usize, // cursor within the current step's list

    // Step 1: project name
    name_input: String,
    name_cursor: usize,

    // Step 2: java version
    java_selected: usize,

    // Step 3: build tool
    build_selected: usize,

    // Step 4: dependencies (multi-select)
    deps_selected: Vec<bool>,
    deps_cursor: usize,

    // Global
    cancelled: bool,
    confirmed: bool,
}

impl App {
    fn new() -> Self {
        let default_name = std::env::current_dir()
            .ok()
            .and_then(|p| {
                p.file_name()
                    .map(|n| n.to_string_lossy().into_owned())
            })
            .unwrap_or_else(|| "demo".to_string());

        let deps_len = COMMON_DEPS.len();
        Self {
            current_step: 0,
            step_cursor: 0,
            name_input: default_name,
            name_cursor: 0,
            java_selected: 2, // default to Java 17
            build_selected: 0,
            deps_selected: vec![false; deps_len],
            deps_cursor: 0,
            cancelled: false,
            confirmed: false,
        }
    }

    fn max_cursor(&self) -> usize {
        match Step::from_index(self.current_step) {
            Step::Name => 0, // no list navigation in name step
            Step::JavaVersion => JAVA_VERSIONS.len().saturating_sub(1),
            Step::BuildTool => BUILD_TOOLS.len().saturating_sub(1),
            Step::Dependencies => COMMON_DEPS.len().saturating_sub(1),
            Step::Preview => 0, // no list navigation in preview
        }
    }

    fn move_up(&mut self) {
        if self.step_cursor > 0 {
            self.step_cursor -= 1;
        }
    }

    fn move_down(&mut self) {
        if self.step_cursor < self.max_cursor() {
            self.step_cursor += 1;
        }
    }

    fn next_step(&mut self) {
        if self.current_step < TOTAL_STEPS - 1 {
            self.current_step += 1;
            self.step_cursor = match Step::from_index(self.current_step) {
                Step::JavaVersion => self.java_selected,
                Step::BuildTool => self.build_selected,
                Step::Dependencies => self.deps_cursor,
                _ => 0,
            };
        }
    }

    fn prev_step(&mut self) {
        if self.current_step > 0 {
            self.current_step -= 1;
            self.step_cursor = match Step::from_index(self.current_step) {
                Step::JavaVersion => self.java_selected,
                Step::BuildTool => self.build_selected,
                Step::Dependencies => self.deps_cursor,
                _ => 0,
            };
        }
    }

    fn toggle_dep(&mut self) {
        if self.step_cursor < self.deps_selected.len() {
            self.deps_selected[self.step_cursor] = !self.deps_selected[self.step_cursor];
        }
    }

    fn name_insert_char(&mut self, c: char) {
        self.name_input.insert(self.name_cursor, c);
        self.name_cursor += 1;
    }

    fn name_delete_char(&mut self) {
        if self.name_cursor > 0 {
            self.name_cursor -= 1;
            self.name_input.remove(self.name_cursor);
        }
    }

    fn build_jex_toml_content(&self) -> String {
        let name = &self.name_input;
        let java_ver = JAVA_VERSIONS[self.java_selected].0;
        let build_tool = BUILD_TOOLS[self.build_selected].0;

        let mut deps_section = String::new();
        for (i, dep) in COMMON_DEPS.iter().enumerate() {
            if self.deps_selected.get(i).copied().unwrap_or(false) {
                let coord = format!("{}:{}", dep.group, dep.artifact);
                deps_section.push_str(&format!(
                    "\"{}\" = \"{}\"\n",
                    coord, dep.default_version
                ));
            }
        }

        let mut output = String::new();
        output.push_str(&format!("[project]\nname = \"{}\"\n", name));
        output.push_str(&format!("java = \"{}\"\n\n", java_ver));

        output.push_str("[build]\n");
        output.push_str("output = \".jex-build\"\n");
        output.push_str("sources = [\"src\"]\n");
        output.push_str("resources = [\"src/main/resources\"]\n");
        output.push_str("compiler_args = [\"-parameters\", \"-encoding\", \"UTF-8\"]\n");

        if build_tool != "javac" {
            output.push_str(&format!("build_tool = \"{}\"\n", build_tool));
        }

        if !deps_section.is_empty() {
            output.push_str(&format!("\n[dependencies]\n{}", deps_section));
        }

        output.push_str("\n[repositories]\nmaven-central = true\n");

        output
    }
}

// ---------------------------------------------------------------------------
// TUI rendering
// ---------------------------------------------------------------------------

fn ui(f: &mut Frame, app: &App) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // title + step indicator
            Constraint::Min(0),   // main content
            Constraint::Length(3), // navigation bar
        ])
        .split(area);

    render_title_bar(f, app, chunks[0]);
    render_step_content(f, app, chunks[1]);
    render_nav_bar(f, app, chunks[2]);
}

fn render_title_bar(f: &mut Frame, app: &App, area: Rect) {
    let step_name = Step::from_index(app.current_step).title();
    let indicator = format!(
        "  jex init  ─  Step {}/{}: {}  ",
        app.current_step + 1,
        TOTAL_STEPS,
        step_name
    );

    let title = Paragraph::new(Line::from(Span::styled(
        indicator,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )))
    .block(
        Block::default()
            .title(" Project Initialization Wizard ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );
    f.render_widget(title, area);
}

fn render_step_content(f: &mut Frame, app: &App, area: Rect) {
    match Step::from_index(app.current_step) {
        Step::Name => render_name_step(f, app, area),
        Step::JavaVersion => render_list_step(f, app, area, "Select Java version", JAVA_VERSIONS, app.java_selected),
        Step::BuildTool => render_list_step(f, app, area, "Select build tool", BUILD_TOOLS, app.build_selected),
        Step::Dependencies => render_deps_step(f, app, area),
        Step::Preview => render_preview_step(f, app, area),
    }
}

fn render_name_step(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // input box
            Constraint::Length(2), // hint
            Constraint::Min(0),   // spacer
        ])
        .split(area);

    // Input display — mask cursor with a block cursor char
    let mut display = app.name_input.clone();
    let cursor_pos = app.name_cursor.min(display.len());
    display.insert(cursor_pos, '▌');

    let input_block = Paragraph::new(Line::from(Span::styled(
        &display,
        Style::default().fg(Color::White),
    )))
    .block(
        Block::default()
            .title(" Project Name ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan)),
    );
    f.render_widget(input_block, chunks[0]);

    let hint = Paragraph::new(Line::from(vec![
        Span::styled(
            "Type the project name. ",
            Style::default().fg(Color::DarkGray),
        ),
        Span::styled(
            "Press → to continue",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
    ]));
    f.render_widget(hint, chunks[1]);
}

fn render_list_step(
    f: &mut Frame,
    app: &App,
    area: Rect,
    title: &str,
    items: &[(&str, &str)],
    selected: usize,
) {
    let list_items: Vec<ListItem> = items
        .iter()
        .enumerate()
        .map(|(i, (val, desc))| {
            let is_cursor = i == app.step_cursor;
            let is_selected = i == selected;

            let marker = if is_cursor { "▶ " } else { "  " };
            let check = if is_selected { "● " } else { "○ " };

            let style = if is_cursor {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if is_selected {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::White)
            };

            ListItem::new(Line::from(vec![
                Span::styled(marker, style),
                Span::styled(check, style),
                Span::styled(desc.to_string(), style),
                Span::styled(
                    format!("  ({})", val),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();

    let list = List::new(list_items).block(
        Block::default()
            .title(format!(" {} ", title))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    f.render_widget(list, area);
}

fn render_deps_step(f: &mut Frame, app: &App, area: Rect) {
    let list_items: Vec<ListItem> = COMMON_DEPS
        .iter()
        .enumerate()
        .map(|(i, dep)| {
            let is_cursor = i == app.step_cursor;
            let is_checked = app.deps_selected.get(i).copied().unwrap_or(false);

            let marker = if is_cursor { "▶ " } else { "  " };
            let check = if is_checked { "☑ " } else { "☐ " };

            let style = if is_cursor {
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD)
            } else if is_checked {
                Style::default().fg(Color::Green)
            } else {
                Style::default().fg(Color::White)
            };

            let coord = format!("{}:{}", dep.group, dep.artifact);

            ListItem::new(Line::from(vec![
                Span::styled(marker, style),
                Span::styled(check, style),
                Span::styled(dep.name.to_string(), style),
                Span::styled(
                    format!("  {}", coord),
                    Style::default().fg(Color::DarkGray),
                ),
            ]))
        })
        .collect();

    let list = List::new(list_items).block(
        Block::default()
            .title(" Select Dependencies (Space to toggle) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray)),
    );

    f.render_widget(list, area);
}

fn render_preview_step(f: &mut Frame, app: &App, area: Rect) {
    let content = app.build_jex_toml_content();

    let preview = Paragraph::new(content)
        .block(
            Block::default()
                .title(" jex.toml Preview ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(preview, area);
}

fn render_nav_bar(f: &mut Frame, app: &App, area: Rect) {
    let nav_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),   // navigation hints
            Constraint::Length(1), // spacer
            Constraint::Length(20), // step dots
        ])
        .split(area);

    // Step progress dots
    let mut dots = String::new();
    for i in 0..TOTAL_STEPS {
        if i == app.current_step {
            dots.push_str("● ");
        } else if i < app.current_step {
            dots.push_str("◆ ");
        } else {
            dots.push_str("○ ");
        }
    }

    let dots_widget = Paragraph::new(Line::from(Span::styled(
        dots.trim(),
        Style::default().fg(Color::Cyan),
    )))
    .alignment(ratatui::layout::Alignment::Right);
    f.render_widget(dots_widget, nav_chunks[2]);

    // Navigation hints
    let hints = match Step::from_index(app.current_step) {
        Step::Name => vec![
            Span::styled("← →", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" switch step  "),
            Span::styled("Enter", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" confirm  "),
            Span::styled("q/Esc", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw(" cancel"),
        ],
        Step::Preview => vec![
            Span::styled("←", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" back  "),
            Span::styled("Enter", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" generate jex.toml  "),
            Span::styled("q/Esc", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw(" cancel"),
        ],
        Step::Dependencies => vec![
            Span::styled("← →", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" switch step  "),
            Span::styled("↑ ↓", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" navigate  "),
            Span::styled("Space", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" toggle  "),
            Span::styled("q/Esc", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw(" cancel"),
        ],
        _ => vec![
            Span::styled("← →", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::raw(" switch step  "),
            Span::styled("↑ ↓", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            Span::raw(" navigate  "),
            Span::styled("Enter", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" select  "),
            Span::styled("q/Esc", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw(" cancel"),
        ],
    };

    let nav_bar = Paragraph::new(Line::from(hints));
    f.render_widget(nav_bar, nav_chunks[0]);
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run the interactive project initialization wizard.
///
/// Returns `Ok(true)` when user confirmed and jex.toml was generated,
/// `Ok(false)` when cancelled, or `Err` on I/O failures.
pub fn run_init_wizard() -> Result<()> {
    // Check if jex.toml already exists
    let cwd = std::env::current_dir()?;
    let jex_toml = cwd.join("jex.toml");
    if jex_toml.exists() {
        eprintln!("当前目录已存在 jex.toml，请先删除或在其他目录操作");
        return Ok(());
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run_app(&mut terminal);

    // Restore terminal
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

    match result {
        Ok(true) => {
            println!("✓ jex.toml 已生成，项目初始化完成！");
            Ok(())
        }
        Ok(false) => {
            println!("已取消项目初始化");
            Ok(())
        }
        Err(e) => Err(e),
    }
}

fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<bool> {
    let mut app = App::new();

    loop {
        terminal.draw(|f| ui(f, &app))?;

        if let Event::Key(key) = event::read()? {
            // Ignore key release events
            if key.kind != KeyEventKind::Press {
                continue;
            }

            // Global keys
            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => {
                    app.cancelled = true;
                    return Ok(false);
                }
                KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    app.cancelled = true;
                    return Ok(false);
                }
                _ => {}
            }

            // Step-specific key handling
            match Step::from_index(app.current_step) {
                Step::Name => match key.code {
                    KeyCode::Right => app.next_step(),
                    KeyCode::Left => app.prev_step(),
                    KeyCode::Char(c) if !c.is_control() => app.name_insert_char(c),
                    KeyCode::Backspace => app.name_delete_char(),
                    KeyCode::Enter if !app.name_input.trim().is_empty() => {
                        app.next_step();
                    }
                    _ => {}
                },
                Step::JavaVersion | Step::BuildTool => match key.code {
                    KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                    KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                    KeyCode::Right => app.next_step(),
                    KeyCode::Left => app.prev_step(),
                    KeyCode::Enter => {
                        // Save selection and move on
                        match Step::from_index(app.current_step) {
                            Step::JavaVersion => app.java_selected = app.step_cursor,
                            Step::BuildTool => app.build_selected = app.step_cursor,
                            _ => {}
                        }
                        app.next_step();
                    }
                    _ => {}
                },
                Step::Dependencies => match key.code {
                    KeyCode::Up | KeyCode::Char('k') => app.move_up(),
                    KeyCode::Down | KeyCode::Char('j') => app.move_down(),
                    KeyCode::Right => {
                        app.deps_cursor = app.step_cursor;
                        app.next_step();
                    }
                    KeyCode::Left => {
                        app.deps_cursor = app.step_cursor;
                        app.prev_step();
                    }
                    KeyCode::Char(' ') => app.toggle_dep(),
                    KeyCode::Enter => {
                        app.deps_cursor = app.step_cursor;
                        app.next_step();
                    }
                    _ => {}
                },
                Step::Preview => match key.code {
                    KeyCode::Left => app.prev_step(),
                    KeyCode::Enter => {
                        // Confirm: write jex.toml
                        write_jex_toml_from_wizard(&app)?;
                        app.confirmed = true;
                        return Ok(true);
                    }
                    _ => {}
                },
            }
        }
    }
}

/// Write the jex.toml file from wizard state.
fn write_jex_toml_from_wizard(app: &App) -> Result<()> {
    let content = app.build_jex_toml_content();
    let cwd = std::env::current_dir()?;
    let path = cwd.join("jex.toml");

    // Atomic write: temp file then rename
    let tmp_path = path.with_extension("toml.tmp");
    std::fs::write(&tmp_path, &content)?;
    std::fs::rename(&tmp_path, &path)?;

    // Create src directory
    std::fs::create_dir_all(cwd.join("src"))?;
    std::fs::create_dir_all(cwd.join("src/main/resources"))?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_new_default_name() {
        let app = App::new();
        // Default name should be the current directory name or "demo"
        assert!(!app.name_input.is_empty());
        assert_eq!(app.current_step, 0);
        assert_eq!(app.java_selected, 2); // Java 17
        assert_eq!(app.build_selected, 0); // javac
    }

    #[test]
    fn test_app_navigation() {
        let mut app = App::new();
        assert_eq!(app.current_step, 0);

        app.next_step();
        assert_eq!(app.current_step, 1);

        app.next_step();
        assert_eq!(app.current_step, 2);

        app.next_step();
        assert_eq!(app.current_step, 3);

        app.next_step();
        assert_eq!(app.current_step, 4);

        // Can't go past last step
        app.next_step();
        assert_eq!(app.current_step, 4);

        app.prev_step();
        assert_eq!(app.current_step, 3);
    }

    #[test]
    fn test_name_editing() {
        let mut app = App::new();
        app.name_input.clear();
        app.name_cursor = 0;

        app.name_insert_char('h');
        app.name_insert_char('i');
        assert_eq!(app.name_input, "hi");
        assert_eq!(app.name_cursor, 2);

        app.name_delete_char();
        assert_eq!(app.name_input, "h");
        assert_eq!(app.name_cursor, 1);
    }

    #[test]
    fn test_dep_toggle() {
        let mut app = App::new();
        app.current_step = 3; // Dependencies step
        app.step_cursor = 0;

        assert!(!app.deps_selected[0]);
        app.toggle_dep();
        assert!(app.deps_selected[0]);
        app.toggle_dep();
        assert!(!app.deps_selected[0]);
    }

    #[test]
    fn test_jex_toml_content_minimal() {
        let mut app = App::new();
        app.name_input = "test-project".to_string();
        app.java_selected = 3; // Java 21
        app.build_selected = 1; // Maven
        // No deps selected
        let content = app.build_jex_toml_content();
        assert!(content.contains("name = \"test-project\""));
        assert!(content.contains("java = \"21\""));
        assert!(content.contains("build_tool = \"maven\""));
        assert!(!content.contains("[dependencies]"));
    }

    #[test]
    fn test_jex_toml_content_with_deps() {
        let mut app = App::new();
        app.name_input = "myapp".to_string();
        app.java_selected = 2; // Java 17
        app.build_selected = 0; // javac
        app.deps_selected[0] = true; // JUnit 5
        app.deps_selected[1] = true; // Gson
        let content = app.build_jex_toml_content();
        assert!(content.contains("[dependencies]"));
        assert!(content.contains("org.junit.jupiter:junit-jupiter-api"));
        assert!(content.contains("com.google.code.gson:gson"));
        assert!(!content.contains("build_tool"));
    }

    #[test]
    fn test_step_from_index() {
        assert_eq!(Step::from_index(0), Step::Name);
        assert_eq!(Step::from_index(4), Step::Preview);
        assert_eq!(Step::from_index(99), Step::Preview); // out of bounds
    }

    #[test]
    fn test_max_cursor_values() {
        let mut app = App::new();
        app.current_step = 1; // JavaVersion
        assert_eq!(app.max_cursor(), JAVA_VERSIONS.len() - 1);

        app.current_step = 2; // BuildTool
        assert_eq!(app.max_cursor(), BUILD_TOOLS.len() - 1);

        app.current_step = 3; // Dependencies
        assert_eq!(app.max_cursor(), COMMON_DEPS.len() - 1);
    }

    #[test]
    fn test_move_up_down_limits() {
        let mut app = App::new();
        app.current_step = 1; // JavaVersion

        // Start at 0, can't move up
        app.step_cursor = 0;
        app.move_up();
        assert_eq!(app.step_cursor, 0);

        // Move down
        app.move_down();
        assert_eq!(app.step_cursor, 1);

        // Move to end
        let max = app.max_cursor();
        app.step_cursor = max;
        app.move_down();
        assert_eq!(app.step_cursor, max);
    }
}
