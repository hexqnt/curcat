//! Экранирование текста при записи без промежуточных строк.

use std::fmt;

#[derive(Clone, Copy)]
enum Context {
    XmlText,
    XmlAttribute,
    HtmlText,
    MarkdownCell,
}

impl Context {
    const fn replacement(self, ch: char) -> Option<&'static str> {
        match (self, ch) {
            (Self::MarkdownCell, '\\') => Some("\\\\"),
            (Self::MarkdownCell, '|') => Some("\\|"),
            (Self::MarkdownCell, '\r' | '\n') => Some("<br>"),
            (Self::MarkdownCell, _) => None,
            (_, '&') => Some("&amp;"),
            (_, '<') => Some("&lt;"),
            (_, '>') => Some("&gt;"),
            (Self::XmlAttribute | Self::HtmlText, '"') => Some("&quot;"),
            (Self::XmlAttribute, '\'') => Some("&apos;"),
            (Self::XmlAttribute, '\n') => Some("&#10;"),
            (Self::XmlAttribute, '\r') => Some("&#13;"),
            (Self::XmlAttribute, '\t') => Some("&#9;"),
            _ => None,
        }
    }
}

pub(super) struct Escaped<'a> {
    input: &'a str,
    context: Context,
}

impl fmt::Display for Escaped<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut start = 0;
        let mut chars = self.input.char_indices().peekable();
        while let Some((index, ch)) = chars.next() {
            let Some(replacement) = self.context.replacement(ch) else {
                continue;
            };
            f.write_str(&self.input[start..index])?;
            f.write_str(replacement)?;
            start = index + ch.len_utf8();
            // CRLF в Markdown соответствует одному переносу, как и одиночные CR и LF.
            if matches!(self.context, Context::MarkdownCell)
                && ch == '\r'
                && let Some((index, _)) = chars.next_if(|&(_, next)| next == '\n')
            {
                start = index + 1;
            }
        }
        f.write_str(&self.input[start..])
    }
}

pub(super) const fn escape_xml_text(input: &str) -> Escaped<'_> {
    Escaped {
        input,
        context: Context::XmlText,
    }
}

pub(super) const fn escape_xml_attr(input: &str) -> Escaped<'_> {
    Escaped {
        input,
        context: Context::XmlAttribute,
    }
}

pub(super) const fn escape_html_text(input: &str) -> Escaped<'_> {
    Escaped {
        input,
        context: Context::HtmlText,
    }
}

pub(super) const fn escape_markdown_cell(input: &str) -> Escaped<'_> {
    Escaped {
        input,
        context: Context::MarkdownCell,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_only_characters_required_by_each_context() {
        let input = "α<&>\"'\t\r\n|\\конец";
        assert_eq!(
            escape_xml_text(input).to_string(),
            "α&lt;&amp;&gt;\"'\t\r\n|\\конец"
        );
        assert_eq!(
            escape_xml_attr(input).to_string(),
            "α&lt;&amp;&gt;&quot;&apos;&#9;&#13;&#10;|\\конец"
        );
        assert_eq!(
            escape_html_text(input).to_string(),
            "α&lt;&amp;&gt;&quot;'\t\r\n|\\конец"
        );
        assert_eq!(
            escape_markdown_cell(input).to_string(),
            "α<&>\"'\t<br>\\|\\\\конец"
        );
    }

    #[test]
    fn markdown_normalizes_mixed_line_endings() {
        for (input, expected) in [
            ("\r\n", "<br>"),
            ("\r\r\n\n", "<br><br><br>"),
            ("\n\r", "<br><br>"),
            ("а\rб\nв\r\nг", "а<br>б<br>в<br>г"),
        ] {
            assert_eq!(escape_markdown_cell(input).to_string(), expected);
        }
    }

    #[test]
    fn preserves_empty_and_unescaped_unicode_text() {
        for input in ["", "123.456", "Ось θ 📈"] {
            for escaped in [
                escape_xml_text(input),
                escape_xml_attr(input),
                escape_html_text(input),
                escape_markdown_cell(input),
            ] {
                assert_eq!(escaped.to_string(), input);
            }
        }
    }
}
