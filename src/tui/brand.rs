use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::Line,
    widgets::Paragraph,
};

pub(super) const WIDTH: u16 = 7;
pub(super) const HEIGHT: u16 = 3;

// Both cell treatments preserve all four strokes, including the longer middle
// service line and the separated, continuous control rail. Never use a one-row
// abbreviation or clip part of the mark to fit a terminal.
pub(super) fn rows(ascii: bool) -> [&'static str; 3] {
    if ascii {
        ["---   |", "----- |", "---   |"]
    } else {
        ["━━━   ┃", "━━━━━ ┃", "━━━   ┃"]
    }
}

pub(super) fn render(frame: &mut Frame, area: Rect, ascii: bool) {
    if area.width < WIDTH || area.height < HEIGHT {
        return;
    }
    frame.render_widget(
        Paragraph::new(rows(ascii).map(Line::from).to_vec())
            .style(Style::default().fg(Color::Cyan)),
        Rect::new(area.x, area.y, WIDTH, HEIGHT),
    );
}
