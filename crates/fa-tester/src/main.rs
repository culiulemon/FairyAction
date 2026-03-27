use std::sync::Arc;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen};
use fa_browser::profile::BrowserProfile;
use fa_browser::session::BrowserSession;
use fa_config::Config;
use fa_dom::service::DomService;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, Paragraph, Wrap};
use ratatui::Frame;

#[derive(Debug)]
struct LogEntry {
    timestamp: String,
    message: String,
    level: LogLevel,
}

#[derive(Debug, Clone)]
enum LogLevel {
    Info,
    Success,
    Error,
    Command,
}

struct App {
    browser: Arc<BrowserSession>,
    dom_content: String,
    dom_scroll: usize,
    url: String,
    title: String,
    current_tab: usize,
    logs: Vec<LogEntry>,
    input: String,
    cursor_position: usize,
    should_quit: bool,
    show_help: bool,
    pending_command: Option<String>,
    pending_refresh: bool,
    pending_reload: bool,
    pending_screenshot: bool,
    pending_new_tab: bool,
    pending_close_tab: bool,
}

impl App {
    async fn new() -> anyhow::Result<Self> {
        let config = Config::load();
        let mut browser_config = config.browser.clone();
        browser_config.headless = false;

        let profile = BrowserProfile::from_config(&browser_config);
        let session = BrowserSession::new(profile).await?;
        let browser = Arc::new(session);

        let url = browser.get_url().await.unwrap_or_else(|_| "about:blank".to_string());
        let title = browser.get_title().await.unwrap_or_default();

        let mut app = Self {
            browser,
            dom_content: String::new(),
            dom_scroll: 0,
            url,
            title,
            current_tab: 0,
            logs: Vec::new(),
            input: String::new(),
            cursor_position: 0,
            should_quit: false,
            show_help: false,
            pending_command: None,
            pending_refresh: false,
            pending_reload: false,
            pending_screenshot: false,
            pending_new_tab: false,
            pending_close_tab: false,
        };

        app.add_log(LogLevel::Info, "FairyAction Tester started. Type 'help' for available commands.");
        app.refresh_dom().await?;
        app.refresh_status().await;

        Ok(app)
    }

    async fn refresh_dom(&mut self) -> anyhow::Result<()> {
        match DomService::get_dom_state(&self.browser).await {
            Ok(state) => {
                self.dom_content = state.llm_representation;
                let count = state.selector_map.len();
                self.add_log(LogLevel::Success, &format!("DOM refreshed: {} interactive elements", count));
            }
            Err(e) => {
                self.add_log(LogLevel::Error, &format!("DOM refresh failed: {}", e));
            }
        }
        Ok(())
    }

    async fn refresh_status(&mut self) {
        self.url = self.browser.get_url().await.unwrap_or_default();
        self.title = self.browser.get_title().await.unwrap_or_default();
    }

