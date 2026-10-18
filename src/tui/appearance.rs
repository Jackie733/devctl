use ratatui::{
    buffer::Buffer,
    style::{Color, Modifier},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    Dark,
    Light,
    Terminal,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Appearance {
    pub theme: Theme,
    pub reduced_motion: bool,
    pub ascii: bool,
    pub no_color: bool,
}

impl Appearance {
    pub fn from_env() -> Result<Self, String> {
        Self::parse(|name| std::env::var_os(name).map(|value| value.to_string_lossy().into_owned()))
    }

    fn parse(get: impl Fn(&str) -> Option<String>) -> Result<Self, String> {
        let theme = match get("DEVCTL_TUI_THEME").as_deref() {
            None | Some("dark") => Theme::Dark,
            Some("light") => Theme::Light,
            Some("terminal") => Theme::Terminal,
            _ => return Err("DEVCTL_TUI_THEME must be dark, light or terminal.".into()),
        };
        let flag = |name| match get(name).as_deref() {
            None | Some("0") => Ok(false),
            Some("1") => Ok(true),
            _ => Err(format!("{name} must be 0 or 1.")),
        };
        Ok(Self {
            theme,
            reduced_motion: flag("DEVCTL_TUI_REDUCED_MOTION")?,
            ascii: flag("DEVCTL_TUI_ASCII")?,
            no_color: get("NO_COLOR").is_some_and(|value| !value.is_empty()),
        })
    }

    // The view uses a small named-color vocabulary. Resolve it once per completed
    // frame, including overlays, so every surface follows the same palette. Log
    // text is never parsed as terminal styling or modified by this presentation pass.
    pub fn apply(self, buffer: &mut Buffer) {
        for cell in &mut buffer.content {
            if self.no_color
                && matches!(cell.bg, Color::DarkGray | Color::Blue | Color::LightYellow)
            {
                cell.modifier.insert(Modifier::UNDERLINED);
            }
            cell.fg = self.foreground(cell.fg);
            cell.bg = self.background(cell.bg);
            if self.no_color {
                cell.underline_color = Color::Reset;
            }
        }
    }

    pub fn foreground(self, color: Color) -> Color {
        if self.no_color {
            return Color::Reset;
        }
        if self.theme == Theme::Terminal {
            return match color {
                Color::White | Color::Gray => Color::Reset,
                other => other,
            };
        }
        let light = self.theme == Theme::Light;
        match color {
            Color::Reset | Color::White | Color::Gray => {
                if light {
                    rgb(0x0f172a)
                } else {
                    rgb(0xe5e7eb)
                }
            }
            Color::DarkGray => {
                if light {
                    rgb(0x475569)
                } else {
                    rgb(0x9ca3af)
                }
            }
            Color::Cyan => {
                if light {
                    rgb(0x0e7490)
                } else {
                    rgb(0x67e8f9)
                }
            }
            Color::Green => {
                if light {
                    rgb(0x166534)
                } else {
                    rgb(0x86efac)
                }
            }
            Color::Yellow => {
                if light {
                    rgb(0x92400e)
                } else {
                    rgb(0xfcd34d)
                }
            }
            Color::LightRed => {
                if light {
                    rgb(0x9f1239)
                } else {
                    rgb(0xfda4af)
                }
            }
            Color::Magenta => {
                if light {
                    rgb(0x6b21a8)
                } else {
                    rgb(0xd8b4fe)
                }
            }
            Color::Black => rgb(0x111827),
            other => other,
        }
    }

    pub fn background(self, color: Color) -> Color {
        if self.no_color {
            return Color::Reset;
        }
        if self.theme == Theme::Terminal {
            return if color == Color::Black {
                Color::Reset
            } else {
                color
            };
        }
        let light = self.theme == Theme::Light;
        match color {
            Color::Reset => {
                if light {
                    rgb(0xf8fafc)
                } else {
                    rgb(0x111827)
                }
            }
            Color::Black => {
                if light {
                    rgb(0xffffff)
                } else {
                    rgb(0x1f2937)
                }
            }
            Color::DarkGray => {
                if light {
                    rgb(0xe2e8f0)
                } else {
                    rgb(0x374151)
                }
            }
            Color::Blue => {
                if light {
                    rgb(0xcbd5e1)
                } else {
                    rgb(0x334155)
                }
            }
            Color::Yellow => {
                if light {
                    rgb(0xfef3c7)
                } else {
                    rgb(0xfcd34d)
                }
            }
            Color::LightYellow => rgb(0xfde68a),
            other => other,
        }
    }
}

const fn rgb(value: u32) -> Color {
    Color::Rgb((value >> 16) as u8, (value >> 8) as u8, value as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appearance_options_validate_without_mutating_process_environment() {
        let appearance = Appearance::parse(|name| match name {
            "DEVCTL_TUI_THEME" => Some("light".into()),
            "DEVCTL_TUI_ASCII" | "DEVCTL_TUI_REDUCED_MOTION" => Some("1".into()),
            "NO_COLOR" => Some("1".into()),
            _ => None,
        })
        .unwrap();
        assert_eq!(appearance.theme, Theme::Light);
        assert!(appearance.ascii && appearance.reduced_motion && appearance.no_color);
        for (name, value) in [
            ("DEVCTL_TUI_THEME", "unknown"),
            ("DEVCTL_TUI_ASCII", "yes"),
            ("DEVCTL_TUI_REDUCED_MOTION", "yes"),
        ] {
            assert!(Appearance::parse(|key| (key == name).then(|| value.into())).is_err());
        }
        assert!(
            !Appearance::parse(|key| (key == "NO_COLOR").then(String::new))
                .unwrap()
                .no_color
        );
    }
}
