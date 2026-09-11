//! 交互式项目健康度仪表盘（`jex dashboard`）
//!
//! 使用 ratatui + crossterm 全屏 TUI，展示：
//! - 左侧面板：导航菜单（项目概览 / 依赖更新 / 安全审计 / 许可证）
//! - 右侧面板：对应内容展示
//! - Tab 切换面板，↑/↓ 面板内导航
//! - r 刷新数据，q 退出

use crate::audit;
use crate::deps;
use crate::error::{Error, Result};
use crate::license_check;
use crate::outdated;

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
// Panel definitions
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Panel {
    Navigation,
    Content,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Overview,
    Outdated,
    Audit,
    License,
}

impl Tab {
    fn label(&self) -> &'static str {
        match self {
            Tab::Overview => "项目概览",
            Tab::Outdated => "依赖更新",
            Tab::Audit => "安全审计",
            Tab::License => "许可证",
        }
    }

    fn all() -> &'static [Tab] {
        &[Tab::Overview, Tab::Outdated, Tab::Audit, Tab::License]
    }
}

// ---------------------------------------------------------------------------
// Dashboard data
// ---------------------------------------------------------------------------

struct DashboardData {
    project_name: String,
    dependency_count: usize,
    java_version: Option<String>,

    outdated: Option<Vec<outdated::OutdatedDep>>,
    outdated_error: Option<String>,

    audit_report: Option<audit::AuditReport>,
    audit_error: Option<String>,

    license_report: Option<license_check::LicenseReport>,
    license_error: Option<String>,
}

impl DashboardData {
    fn new() -> Self {
        Self {
            project_name: String::from("(未知)"),
            dependency_count: 0,
            java_version: None,
            outdated: None,
            outdated_error: None,
            audit_report: None,
            audit_error: None,
            license_report: None,
            license_error: None,
        }
    }

    fn refresh(&mut self) {
        // Project overview
        match deps::read_jex_toml() {
            Ok(config) => {
                self.project_name = config
                    .project
                    .as_ref()
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| "(未命名)".to_string());
                self.java_version = config
                    .project
                    .as_ref()
                    .and_then(|p| p.java.clone());
                self.dependency_count = config
                    .dependencies
                    .as_ref()
                    .map(|d| d.len())
                    .unwrap_or(0);
            }
            Err(e) => {
                self.project_name = format!("读取失败: {e}");
            }
        }

        // Outdated dependencies
        match outdated::check_outdated() {
            Ok(deps) => {
                self.outdated = Some(deps);
                self.outdated_error = None;
            }
            Err(e) => {
                self.outdated = None;
                self.outdated_error = Some(format!("{e}"));
            }
        }

        // Security audit
        match audit::check_vulnerabilities() {
            Ok(vulns) => {
                let report = audit::generate_report(vulns);
                self.audit_report = Some(report);
                self.audit_error = None;
            }
            Err(e) => {
                self.audit_report = None;
                self.audit_error = Some(format!("{e}"));
            }
        }

        // License check
        match license_check::check_all_licenses() {
            Ok(report) => {
                self.license_report = Some(report);
                self.license_error = None;
            }
            Err(e) => {
                self.license_report = None;
                self.license_error = Some(format!("{e}"));
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Application state
// ---------------------------------------------------------------------------

struct App {
    active_panel: Panel,
    nav_selected: usize,
    content_scroll: usize,
    data: DashboardData,
    show_help: bool,
}

impl App {
    fn new() -> Self {
        let mut data = DashboardData::new();
        data.refresh();
        Self {
            active_panel: Panel::Navigation,
            nav_selected: 0,
            content_scroll: 0,
            data,
            show_help: false,
        }
    }

    fn refresh_data(&mut self) {
        self.data.refresh();
        self.content_scroll = 0;
    }

    fn current_tab(&self) -> Tab {
        Tab::all()[self.nav_selected]
    }

    fn nav_up(&mut self) {
        if self.nav_selected > 0 {
            self.nav_selected -= 1;
            self.content_scroll = 0;
        }
    }

    fn nav_down(&mut self) {
        if self.nav_selected + 1 < Tab::all().len() {
            self.nav_selected += 1;
            self.content_scroll = 0;
        }
    }

    fn scroll_up(&mut self) {
        self.content_scroll = self.content_scroll.saturating_sub(1);
    }

    fn scroll_down(&mut self) {
        self.content_scroll += 1;
    }

    fn switch_panel(&mut self) {
        self.active_panel = match self.active_panel {
            Panel::Navigation => Panel::Content,
            Panel::Content => Panel::Navigation,
        };
    }
}

// ---------------------------------------------------------------------------
// TUI rendering
// ---------------------------------------------------------------------------

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // main area
            Constraint::Length(1), // status bar
        ])
        .split(f.area());

