use ratatui::{
    buffer::Buffer,
    style::{Color, Modifier, Style},
};

fn ansi_fg(color: Color) -> Option<String> {
    match color {
        Color::Reset => None,
        Color::Black => Some("30".into()),
        Color::Red => Some("31".into()),
        Color::Green => Some("32".into()),
        Color::Yellow => Some("33".into()),
        Color::Blue => Some("34".into()),
        Color::Magenta => Some("35".into()),
        Color::Cyan => Some("36".into()),
        Color::Gray => Some("37".into()),
        Color::DarkGray => Some("90".into()),
        Color::LightRed => Some("91".into()),
        Color::LightGreen => Some("92".into()),
        Color::LightYellow => Some("93".into()),
        Color::LightBlue => Some("94".into()),
        Color::LightMagenta => Some("95".into()),
        Color::LightCyan => Some("96".into()),
        Color::White => Some("97".into()),
        Color::Rgb(r, g, b) => Some(format!("38;2;{r};{g};{b}")),
        Color::Indexed(i) => Some(format!("38;5;{i}")),
    }
}

fn ansi_bg(color: Color) -> Option<String> {
    match color {
        Color::Reset => None,
        Color::Black => Some("40".into()),
        Color::Red => Some("41".into()),
        Color::Green => Some("42".into()),
        Color::Yellow => Some("43".into()),
        Color::Blue => Some("44".into()),
        Color::Magenta => Some("45".into()),
        Color::Cyan => Some("46".into()),
        Color::Gray => Some("47".into()),
        Color::DarkGray => Some("100".into()),
        Color::LightRed => Some("101".into()),
        Color::LightGreen => Some("102".into()),
        Color::LightYellow => Some("103".into()),
        Color::LightBlue => Some("104".into()),
        Color::LightMagenta => Some("105".into()),
        Color::LightCyan => Some("106".into()),
        Color::White => Some("107".into()),
        Color::Rgb(r, g, b) => Some(format!("48;2;{r};{g};{b}")),
        Color::Indexed(i) => Some(format!("48;5;{i}")),
    }
}

fn style_to_sgr(style: Style) -> String {
    let mut codes = vec!["0".to_string()];
    if let Some(fg) = style.fg.and_then(ansi_fg) {
        codes.push(fg);
    }
    if let Some(bg) = style.bg.and_then(ansi_bg) {
        codes.push(bg);
    }
    if style.add_modifier.contains(Modifier::BOLD) {
        codes.push("1".into());
    }
    if style.add_modifier.contains(Modifier::DIM) {
        codes.push("2".into());
    }
    if style.add_modifier.contains(Modifier::ITALIC) {
        codes.push("3".into());
    }
    if style.add_modifier.contains(Modifier::UNDERLINED) {
        codes.push("4".into());
    }
    if style.add_modifier.contains(Modifier::REVERSED) {
        codes.push("7".into());
    }
    if style.add_modifier.contains(Modifier::CROSSED_OUT) {
        codes.push("9".into());
    }
    format!("\x1b[{}m", codes.join(";"))
}

pub fn buffer_to_ansi_string(buf: &Buffer) -> String {
    let area = buf.area;

    let mut last_nonblank = None;
    for y in area.top()..area.bottom() {
        let has_content = (area.left()..area.right()).any(|x| {
            buf.cell((x, y))
                .map_or(false, |c| !c.symbol().trim().is_empty())
        });
        if has_content {
            last_nonblank = Some(y);
        }
    }
    let Some(last_row) = last_nonblank else {
        return String::new();
    };

    let mut out = String::new();
    let mut last_style: Option<Style> = None;

    for y in area.top()..=last_row {
        for x in area.left()..area.right() {
            if let Some(cell) = buf.cell((x, y)) {
                let style = cell.style();
                if Some(style) != last_style {
                    out.push_str(&style_to_sgr(style));
                    last_style = Some(style);
                }
                out.push_str(cell.symbol());
            }
        }
        out.push_str("\x1b[0m");
        last_style = None;
        if y < last_row {
            out.push('\n');
        }
    }
    out
}