    async fn execute_command(&mut self, cmd: &str) {
        let parts: Vec<&str> = cmd.trim().splitn(2, ' ').collect();
        let command = parts[0].to_lowercase();
        let args = if parts.len() > 1 { parts[1].trim() } else { "" };

        self.add_log(LogLevel::Command, cmd);

        match command.as_str() {
            "navigate" | "nav" => {
                if args.is_empty() {
                    self.add_log(LogLevel::Error, "Usage: navigate <url>");
                    return;
                }
                match self.browser.navigate(args).await {
                    Ok(_) => {
                        self.add_log(LogLevel::Success, &format!("Navigated to {}", args));
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        let _ = self.refresh_dom().await;
                    }
                    Err(e) => self.add_log(LogLevel::Error, &format!("Navigate failed: {}", e)),
                }
            }
            "click" => {
                let index_str = args.trim_start_matches('[').trim_end_matches(']');
                if let Ok(index) = index_str.parse::<usize>() {
                    match self.browser.click_element(index).await {
                        Ok(_) => {
                            self.add_log(LogLevel::Success, &format!("Clicked element [{}]", index));
                            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                        }
                        Err(e) => self.add_log(LogLevel::Error, &format!("Click failed: {}", e)),
                    }
                } else {
                    self.add_log(LogLevel::Error, "Usage: click <index> (e.g., click 0 or click [0])");
                }
            }
            "input" => {
                let input_parts: Vec<&str> = args.splitn(2, char::is_whitespace).collect();
                if input_parts.len() < 2 {
                    self.add_log(LogLevel::Error, "Usage: input <index> <text>");
                    return;
                }
                let index_str = input_parts[0].trim_start_matches('[').trim_end_matches(']');
                if let Ok(index) = index_str.parse::<usize>() {
                    let text = input_parts[1].trim_matches('"').trim_matches('\'');
                    match self.browser.type_text(index, text).await {
                        Ok(_) => self.add_log(LogLevel::Success, &format!("Typed '{}' into [{}]", text, index)),
                        Err(e) => self.add_log(LogLevel::Error, &format!("Input failed: {}", e)),
                    }
                } else {
                    self.add_log(LogLevel::Error, "Usage: input <index> <text>");
                }
            }
            "scroll" => {
                let scroll_parts: Vec<&str> = args.split_whitespace().collect();
                let direction = scroll_parts.first().copied().unwrap_or("down");
                let amount: u32 = scroll_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(500);
                match self.browser.scroll(direction, amount).await {
                    Ok(_) => self.add_log(LogLevel::Success, &format!("Scrolled {} by {}px", direction, amount)),
                    Err(e) => self.add_log(LogLevel::Error, &format!("Scroll failed: {}", e)),
                }
            }
            "press" | "send_keys" => {
                let keys = args;
                if keys.is_empty() {
                    self.add_log(LogLevel::Error, "Usage: press <key> (e.g., press Enter, press Control+a)");
                    return;
                }
                match self.browser.send_keys(keys).await {
                    Ok(_) => self.add_log(LogLevel::Success, &format!("Sent keys: {}", keys)),
                    Err(e) => self.add_log(LogLevel::Error, &format!("Send keys failed: {}", e)),
                }
            }
            "screenshot" | "ss" => {
                match self.browser.screenshot().await {
                    Ok(_) => self.add_log(LogLevel::Success, "Screenshot captured"),
                    Err(e) => self.add_log(LogLevel::Error, &format!("Screenshot failed: {}", e)),
                }
            }
            "back" => {
                match self.browser.go_back().await {
                    Ok(_) => {
                        self.add_log(LogLevel::Success, "Navigated back");
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        let _ = self.refresh_dom().await;
                    }
                    Err(e) => self.add_log(LogLevel::Error, &format!("Go back failed: {}", e)),
                }
            }
            "forward" | "fwd" => {
                match self.browser.go_forward().await {
                    Ok(_) => {
                        self.add_log(LogLevel::Success, "Navigated forward");
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        let _ = self.refresh_dom().await;
                    }
                    Err(e) => self.add_log(LogLevel::Error, &format!("Go forward failed: {}", e)),
                }
            }
            "reload" | "refresh" => {
                match self.browser.reload().await {
                    Ok(_) => {
                        self.add_log(LogLevel::Success, "Page reloaded");
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        let _ = self.refresh_dom().await;
                    }
                    Err(e) => self.add_log(LogLevel::Error, &format!("Reload failed: {}", e)),
                }
            }
            "tab-new" => {
                let url_arg = if args.is_empty() { None } else { Some(args) };
                match self.browser.new_tab(url_arg).await {
                    Ok(_) => {
                        self.add_log(LogLevel::Success, "New tab opened");
                        let _ = self.refresh_status().await;
                    }
                    Err(e) => self.add_log(LogLevel::Error, &format!("New tab failed: {}", e)),
                }
            }
            "tab-switch" => {
                if let Ok(index) = args.parse::<usize>() {
                    match self.browser.switch_tab(index).await {
                        Ok(_) => {
                            self.current_tab = index;
                            self.add_log(LogLevel::Success, &format!("Switched to tab {}", index));
                            let _ = self.refresh_dom().await;
                            let _ = self.refresh_status().await;
                        }
                        Err(e) => self.add_log(LogLevel::Error, &format!("Tab switch failed: {}", e)),
                    }
                } else {
                    self.add_log(LogLevel::Error, "Usage: tab-switch <index>");
                }
            }
            "tab-close" => {
                if let Ok(index) = args.parse::<usize>() {
                    match self.browser.close_tab(index).await {
                        Ok(_) => {
                            self.add_log(LogLevel::Success, &format!("Tab {} closed", index));
                            let _ = self.refresh_status().await;
                        }
                        Err(e) => self.add_log(LogLevel::Error, &format!("Tab close failed: {}", e)),
                    }
                } else {
                    self.add_log(LogLevel::Error, "Usage: tab-close <index>");
                }
            }
            "eval" | "js" => {
                if args.is_empty() {
                    self.add_log(LogLevel::Error, "Usage: eval <javascript code>");
                    return;
                }
                match self.browser.evaluate_js(args).await {
                    Ok(result) => {
                        let val = result["result"]["value"].as_str().unwrap_or("undefined");
                        self.add_log(LogLevel::Success, &format!("Result: {}", val));
                    }
                    Err(e) => self.add_log(LogLevel::Error, &format!("Eval failed: {}", e)),
                }
            }
            "find" => {
                if args.is_empty() {
                    self.add_log(LogLevel::Error, "Usage: find <text>");
                    return;
                }
                let js = format!(
                    r#"(function() {{
                        var text = document.body.innerText;
                        var idx = text.indexOf(arguments[0]);
                        if (idx >= 0) {{
                            var context = text.substring(Math.max(0, idx - 50), Math.min(text.length, idx + 50));
                            return 'Found at position ' + idx + ': ...' + context + '...';
                        }}
                        return 'Not found';
                    }})()"#
                );
                match self.browser.evaluate_js(&js).await {
                    Ok(result) => {
                        let val = result["result"]["value"].as_str().unwrap_or("undefined");
                        self.add_log(LogLevel::Info, &format!("Search '{}': {}", args, val));
                    }
                    Err(e) => self.add_log(LogLevel::Error, &format!("Find failed: {}", e)),
                }
            }
            "dom" => {
                let _ = self.refresh_dom().await;
            }
            "url" => {
                self.add_log(LogLevel::Info, &format!("Current URL: {}", self.url));
            }
            "title" => {
                self.add_log(LogLevel::Info, &format!("Title: {}", self.title));
            }
            "clear" => {
                self.logs.clear();
                self.add_log(LogLevel::Info, "Logs cleared");
            }
            "help" | "?" => {
                self.show_help = !self.show_help;
            }
            "quit" | "exit" => {
                self.should_quit = true;
            }
            _ => {
                self.add_log(LogLevel::Error, &format!("Unknown command: '{}'. Type 'help' for available commands.", command));
            }
        }

        self.refresh_status().await;
    }

    fn add_log(&mut self, level: LogLevel, message: &str) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let timestamp = format!("{:02}:{:02}:{:02}", (now / 3600) % 24, (now / 60) % 60, now % 60);
        self.logs.push(LogEntry {
            timestamp,
            message: message.to_string(),
            level,
        });
        if self.logs.len() > 500 {
            self.logs.remove(0);
        }
    }

    fn handle_key(&mut self, key: event::KeyEvent) {
        if self.show_help {
            match key.code {
                KeyCode::Char('?') | KeyCode::Esc | KeyCode::Char('h') => self.show_help = false,
                _ => {}
            }
            return;
        }

        match key.code {
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    self.handle_ctrl_key(c);
                } else {
                    self.input.insert(self.cursor_position, c);
                    self.cursor_position += c.len_utf8();
                }
            }
            KeyCode::Backspace => {
                if self.cursor_position > 0 {
                    let prev_char_end = self.cursor_position;
                    let prev_char_start = self.input[..prev_char_end]
                        .char_indices()
                        .next_back()
                        .map_or(0, |(i, _)| i);
                    self.input.drain(prev_char_start..prev_char_end);
                    self.cursor_position = prev_char_start;
                }
            }
            KeyCode::Delete => {
                if self.cursor_position < self.input.len() {
                    let next_char_end = self.input[self.cursor_position..]
                        .char_indices()
                        .nth(1)
                        .map_or(self.input.len(), |(i, _)| self.cursor_position + i);
                    self.input.drain(self.cursor_position..next_char_end);
                }
            }
            KeyCode::Left => {
                if self.cursor_position > 0 {
                    self.cursor_position = self.input[..self.cursor_position]
                        .char_indices()
                        .next_back()
                        .map_or(0, |(i, _)| i);
                }
            }
            KeyCode::Right => {
                if self.cursor_position < self.input.len() {
                    self.cursor_position = self.input[self.cursor_position..]
                        .char_indices()
                        .nth(1)
                        .map_or(self.input.len(), |(i, _)| self.cursor_position + i);
                }
            }
            KeyCode::Home => self.cursor_position = 0,
            KeyCode::End => self.cursor_position = self.input.len(),
            KeyCode::Enter => {
                let cmd = self.input.clone();
                if !cmd.trim().is_empty() {
                    self.input.clear();
                    self.cursor_position = 0;
                    self.pending_command = Some(cmd);
                }
            }
            KeyCode::Esc => {
                self.input.clear();
                self.cursor_position = 0;
            }
            KeyCode::F(5) => {
                self.pending_refresh = true;
            }
            _ => {}
        }
    }

    fn handle_ctrl_key(&mut self, c: char) {
        match c {
            'c' => self.should_quit = true,
            'l' => {
                self.input.clear();
                self.cursor_position = 0;
            }
            'r' => self.pending_reload = true,
            'd' => self.pending_screenshot = true,
            'n' => self.pending_new_tab = true,
            'w' => self.pending_close_tab = true,
            _ => {}
        }
    }
}

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(3),
        ])
        .split(f.area());

    let title = Line::from(vec![
        Span::styled(" FairyAction Tester ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" "),
        Span::styled(truncate_str(&app.url, 50), Style::default().fg(Color::DarkGray)),
        Span::raw(" | "),
        Span::styled(truncate_str(&app.title, 40), Style::default().fg(Color::Yellow)),
    ]);
    f.render_widget(
        Paragraph::new(title).style(Style::default().bg(Color::DarkGray)),
        chunks[0],
    );

    let middle = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(65),
            Constraint::Percentage(35),
        ])
        .split(chunks[1]);

    let dom_block = Block::default()
        .title(" DOM Tree (F5 to refresh) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));
    let dom_paragraph = Paragraph::new(app.dom_content.clone())
        .block(dom_block)
        .wrap(Wrap { trim: false })
        .scroll((app.dom_scroll as u16, 0));
    f.render_widget(dom_paragraph, middle[0]);

    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(6),
            Constraint::Min(0),
        ])
        .split(middle[1]);

    let status_block = Block::default()
        .title(" Status ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Green));
    let status_text = format!(
        "URL:    {}\nTitle:  {}\nTab:    {}",
        truncate_str(&app.url, 60),
        truncate_str(&app.title, 50),
        app.current_tab,
    );
    f.render_widget(
        Paragraph::new(status_text).block(status_block),
        right_chunks[0],
    );

    let log_block = Block::default()
        .title(" Log ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let log_items: Vec<ListItem> = app
        .logs
        .iter()
        .rev()
        .take(100)
        .rev()
        .map(|entry| {
            let style = match entry.level {
                LogLevel::Success => Style::default().fg(Color::Green),
                LogLevel::Error => Style::default().fg(Color::Red),
                LogLevel::Command => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                LogLevel::Info => Style::default().fg(Color::Gray),
            };
            let line = Line::from(vec![
                Span::styled(format!(" {} ", entry.timestamp), Style::default().fg(Color::DarkGray)),
                Span::styled(entry.message.clone(), style),
            ]);
            ListItem::new(line)
        })
        .collect();
    let log_list = List::new(log_items).block(log_block);
    f.render_widget(log_list, right_chunks[1]);

    let input_block = Block::default()
        .title(" Command (type 'help' for commands) ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Magenta));
    let input = Paragraph::new(app.input.clone())
        .block(input_block)
        .style(Style::default().fg(Color::White));
    f.render_widget(input, chunks[2]);
    f.set_cursor_position(
        (
            chunks[2].x + unicode_width(&app.input[..app.cursor_position]) as u16 + 1,
            chunks[2].y + 1,
        ),
    );

    if app.show_help {
        let help_lines = vec![
            Line::from(Span::styled("  FairyAction Tester - Help", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))),
            Line::from(""),
            Line::from(Span::styled("  Commands:", Style::default().add_modifier(Modifier::BOLD))),
            Line::from("    navigate <url>        Navigate to URL"),
            Line::from("    click <index>         Click element by index"),
            Line::from("    input <index> <text>  Type text into element"),
            Line::from("    scroll <dir> [amount] Scroll page (up/down)"),
            Line::from("    press <key>           Send key (Enter, Escape, Tab)"),
            Line::from("    screenshot            Take screenshot"),
            Line::from("    back / forward        Browser navigation"),
            Line::from("    reload                Reload page"),
            Line::from("    tab-new / tab-switch / tab-close  Tab management"),
            Line::from("    eval <js>             Execute JavaScript"),
            Line::from("    find <text>           Search in page"),
            Line::from("    dom                   Refresh DOM tree"),
            Line::from("    url / title           Show current info"),
            Line::from("    clear                 Clear logs"),
            Line::from("    quit                  Exit tester"),
            Line::from(""),
            Line::from(Span::styled("  Shortcuts:", Style::default().add_modifier(Modifier::BOLD))),
            Line::from("    Ctrl+C  Quit    Ctrl+R  Reload    Ctrl+D  Screenshot"),
            Line::from("    Ctrl+N  New tab Ctrl+W  Close tab F5  Refresh DOM"),
            Line::from("    Esc     Clear input   ?/h  Toggle help"),
        ];
        let help_block = Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().bg(Color::Black));
        let help_paragraph = Paragraph::new(help_lines).block(help_block);
        let area = centered_rect(70, 80, f.area());
        f.render_widget(help_paragraph, area);
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_width = r.width * percent_x / 100;
    let popup_height = r.height * percent_y / 100;
    let x = (r.width.saturating_sub(popup_width)) / 2;
    let y = (r.height.saturating_sub(popup_height)) / 2;
    Rect::new(x, y, popup_width, popup_height)
}

fn truncate_str(s: &str, max_len: usize) -> &str {
    if s.len() <= max_len {
        return s;
    }
    let mut end = max_len;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    &s[..end]
}

fn unicode_width(s: &str) -> usize {
    use unicode_width::UnicodeWidthStr;
    s.width()
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_target(false)
        .init();

    enable_raw_mode()?;
    execute!(std::io::stdout(), EnterAlternateScreen)?;
    let terminal = ratatui::Terminal::new(
        ratatui::backend::CrosstermBackend::new(std::io::stdout()),
    )?;

    let mut app = App::new().await?;

    let result = run_app(terminal, &mut app).await;

    disable_raw_mode()?;
    execute!(std::io::stdout(), LeaveAlternateScreen)?;

    if let Err(e) = result {
        eprintln!("Error: {}", e);
    }

    Ok(())
}

async fn run_app(
    mut terminal: ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>,
    app: &mut App,
) -> anyhow::Result<()> {
    loop {
        terminal.draw(|f| ui(f, app))?;

        if event::poll(std::time::Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key);
                }
            }
        }

        if app.should_quit {
            break;
        }

        if let Some(cmd) = app.pending_command.take() {
            app.execute_command(&cmd).await;
        }
        if app.pending_refresh {
            app.pending_refresh = false;
            let _ = app.refresh_dom().await;
        }
        if app.pending_reload {
            app.pending_reload = false;
            match app.browser.reload().await {
                Ok(_) => app.add_log(LogLevel::Success, "Page reloaded"),
                Err(e) => app.add_log(LogLevel::Error, &format!("Reload failed: {}", e)),
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let _ = app.refresh_dom().await;
        }
        if app.pending_screenshot {
            app.pending_screenshot = false;
            match app.browser.screenshot().await {
                Ok(_) => app.add_log(LogLevel::Success, "Screenshot captured"),
                Err(e) => app.add_log(LogLevel::Error, &format!("Screenshot failed: {}", e)),
            }
        }
        if app.pending_new_tab {
            app.pending_new_tab = false;
            match app.browser.new_tab(None).await {
                Ok(_) => {
                    app.add_log(LogLevel::Success, "New tab opened");
                    let _ = app.refresh_status().await;
                }
                Err(e) => app.add_log(LogLevel::Error, &format!("New tab failed: {}", e)),
            }
        }
        if app.pending_close_tab {
            app.pending_close_tab = false;
            match app.browser.close_tab(0).await {
                Ok(_) => app.add_log(LogLevel::Success, "Tab closed"),
                Err(e) => app.add_log(LogLevel::Error, &format!("Close tab failed: {}", e)),
            }
            let _ = app.refresh_status().await;
        }
    }

    Ok(())
}
