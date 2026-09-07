use crate::{format::format_col, viewer::Viewer};
use arboard::Clipboard;

impl Viewer {
    pub fn yank(&mut self) -> extendr_api::Result<()> {
        if self.schema.is_empty() || self.schema.nrow == 0 {
            return Ok(());
        }
        let (row, col) = self.selected_cell();
        let (name, robj) = self.schema.column_at(&self.data, col, Some(row..row + 1))?;
        let truncate = if self.truncate {
            Some(self.truncate_size)
        } else {
            None
        };
        let table_col = format_col(Some(name), truncate, robj)?;
        let value_text = table_col.values_text.first().ok_or_else(|| {
            extendr_api::Error::Other(
                "internal error: no formatted value for the selected cell".to_string(),
            )
        })?;
        let value_str = value_text
            .lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n");
        copy_to_clipboard(value_str.trim().to_string());
        Ok(())
    }
}

#[cfg(target_os = "linux")]
use arboard::SetExtLinux;
pub fn copy_to_clipboard(text: String) {
    #[cfg(target_os = "linux")]
    {
        std::thread::spawn(move || {
            if let Ok(mut clipboard) = Clipboard::new() {
                let _ = clipboard.set().wait().text(text);
            }
        });
    }
    #[cfg(not(target_os = "linux"))]
    {
        if let Ok(mut clipboard) = Clipboard::new() {
            let _ = clipboard.set_text(text);
        }
    }
}