    let main_area = chunks[0];
    let status_area = chunks[1];

    // Split main into left (nav) + right (content)
    let horiz = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(20), // nav panel
            Constraint::Min(0),    // content panel
        ])
        .split(main_area);

    render_nav(f, app, horiz[0]);
    render_content(f, app, horiz[1]);
    render_status_bar(f, app, status_area);

    if app.show_help {
        render_help_overlay(f);
    }
}

fn render_nav(f: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = Tab::all()
        .iter()
        .enumerate()
        .map(|(i, tab)| {
            let style = if i == app.nav_selected && app.active_panel == Panel::Navigation {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else if i == app.nav_selected {
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::DarkGray)
            };

            // Add status indicators
            let indicator = match tab {
                Tab::Outdated => match &app.data.outdated {
                    Some(v) if !v.is_empty() => " ⚠",
                    Some(_) => " ✓",
                    None => " !",
                },
                Tab::Audit => match &app.data.audit_report {
                    Some(r) if r.total_vulnerabilities > 0 => " 🔴",
                    Some(_) => " 🟢",
                    None => " !",
                },
                Tab::License => match &app.data.license_report {
                    Some(r) if !r.incompatible.is_empty() => " ⚠",
                    Some(_) => " ✓",
                    None => " !",
                },
                _ => "",
            };

            let label = format!("  {}{}", tab.label(), indicator);
            ListItem::new(Line::from(Span::styled(label, style)))
        })
        .collect();

    let border_style = if app.active_panel == Panel::Navigation {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let list = List::new(items).block(
        Block::default()
            .title(" 导航 ")
            .borders(Borders::ALL)
            .border_style(border_style),
    );

    let mut state = ListState::default();
    state.select(Some(app.nav_selected));
    f.render_stateful_widget(list, area, &mut state);
}

fn render_content(f: &mut Frame, app: &App, area: Rect) {
    let border_style = if app.active_panel == Panel::Content {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    let block = Block::default()
        .title(format!(" {} ", app.current_tab().label()))
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    f.render_widget(block, area);

    match app.current_tab() {
        Tab::Overview => render_overview(f, app, inner),
        Tab::Outdated => render_outdated(f, app, inner),
        Tab::Audit => render_audit(f, app, inner),
        Tab::License => render_license(f, app, inner),
    }
}

fn render_overview(f: &mut Frame, app: &App, area: Rect) {
    let mut lines: Vec<Line> = Vec::new();

    lines.push(Line::from(Span::styled(
        " 项目概览",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )));
    lines.push(Line::from(""));

    lines.push(Line::from(vec![
        Span::styled("  项目名称: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            app.data.project_name.clone(),
            Style::default().fg(Color::White),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::styled("  依赖数量: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{}", app.data.dependency_count),
            Style::default().fg(Color::Green),
        ),
    ]));

    let java_ver = app
        .data
        .java_version
        .as_deref()
        .unwrap_or("未指定");
    lines.push(Line::from(vec![
        Span::styled("  Java 版本: ", Style::default().fg(Color::DarkGray)),
        Span::styled(java_ver, Style::default().fg(Color::White)),
    ]));

    lines.push(Line::from(""));

    // Summary line
    let outdated_count = app
        .data
        .outdated
        .as_ref()
        .map(|v| v.len())
        .unwrap_or(0);
    let vuln_count = app
        .data
        .audit_report
        .as_ref()
        .map(|r| r.total_vulnerabilities)
        .unwrap_or(0);
    let incompatible_count = app
        .data
        .license_report
        .as_ref()
        .map(|r| r.incompatible.len())
        .unwrap_or(0);

    lines.push(Line::from(vec![
        Span::styled("  依赖更新: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} 个可更新", outdated_count),
            Style::default().fg(if outdated_count > 0 {
                Color::Yellow
            } else {
                Color::Green
            }),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::styled("  安全漏洞: ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} 个", vuln_count),
            Style::default().fg(if vuln_count > 0 {
                Color::Red
            } else {
                Color::Green
            }),
        ),
    ]));

    lines.push(Line::from(vec![
        Span::styled("  许可证:   ", Style::default().fg(Color::DarkGray)),
        Span::styled(
            format!("{} 个不兼容", incompatible_count),
            Style::default().fg(if incompatible_count > 0 {
                Color::Red
            } else {
                Color::Green
            }),
        ),
    ]));

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
    f.render_widget(paragraph, area);
}

fn render_outdated(f: &mut Frame, app: &App, area: Rect) {
    if let Some(ref err) = app.data.outdated_error {
        let msg = vec![
            Line::from(Span::styled(
                " ⚠ 数据不可用",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!("  {err}"),
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  提示: 请确认 Coursier (cs) 已安装且网络可用",
                Style::default().fg(Color::DarkGray),
            )),
        ];
        let paragraph = Paragraph::new(msg).wrap(Wrap { trim: false });
        f.render_widget(paragraph, area);
        return;
    }

    match &app.data.outdated {
        Some(deps) if deps.is_empty() => {
            let msg = vec![
                Line::from(Span::styled(
                    " ✅ 所有依赖已是最新版本",
                    Style::default().fg(Color::Green),
                )),
            ];
            let paragraph = Paragraph::new(msg).wrap(Wrap { trim: false });
            f.render_widget(paragraph, area);
        }
        Some(deps) => {
            let mut lines: Vec<Line> = Vec::new();
            lines.push(Line::from(vec![
                Span::styled(
                    format!(" 找到 {} 个可更新依赖", deps.len()),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::from(""));

            for dep in deps {
                lines.push(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(
                        format!("{}:{}", dep.group, dep.artifact),
                        Style::default().fg(Color::White),
                    ),
                ]));
                lines.push(Line::from(vec![
                    Span::styled(
                        "    ",
                        Style::default(),
                    ),
                    Span::styled(
                        format!("{} → {}", dep.current, dep.latest),
                        Style::default().fg(Color::Cyan),
                    ),
                ]));
            }

            // Apply scroll offset
            let scrolled: Vec<Line> = lines
                .into_iter()
                .skip(app.content_scroll)
                .collect();

            let paragraph = Paragraph::new(scrolled).wrap(Wrap { trim: false });
            f.render_widget(paragraph, area);
        }
        None => {
            let msg = vec![
                Line::from(Span::styled(
                    "  加载中...",
                    Style::default().fg(Color::DarkGray),
                )),
            ];
            let paragraph = Paragraph::new(msg).wrap(Wrap { trim: false });
            f.render_widget(paragraph, area);
        }
    }
}

fn render_audit(f: &mut Frame, app: &App, area: Rect) {
    if let Some(ref err) = app.data.audit_error {
        let msg = vec![
            Line::from(Span::styled(
                " ⚠ 数据不可用",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!("  {err}"),
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  提示: 请确认网络可访问 OSV 数据库",
                Style::default().fg(Color::DarkGray),
            )),
        ];
        let paragraph = Paragraph::new(msg).wrap(Wrap { trim: false });
        f.render_widget(paragraph, area);
        return;
    }

    match &app.data.audit_report {
        Some(report) if report.total_vulnerabilities == 0 => {
            let msg = vec![Line::from(Span::styled(
                " ✅ 未发现安全漏洞",
                Style::default().fg(Color::Green),
            ))];
            let paragraph = Paragraph::new(msg).wrap(Wrap { trim: false });
            f.render_widget(paragraph, area);
        }
        Some(report) => {
            let mut lines: Vec<Line> = Vec::new();

            lines.push(Line::from(Span::styled(
                format!(" 发现 {} 个安全漏洞", report.total_vulnerabilities),
                Style::default()
                    .fg(Color::Red)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));

            // Severity summary
            lines.push(Line::from(vec![
                Span::styled("  CRITICAL: ", Style::default().fg(Color::Red)),
                Span::styled(
                    format!("{}", report.critical),
                    Style::default().fg(Color::Red),
                ),
                Span::styled("  HIGH: ", Style::default().fg(Color::Red)),
                Span::styled(
                    format!("{}", report.high),
                    Style::default().fg(Color::Red),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  MEDIUM:   ", Style::default().fg(Color::Yellow)),
                Span::styled(
                    format!("{}", report.medium),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled("  LOW:      ", Style::default().fg(Color::Green)),
                Span::styled(
                    format!("{}", report.low),
                    Style::default().fg(Color::Green),
                ),
            ]));
            lines.push(Line::from(""));

            for vuln in &report.vulnerabilities {
                let sev_color = match vuln.severity {
                    audit::Severity::Critical | audit::Severity::High => Color::Red,
                    audit::Severity::Medium => Color::Yellow,
                    audit::Severity::Low => Color::Green,
                };

                lines.push(Line::from(vec![
                    Span::styled("  ", Style::default()),
                    Span::styled(
                        vuln.severity.display().to_string(),
                        Style::default().fg(sev_color),
                    ),
                    Span::styled(
                        format!(" [{}] {}:{}", vuln.cve_id, vuln.group, vuln.artifact),
                        Style::default().fg(Color::White),
                    ),
                ]));
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("    {}", vuln.description),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
                if let Some(fix) = audit::get_fix_suggestion(vuln) {
                    lines.push(Line::from(vec![
                        Span::styled(
                            format!("    修复: {fix}"),
                            Style::default().fg(Color::Cyan),
                        ),
                    ]));
                }
                lines.push(Line::from(""));
            }

            let scrolled: Vec<Line> = lines
                .into_iter()
                .skip(app.content_scroll)
                .collect();

            let paragraph = Paragraph::new(scrolled).wrap(Wrap { trim: false });
            f.render_widget(paragraph, area);
        }
        None => {
            let msg = vec![Line::from(Span::styled(
                "  加载中...",
                Style::default().fg(Color::DarkGray),
            ))];
            let paragraph = Paragraph::new(msg).wrap(Wrap { trim: false });
            f.render_widget(paragraph, area);
        }
    }
}

fn render_license(f: &mut Frame, app: &App, area: Rect) {
    if let Some(ref err) = app.data.license_error {
        let msg = vec![
            Line::from(Span::styled(
                " ⚠ 数据不可用",
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                format!("  {err}"),
                Style::default().fg(Color::DarkGray),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "  提示: 请确认 Coursier (cs) 已安装且网络可用",
                Style::default().fg(Color::DarkGray),
            )),
        ];
        let paragraph = Paragraph::new(msg).wrap(Wrap { trim: false });
        f.render_widget(paragraph, area);
        return;
    }

    match &app.data.license_report {
        Some(report) => {
            let mut lines: Vec<Line> = Vec::new();

            // Summary
            lines.push(Line::from(Span::styled(
                " 许可证合规性报告",
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )));
            lines.push(Line::from(""));

            let total = report.compatible.len()
                + report.incompatible.len()
                + report.unknown.len();

            lines.push(Line::from(vec![
                Span::styled("  总计:   ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{total} 个依赖"),
                    Style::default().fg(Color::White),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  兼容:   ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{} 个", report.compatible.len()),
                    Style::default().fg(Color::Green),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  不兼容: ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{} 个", report.incompatible.len()),
                    Style::default().fg(Color::Red),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled("  未知:   ", Style::default().fg(Color::DarkGray)),
                Span::styled(
                    format!("{} 个", report.unknown.len()),
                    Style::default().fg(Color::Yellow),
                ),
            ]));

            if !report.incompatible.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    " 不兼容的依赖:",
                    Style::default()
                        .fg(Color::Red)
                        .add_modifier(Modifier::BOLD),
                )));
                for dep in &report.incompatible {
                    lines.push(Line::from(vec![
                        Span::styled(format!("  ❌ {dep}"), Style::default().fg(Color::Red)),
                    ]));
                }
            }

            if !report.unknown.is_empty() {
                lines.push(Line::from(""));
                lines.push(Line::from(Span::styled(
                    " 未知许可证:",
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )));
                for dep in &report.unknown {
                    lines.push(Line::from(vec![
                        Span::styled(format!("  ❓ {dep}"), Style::default().fg(Color::Yellow)),
                    ]));
                }
            }

            let scrolled: Vec<Line> = lines
                .into_iter()
                .skip(app.content_scroll)
                .collect();

            let paragraph = Paragraph::new(scrolled).wrap(Wrap { trim: false });
            f.render_widget(paragraph, area);
        }
        None => {
            let msg = vec![Line::from(Span::styled(
                "  加载中...",
                Style::default().fg(Color::DarkGray),
            ))];
            let paragraph = Paragraph::new(msg).wrap(Wrap { trim: false });
            f.render_widget(paragraph, area);
        }
    }
}

fn render_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let status = if app.show_help {
        " Tab:切换面板  ↑↓:导航  r:刷新  q:退出  ?:帮助 ".to_string()
    } else {
        let panel_hint = match app.active_panel {
            Panel::Navigation => "[导航]",
            Panel::Content => "[内容]",
        };
        format!(
            " {} jex dashboard | Tab:切换  ↑↓:导航  r:刷新  q:退出  ?:帮助 ",
            panel_hint
        )
    };

    let status_bar = Paragraph::new(status).style(
        Style::default()
            .fg(Color::White)
            .bg(Color::DarkGray),
    );
    f.render_widget(status_bar, area);
}

