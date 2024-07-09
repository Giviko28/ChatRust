use tui::{
    backend::Backend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    text::{Span, Spans},
    widgets::{Block, Borders, Paragraph},
};

use std::collections::VecDeque;

pub struct ChatUI<B: Backend> {
    terminal: tui::Terminal<B>,
    messages: VecDeque<Span<'static>>,
    errors: VecDeque<Span<'static>>,
    users: VecDeque<Span<'static>>,
}

impl<B: Backend> ChatUI<B> {
    pub fn new(terminal: tui::Terminal<B>) -> Self {
        Self {
            terminal,
            messages: VecDeque::new(),
            errors: VecDeque::new(),
            users: VecDeque::new(),
        }
    }

    pub fn push_message(&mut self, message: Span<'static>) {
        self.messages.push_back(message);
        self.draw();
    }

    pub fn push_error(&mut self, error: Span<'static>) {
        let mut error_copy = error.clone();
        error_copy.style = Style::default().fg(Color::Red);
        self.errors.push_back(error_copy);
        self.draw();
    }

    pub fn add_user(&mut self, user: Span<'static>) {
        self.users.push_back(user);
        self.draw();
    }

    pub fn delete_user(&mut self, username: &str) {
        self.users.retain(|user| user.content != username);
        self.draw();
    }

    pub fn draw(&mut self) {
        match self.terminal.clear() {
            Ok(_) => {}
            Err(err) => {
                let error_message = Span::styled(
                    format!("Error clearing terminal: {}", err),
                    Style::default().fg(Color::Red),
                );
                self.push_error(error_message);
            }
        }
        let mut f = self.terminal.get_frame();
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(
                [
                    Constraint::Percentage(10),
                    Constraint::Percentage(80),
                    Constraint::Percentage(10),
                ]
                .as_ref(),
            )
            .split(f.size());

        let outer_block = Block::default()
            .title("[X5-Chat Server]")
            .borders(Borders::ALL);
        f.render_widget(outer_block, chunks[1]);

        let inner_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .margin(1)
            .constraints(
                [
                    Constraint::Percentage(33),
                    Constraint::Percentage(34),
                    Constraint::Percentage(33),
                ]
                .as_ref(),
            )
            .split(chunks[1]);

        let normal_block = Block::default().title("Message logs").borders(Borders::ALL);
        f.render_widget(normal_block.clone(), inner_chunks[0]);

        let users_block = Block::default()
            .title("Current users")
            .borders(Borders::ALL);
        f.render_widget(users_block.clone(), inner_chunks[1]);

        let error_block = Block::default().title("Error logs").borders(Borders::ALL);
        f.render_widget(error_block.clone(), inner_chunks[2]);

        // Render messages inside the boxes
        let normal_paragraph = Paragraph::new(
            self.messages
                .iter()
                .map(|m| Spans::from(vec![m.clone()]))
                .collect::<Vec<_>>(),
        )
        .style(Style::default().fg(Color::White))
        .block(normal_block);

        f.render_widget(normal_paragraph, inner_chunks[0]);

        let users_paragraph = Paragraph::new(
            self.users
                .iter()
                .map(|m| Spans::from(vec![m.clone()]))
                .collect::<Vec<_>>(),
        )
        .style(Style::default().fg(Color::White))
        .block(users_block);
        f.render_widget(users_paragraph, inner_chunks[1]);

        let error_paragraph = Paragraph::new(
            self.errors
                .iter()
                .map(|m| Spans::from(vec![m.clone()]))
                .collect::<Vec<_>>(),
        )
        .style(Style::default().fg(Color::Red))
        .block(error_block);

        f.render_widget(error_paragraph, inner_chunks[2]);

        match self.terminal.flush() {
            Ok(_) => {}
            Err(err) => {
                let error_message = Span::styled(
                    format!("Error flushing terminal: {}", err),
                    Style::default().fg(Color::Red),
                );
                self.push_error(error_message);
            }
        }
    }
}

/*pub fn show_ui() -> Result<(), Box<dyn std::error::Error>> {
    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    terminal.hide_cursor()?;

    //terminal.clear()?; // Clear the terminal before getting the frame
    let mut chat_ui = ChatUI::new(terminal);

    chat_ui.draw(); // Call the ui function

    //chat_ui.terminal.flush()?;

    Ok(())
}
*/
