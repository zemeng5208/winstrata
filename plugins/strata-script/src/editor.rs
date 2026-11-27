//! 三种语言共用的词法着色、补全和诊断。
//!
//! 这里不引入新的编辑器组件。调用方把记号和诊断交给已有界面。

const PYTHON: &[&str] = &[
    "and", "as", "assert", "break", "class", "continue", "def", "elif", "else", "except", "for",
    "from", "if", "import", "in", "not", "or", "pass", "print", "return", "try", "while", "with",
];

const SCRIPT: &[&str] = &[
    "class", "const", "console", "else", "export", "for", "function", "if", "import", "interface",
    "let", "number", "return", "string", "type", "var", "while",
];

pub(crate) fn known_language(name: &str) -> bool {
    matches!(name, "python" | "javascript" | "typescript")
}

pub(crate) fn highlight(language: &str, source: &str) -> Vec<(&'static str, usize)> {
    scan(language, source).tokens
}

pub(crate) fn complete(language: &str, source: &str, prefix: &str) -> Vec<String> {
    if prefix.is_empty() || prefix.len() > 32 {
        return Vec::new();
    }
    let mut items: Vec<String> = words(language)
        .iter()
        .copied()
        .filter(|word| word.starts_with(prefix) && *word != prefix)
        .map(str::to_owned)
        .collect();
    for word in idents(source) {
        if word.starts_with(prefix) && word != prefix && !items.iter().any(|item| item == &word) {
            items.push(word);
        }
    }
    items.sort();
    items.truncate(8);
    items
}

pub(crate) fn diagnose(language: &str, source: &str) -> Vec<(u32, &'static str)> {
    scan(language, source).diagnostics
}

struct Scan {
    tokens: Vec<(&'static str, usize)>,
    diagnostics: Vec<(u32, &'static str)>,
}

fn scan(language: &str, source: &str) -> Scan {
    let mut tokens = Vec::new();
    let mut diagnostics = Vec::new();
    if source.trim().is_empty() {
        diagnostics.push((1, "empty_source"));
        return Scan { tokens, diagnostics };
    }
    let python = language == "python";
    let bytes = source.as_bytes();
    let mut index = 0;
    let mut line = 1u32;
    let mut stack: Vec<(u8, u32)> = Vec::new();
    while index < bytes.len() {
        let start_line = line;
        let byte = bytes[index];
        if byte == b'\n' {
            line += 1;
            index += 1;
            continue;
        }
        if byte.is_ascii_whitespace() {
            index += 1;
            continue;
        }
        if python && byte == b'#' || !python && byte == b'/' && bytes.get(index + 1) == Some(&b'/')
        {
            let start = index;
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            push(&mut tokens, "com", index - start);
            continue;
        }
        if matches!(byte, b'\'' | b'"' | b'`') {
            let quote = byte;
            let start = index;
            index += 1;
            let mut closed = false;
            while index < bytes.len() {
                let current = bytes[index];
                if current == b'\n' {
                    line += 1;
                }
                if current == b'\\' {
                    index += 1;
                    if index < bytes.len() && bytes[index] == b'\n' {
                        line += 1;
                    }
                    index += 1;
                    continue;
                }
                index += 1;
                if current == quote {
                    closed = true;
                    break;
                }
            }
            push(&mut tokens, "str", index - start);
            if !closed {
                diagnostics.push((start_line, "unclosed_string"));
            }
            continue;
        }
        if byte.is_ascii_digit() {
            let start = index;
            while index < bytes.len() && bytes[index].is_ascii_digit() {
                index += 1;
            }
            push(&mut tokens, "num", index - start);
            continue;
        }
        if byte.is_ascii_alphabetic() || byte == b'_' {
            let start = index;
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
            {
                index += 1;
            }
            let word = &source[start..index];
            let kind = if words(language).contains(&word) {
                "kw"
            } else {
                "id"
            };
            push(&mut tokens, kind, index - start);
            continue;
        }
        if matches!(byte, b'(' | b'[' | b'{') {
            stack.push((byte, start_line));
        } else if matches!(byte, b')' | b']' | b'}') {
            let expected = match byte {
                b')' => b'(',
                b']' => b'[',
                _ => b'{',
            };
            match stack.pop() {
                Some((open, _)) if open == expected => {}
                _ => diagnostics.push((start_line, "unmatched_bracket")),
            }
        }
        push(&mut tokens, "punct", 1);
        index += 1;
    }
    for (_, open_line) in stack {
        diagnostics.push((open_line, "unmatched_bracket"));
    }
    tokens.truncate(24);
    diagnostics.truncate(8);
    Scan { tokens, diagnostics }
}

fn push(tokens: &mut Vec<(&'static str, usize)>, kind: &'static str, len: usize) {
    if tokens.len() < 24 && len > 0 {
        tokens.push((kind, len));
    }
}

fn words(language: &str) -> &'static [&'static str] {
    if language == "python" {
        PYTHON
    } else {
        SCRIPT
    }
}

fn idents(source: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    for character in source.chars() {
        if character.is_ascii_alphanumeric() || character == '_' {
            current.push(character);
        } else if !current.is_empty() {
            if current.len() <= 32 && current.as_bytes()[0].is_ascii_alphabetic() {
                items.push(std::mem::take(&mut current));
            } else {
                current.clear();
            }
        }
    }
    if !current.is_empty() && current.as_bytes()[0].is_ascii_alphabetic() {
        items.push(current);
    }
    items
}

pub(crate) fn extension(language: &str) -> &'static str {
    match language {
        "python" => "py",
        "typescript" => "ts",
        _ => "mjs",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostics_cover_empty_unclosed_and_unmatched_forms() {
        assert_eq!(diagnose("python", "  \n").first().copied(), Some((1, "empty_source")));
        assert!(diagnose("javascript", "const value = \"open").iter().any(|item| item.1 == "unclosed_string"));
        assert!(diagnose("typescript", "function value( {\n").iter().any(|item| item.1 == "unmatched_bracket"));
    }

    #[test]
    fn completion_and_highlight_use_the_language_words() {
        assert_eq!(complete("python", "print(1)\n", "pri"), vec!["print".to_owned()]);
        assert!(highlight("python", "print(6)\n").iter().any(|token| token.0 == "kw"));
        assert!(complete("typescript", "const value: number = 1\n", "num").contains(&"number".to_owned()));
    }
}