fn render_help_overlay(f: &mut Frame) {
    let area = f.area();
    let popup_width = 48.min(area.width - 4);
    let popup_height = 16.min(area.height - 4);
    let x = (area.width - popup_width) / 2;
    let y = (area.height - popup_height) / 2;
    let popup_area = Rect::new(x, y, popup_width, popup_height);

    f.render_widget(Clear, popup_area);

    let help_text = vec![
        Line::from(Span::styled(
            "  jex dashboard 帮助",
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Tab", Style::default().fg(Color::Cyan)),
            Span::raw("      切换导航/内容面板"),
        ]),
        Line::from(vec![
            Span::styled("  ↑ / k", Style::default().fg(Color::Cyan)),
            Span::raw("      向上移动"),
        ]),
        Line::from(vec![
            Span::styled("  ↓ / j", Style::default().fg(Color::Cyan)),
            Span::raw("      向下移动"),
        ]),
        Line::from(vec![
            Span::styled("  r", Style::default().fg(Color::Cyan)),
            Span::raw("        刷新数据"),
        ]),
        Line::from(vec![
            Span::styled("  q / Esc", Style::default().fg(Color::Cyan)),
            Span::raw("     退出仪表盘"),
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

/// 运行交互式项目健康度仪表盘
///
/// 会接管终端（进入 raw mode + alternate screen），退出时恢复。
pub fn run_dashboard() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)
        .map_err(|e| Error::new(format!("初始化终端失败: {e}")))?;

    let mut app = App::new();

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

        if let Event::Key(key) =
            event::read().map_err(|e| Error::new(format!("读取事件失败: {e}")))?
        {
            if key.kind != KeyEventKind::Press {
                continue;
            }

            if app.show_help {
                app.show_help = false;
                continue;
            }

            match key.code {
                KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                KeyCode::Char('?') => {
                    app.show_help = true;
                }
                KeyCode::Char('r') => {
                    app.refresh_data();
                }
                KeyCode::Tab => {
                    app.switch_panel();
                }
                KeyCode::Up | KeyCode::Char('k') => match app.active_panel {
                    Panel::Navigation => app.nav_up(),
                    Panel::Content => app.scroll_up(),
                },
                KeyCode::Down | KeyCode::Char('j') => match app.active_panel {
                    Panel::Navigation => app.nav_down(),
                    Panel::Content => app.scroll_down(),
                },
                KeyCode::Home => {
                    app.content_scroll = 0;
                }
                KeyCode::End => {
                    app.content_scroll = usize::MAX;
                }
                KeyCode::PageUp => {
                    app.content_scroll = app.content_scroll.saturating_sub(10);
                }
                KeyCode::PageDown => {
                    app.content_scroll += 10;
                }
                _ => {}
            }
        }
    }
}
