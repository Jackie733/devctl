use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::{Position, Rect};

use super::{
    Action,
    app::{App, Focus, LogSelection},
    view,
};

pub(super) fn handle(
    app: &mut App,
    mouse: MouseEvent,
    area: Rect,
    viewport: Option<&view::LogViewport>,
) -> Action {
    if !view::usable(area)
        || app.help
        || app.confirm_shutdown
        || app.confirm_remove.is_some()
        || app.add.is_some()
        || app.search_input.is_some()
    {
        app.logs.drag = None;
        return Action::None;
    }
    let in_projects = view::projects_area(area, app)
        .is_some_and(|projects| projects.contains(Position::new(mouse.column, mouse.row)));
    if in_projects && app.logs.drag.is_none() {
        if !mouse.modifiers.is_empty() {
            return Action::None;
        }
        return match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let focused = app.focus != Focus::Projects;
                app.focus(Focus::Projects);
                if let Some(id) = view::project_at(area, app, mouse.column, mouse.row)
                    && app.select_id(&id)
                {
                    Action::Selection
                } else if focused {
                    Action::Redraw
                } else {
                    Action::None
                }
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let focused = app.focus != Focus::Projects;
                app.focus(Focus::Projects);
                let delta = if mouse.kind == MouseEventKind::ScrollUp {
                    -1
                } else {
                    1
                };
                if app.select(delta) {
                    Action::Selection
                } else if focused {
                    Action::Redraw
                } else {
                    Action::None
                }
            }
            _ => Action::None,
        };
    }
    let Some(viewport) = viewport.filter(|viewport| viewport.matches(area, app)) else {
        app.logs.drag = None;
        return Action::None;
    };
    let content = viewport.content;
    let inside = content.contains(Position::new(mouse.column, mouse.row));
    if matches!(mouse.kind, MouseEventKind::Up(MouseButton::Left)) {
        if let Some(anchor) = app.logs.drag.take()
            && let Some(head) = viewport.position(app, mouse.column, mouse.row)
            && (app.logs.selection.is_some() || head != anchor)
        {
            app.logs.selection = Some(LogSelection {
                anchor: anchor.sequence,
                head: head.sequence,
                text: Some((anchor.cell, head.cell)),
            });
            return Action::Redraw;
        }
        return Action::None;
    }
    if !mouse.modifiers.is_empty() && mouse.modifiers != KeyModifiers::SHIFT {
        return Action::None;
    }
    match mouse.kind {
        MouseEventKind::ScrollUp | MouseEventKind::ScrollDown if inside => {
            app.focus(Focus::Logs);
            if !app.logs.paused {
                app.logs.top = viewport
                    .top
                    .filter(|top| app.logs.records.iter().any(|r| r.sequence == *top));
            }
            app.logs.scroll(
                if mouse.kind == MouseEventKind::ScrollUp {
                    -3
                } else {
                    3
                },
                usize::from(content.height),
            );
            Action::Redraw
        }
        MouseEventKind::Down(MouseButton::Right) if inside && app.logs.selection.is_some() => {
            Action::Copy
        }
        MouseEventKind::Down(MouseButton::Left) if inside => {
            let Some(position) = viewport.position(app, mouse.column, mouse.row) else {
                app.logs.drag = None;
                return Action::None;
            };
            app.focus(Focus::Logs);
            app.goto_top = false;
            app.logs.cancel_search();
            app.logs.selection = None;
            app.logs.selection_lost = false;
            app.logs.top = viewport
                .top
                .filter(|top| app.logs.records.iter().any(|r| r.sequence == *top));
            app.logs.horizontal = viewport.horizontal;
            app.logs.reading = viewport.reading;
            app.logs.pause(usize::from(content.height));
            app.logs.drag = Some(position);
            app.selection_epoch += 1;
            Action::Selection
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            let Some(anchor) = app.logs.drag else {
                return Action::None;
            };
            let Some(head) = viewport.position(app, mouse.column, mouse.row) else {
                return Action::None;
            };
            app.logs.selection = Some(LogSelection {
                anchor: anchor.sequence,
                head: head.sequence,
                text: Some((anchor.cell, head.cell)),
            });
            Action::Redraw
        }
        MouseEventKind::Down(MouseButton::Left) => {
            app.logs.drag = None;
            Action::None
        }
        _ => Action::None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        logs::{Batch, Record, Source},
        tui::app::LogQuery,
    };

    fn handle(app: &mut App, mouse: MouseEvent, area: Rect) -> Action {
        let viewport = view::LogViewport::capture(area, app);
        super::handle(app, mouse, area, viewport.as_ref())
    }

    fn app(texts: &[&str]) -> App {
        let mut app = App::new();
        app.focus(Focus::Logs);
        app.logs.append(Batch {
            records: texts
                .iter()
                .enumerate()
                .map(|(i, text)| Record {
                    sequence: i as u64 + 1,
                    captured_ms: 0,
                    instance: 1,
                    source: Source::Stdout,
                    text: (*text).into(),
                    truncated: false,
                })
                .collect(),
            oldest: Some(1),
            latest: texts.len() as u64,
            ..Batch::default()
        });
        app
    }

    fn event(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
        MouseEvent {
            kind,
            column,
            row,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn coordinates(app: &App, area: Rect, sequence: u64, byte: usize) -> (u16, u16) {
        let content = view::log_content(area, app).unwrap();
        let index = app
            .logs
            .records
            .iter()
            .position(|r| r.sequence == sequence)
            .unwrap();
        let row = content.y + (index - app.logs.start(usize::from(content.height))) as u16;
        assert!(row < content.bottom());
        let viewport = view::LogViewport::capture(area, app).unwrap();
        for column in content.x..content.right() {
            if viewport.position(app, column, row).is_some_and(|p| {
                p.sequence == sequence && p.cell.start == byte && p.cell.end > byte
            }) {
                return (column, row);
            }
        }
        panic!("Missing visible text at {sequence}:{byte}");
    }

    fn drag(app: &mut App, area: Rect, from: (u64, usize), to: (u64, usize)) {
        let (x, y) = coordinates(app, area, from.0, from.1);
        assert!(matches!(
            handle(
                app,
                event(MouseEventKind::Down(MouseButton::Left), x, y),
                area
            ),
            Action::Selection
        ));
        let (x, y) = coordinates(app, area, to.0, to.1);
        handle(
            app,
            event(MouseEventKind::Drag(MouseButton::Left), x, y),
            area,
        );
        handle(
            app,
            event(MouseEventKind::Up(MouseButton::Left), x, y),
            area,
        );
        assert!(app.logs.drag.is_none());
    }

    #[test]
    fn text_drag_copies_graphemes_and_raw_tabs_in_either_direction_and_across_rows() {
        let text = "prefix 中文🙂e\u{301}\tend suffix";
        for area in [
            Rect::new(0, 0, 120, 30),
            Rect::new(0, 0, 60, 16),
            Rect::new(0, 0, 200, 30),
        ] {
            let mut app = app(&[text, "second body"]);
            app.logs.records[0].captured_ms = 1_000;
            let first = text.find('中').unwrap();
            let last = text.find("end").unwrap() + 2;
            drag(&mut app, area, (1, first), (1, last));
            assert_eq!(
                app.logs.selected_text().unwrap(),
                ("中文🙂e\u{301}\tend".into(), 1)
            );
            let before = app.selection_epoch;
            drag(&mut app, area, (1, last), (1, first));
            assert_eq!(app.logs.selected_text().unwrap().0, "中文🙂e\u{301}\tend");
            assert!(
                app.selection_epoch > before,
                "A fresh drag invalidates old clipboard completions"
            );
            drag(&mut app, area, (2, 5), (1, first));
            assert_eq!(
                app.logs.selected_text().unwrap().0,
                format!("{}\nsecond", &text[first..])
            );
            let (x, y) = coordinates(&app, area, 1, first);
            assert!(matches!(
                handle(
                    &mut app,
                    event(MouseEventKind::Down(MouseButton::Right), x, y),
                    area
                ),
                Action::Copy
            ));
        }
    }

    #[test]
    fn wheel_loads_history_and_keeps_selection_until_explicit_follow() {
        let texts: Vec<_> = (0..200).map(|_| "body").collect();
        let mut app = app(&texts);
        let area = Rect::new(0, 0, 120, 30);
        drag(&mut app, area, (199, 1), (200, 2));
        let selection = app.logs.selection;
        let selected = app.logs.selected_text().unwrap();
        let content = view::log_content(area, &app).unwrap();
        for _ in 0..100 {
            handle(
                &mut app,
                event(MouseEventKind::ScrollUp, content.x, content.y),
                area,
            );
        }
        assert_eq!(app.logs.top, Some(1));
        for _ in 0..100 {
            handle(
                &mut app,
                event(MouseEventKind::ScrollDown, content.x, content.y),
                area,
            );
        }
        assert_eq!(app.logs.selection, selection);
        assert_eq!(app.logs.selected_text().unwrap(), selected);
        assert!(
            app.logs.paused,
            "Wheel scrolling must not discard a selection at the tail"
        );
        app.logs.selection = None;
        handle(
            &mut app,
            event(MouseEventKind::ScrollDown, content.x, content.y),
            area,
        );
        assert!(!app.logs.paused);

        // The retained beginning lies outside the currently loaded client page.
        app.logs = Default::default();
        let mut batch = Batch {
            oldest: Some(1),
            latest: 200,
            ..Batch::default()
        };
        batch.records = (101..=200)
            .map(|sequence| Record {
                sequence,
                captured_ms: 0,
                instance: 1,
                source: Source::Stdout,
                text: sequence.to_string(),
                truncated: false,
            })
            .collect();
        app.logs.append(batch);
        for _ in 0..100 {
            handle(
                &mut app,
                event(MouseEventKind::ScrollUp, content.x, content.y),
                area,
            );
        }
        assert_eq!(app.logs.requested, Some(LogQuery::Before(101)));
        let mut page = Batch {
            oldest: Some(1),
            latest: 200,
            ..Batch::default()
        };
        page.records = (1..=100)
            .map(|sequence| Record {
                sequence,
                captured_ms: 0,
                instance: 1,
                source: Source::Stdout,
                text: sequence.to_string(),
                truncated: false,
            })
            .collect();
        app.logs.apply_page(LogQuery::Before(101), page);
        handle(
            &mut app,
            event(MouseEventKind::ScrollUp, content.x, content.y),
            area,
        );
        assert_eq!(app.logs.top, Some(98));
    }

    #[test]
    fn overlays_hidden_panes_and_non_log_regions_do_not_receive_mouse_actions() {
        let mut app = app(&["body"]);
        let area = Rect::new(0, 0, 120, 30);
        let content = view::log_content(area, &app).unwrap();
        let input = event(
            MouseEventKind::Down(MouseButton::Left),
            content.x + 12,
            content.y,
        );
        for modal in 0..5 {
            app.help = modal == 0;
            app.confirm_shutdown = modal == 1;
            app.confirm_remove = (modal == 2).then(|| "web".into());
            app.search_input = (modal == 3).then(String::new);
            app.add = (modal == 4).then(super::super::onboarding::Dialog::new);
            assert!(matches!(handle(&mut app, input, area), Action::None));
            assert!(app.logs.drag.is_none() && app.logs.selection.is_none());
        }
        app.add = None;
        assert!(matches!(
            handle(&mut app, event(MouseEventKind::ScrollUp, 1, 0), area),
            Action::None
        ));
        app.focus(Focus::Projects);
        assert!(matches!(
            handle(&mut app, input, Rect::new(0, 0, 60, 16)),
            Action::None
        ));
        assert!(!app.logs.paused);
        assert!(app.pending.is_none());
    }

    #[test]
    fn projects_panel_click_and_wheel_select_and_focus_projects() {
        use crate::{
            control::Response,
            manager::{ServiceState, ServiceStatus},
        };

        let mut app = app(&["body"]);
        app.apply_status(Response {
            workspace_id: "ws".into(),
            services: [
                (
                    "api".into(),
                    ServiceStatus {
                        state: ServiceState::Running,
                        ..ServiceStatus::default()
                    },
                ),
                (
                    "web".into(),
                    ServiceStatus {
                        state: ServiceState::Stopped,
                        cleanup_confirmed: true,
                        ..ServiceStatus::default()
                    },
                ),
            ]
            .into(),
            ..Response::default()
        });
        app.selected = Some("api".into());
        app.focus(Focus::Logs);
        let area = Rect::new(0, 0, 120, 30);
        let row = view::projects_area(area, &app).unwrap().y + 2;

        assert!(matches!(
            handle(
                &mut app,
                event(MouseEventKind::Down(MouseButton::Left), 2, row),
                area
            ),
            Action::Selection
        ));
        assert_eq!(app.focus, Focus::Projects);
        assert_eq!(app.selected.as_deref(), Some("web"));

        assert!(matches!(
            handle(
                &mut app,
                event(MouseEventKind::Down(MouseButton::Left), 2, row),
                area
            ),
            Action::None
        ));
        app.focus(Focus::Logs);
        assert!(matches!(
            handle(
                &mut app,
                event(MouseEventKind::Down(MouseButton::Left), 2, row),
                area
            ),
            Action::Redraw
        ));
        assert_eq!(app.focus, Focus::Projects);
        assert_eq!(app.selected.as_deref(), Some("web"));

        app.focus(Focus::Logs);
        assert!(matches!(
            handle(&mut app, event(MouseEventKind::ScrollUp, 2, row), area),
            Action::Selection
        ));
        assert_eq!(app.focus, Focus::Projects);
        assert_eq!(app.selected.as_deref(), Some("api"));

        assert!(matches!(
            handle(&mut app, event(MouseEventKind::ScrollDown, 2, row), area),
            Action::Selection
        ));
        assert_eq!(app.selected.as_deref(), Some("web"));
        assert!(app.pending.is_none());
    }

    #[test]
    fn text_drag_uses_horizontal_offset_and_eviction_cancels_it() {
        let mut app = app(&["0123456789中文🙂tail"]);
        let area = Rect::new(0, 0, 60, 16);
        app.logs.horizontal = 14;
        drag(&mut app, area, (1, 10), (1, 16));
        assert_eq!(app.logs.selected_text().unwrap().0, "中文🙂");
        let (x, y) = coordinates(&app, area, 1, 10);
        handle(
            &mut app,
            event(MouseEventKind::Down(MouseButton::Left), x, y),
            area,
        );
        let batch = Batch {
            records: (2..=4200)
                .map(|sequence| Record {
                    sequence,
                    captured_ms: 0,
                    instance: 1,
                    source: Source::Stdout,
                    text: "later".into(),
                    truncated: false,
                })
                .collect(),
            ..Batch::default()
        };
        app.logs.append(batch);
        assert!(app.logs.drag.is_none());
        handle(
            &mut app,
            event(MouseEventKind::Drag(MouseButton::Left), x, y),
            area,
        );
        assert!(app.logs.selection.is_none());
    }

    #[test]
    fn clicks_use_the_drawn_rows_when_output_arrives_before_the_next_redraw() {
        let mut app = app(&["body"; 60]);
        let area = Rect::new(0, 0, 120, 30);
        let viewport = view::LogViewport::capture(area, &app).unwrap();
        let sequence = viewport.top.unwrap();
        let (x, y) = coordinates(&app, area, sequence, 0);
        app.logs.append(Batch {
            records: (61..=100)
                .map(|sequence| Record {
                    sequence,
                    captured_ms: 0,
                    instance: 1,
                    source: Source::Stdout,
                    text: "later".into(),
                    truncated: false,
                })
                .collect(),
            latest: 100,
            oldest: Some(1),
            ..Batch::default()
        });
        super::handle(
            &mut app,
            event(MouseEventKind::Down(MouseButton::Left), x, y),
            area,
            Some(&viewport),
        );
        assert_eq!(app.logs.drag.unwrap().sequence, sequence);
        assert_eq!(app.logs.top, Some(sequence));
        super::handle(
            &mut app,
            event(MouseEventKind::Drag(MouseButton::Left), x + 1, y),
            area,
            Some(&viewport),
        );
        assert_eq!(app.logs.selected_text().unwrap().0, "bo");
        app.selected = Some("another-service".into());
        assert!(matches!(
            super::handle(
                &mut app,
                event(MouseEventKind::Drag(MouseButton::Left), x + 2, y),
                area,
                Some(&viewport)
            ),
            Action::None
        ));
        assert!(app.logs.drag.is_none());
    }
}
