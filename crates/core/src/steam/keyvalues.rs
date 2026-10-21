//! Bounded KeyValues reader for Steam's local text configuration and manifests.
//! Not a general Source-engine interpreter: includes/conditions are refused.
use crate::error::{AppError, Result};

#[derive(Clone, Debug)]
pub enum Value {
    Text(String),
    Object(Vec<(String, Value)>),
}
impl Value {
    pub fn object(&self) -> Result<&[(String, Value)]> {
        match self {
            Self::Object(value) => Ok(value),
            _ => Err(bad("Expected an object")),
        }
    }
    pub fn text(&self) -> Result<&str> {
        match self {
            Self::Text(value) => Ok(value),
            _ => Err(bad("Expected text")),
        }
    }
    pub fn get(&self, key: &str) -> Result<Option<&Value>> {
        let mut values = self
            .object()?
            .iter()
            .filter(|(k, _)| k.eq_ignore_ascii_case(key));
        let result = values.next().map(|(_, v)| v);
        if values.next().is_some() {
            return Err(bad("Duplicate identity or installation field"));
        }
        Ok(result)
    }
    pub fn required(&self, key: &str) -> Result<&str> {
        self.get(key)?
            .ok_or_else(|| bad("A required field is missing"))?
            .text()
    }
}
fn bad(message: &str) -> AppError {
    AppError::new("STEAM_MANIFEST", message)
}
#[derive(Debug)]
enum Token {
    Text(String),
    Open,
    Close,
}
struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
    nodes: usize,
}
impl Reader<'_> {
    fn token(&mut self) -> Result<Option<Token>> {
        loop {
            while self
                .bytes
                .get(self.pos)
                .is_some_and(u8::is_ascii_whitespace)
            {
                self.pos += 1;
            }
            if self.bytes.get(self.pos..self.pos + 2) == Some(b"//") {
                while self.bytes.get(self.pos).is_some_and(|b| *b != b'\n') {
                    self.pos += 1;
                }
                continue;
            }
            break;
        }
        let Some(first) = self.bytes.get(self.pos).copied() else {
            return Ok(None);
        };
        self.pos += 1;
        if first == b'{' {
            return Ok(Some(Token::Open));
        }
        if first == b'}' {
            return Ok(Some(Token::Close));
        }
        if first == b'"' {
            let mut text = Vec::new();
            loop {
                let byte = self
                    .bytes
                    .get(self.pos)
                    .copied()
                    .ok_or_else(|| bad("Unterminated quoted value"))?;
                self.pos += 1;
                if byte == b'"' {
                    break;
                }
                if byte == b'\\' {
                    let next = self
                        .bytes
                        .get(self.pos)
                        .copied()
                        .ok_or_else(|| bad("Incomplete escape"))?;
                    self.pos += 1;
                    match next {
                        b'"' | b'\\' => text.push(next),
                        b'n' => text.push(b'\n'),
                        b't' => text.push(b'\t'),
                        b'r' => text.push(b'\r'),
                        _ => {
                            text.push(b'\\');
                            text.push(next);
                        }
                    }
                } else {
                    text.push(byte);
                }
                if text.len() > 4096 {
                    return Err(bad("Value exceeds the supported length"));
                }
            }
            return Ok(Some(Token::Text(
                String::from_utf8(text).map_err(|_| bad("Invalid UTF-8 value"))?,
            )));
        }
        let start = self.pos - 1;
        while self
            .bytes
            .get(self.pos)
            .is_some_and(|b| !b.is_ascii_whitespace() && !b"{}\"".contains(b))
        {
            self.pos += 1;
        }
        let value = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| bad("Invalid UTF-8 value"))?;
        if value.len() > 4096 || value.starts_with(['#', '[']) {
            return Err(bad("Unsupported directive or oversized value"));
        }
        Ok(Some(Token::Text(value.into())))
    }
    fn object(&mut self, depth: usize, nested: bool) -> Result<Value> {
        if depth > 16 {
            return Err(bad("Configuration nesting exceeds the limit"));
        }
        let mut values = vec![];
        loop {
            let key = match self.token()? {
                None if !nested => break,
                Some(Token::Close) if nested => break,
                Some(Token::Text(key)) => key,
                _ => return Err(bad("Unbalanced object or missing key")),
            };
            self.nodes += 1;
            if self.nodes > 100_000 {
                return Err(bad("Configuration has too many fields"));
            }
            let value = match self.token()? {
                Some(Token::Text(value)) => Value::Text(value),
                Some(Token::Open) => self.object(depth + 1, true)?,
                _ => return Err(bad("Missing field value")),
            };
            values.push((key, value));
        }
        Ok(Value::Object(values))
    }
}
pub fn parse(text: &str) -> Result<Value> {
    if text.len() > 4 * 1024 * 1024 || text.contains('\0') {
        return Err(bad("Configuration is oversized or contains NUL bytes"));
    }
    let mut reader = Reader {
        bytes: text.trim_start_matches('\u{feff}').as_bytes(),
        pos: 0,
        nodes: 0,
    };
    reader.object(0, false)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_nested_unicode_escaped_quotes_backslashes_and_comments() {
        let root=parse("\u{feff}// comment\n\"LibraryFolders\" { \"0\" { \"path\" \"/games/Été \\\"one\\\"\" \"label\" \"C:\\\\Games\" } }").unwrap();
        let lib = root
            .get("libraryfolders")
            .unwrap()
            .unwrap()
            .get("0")
            .unwrap()
            .unwrap();
        assert_eq!(lib.required("path").unwrap(), "/games/Été \"one\"");
        assert_eq!(lib.required("label").unwrap(), "C:\\Games");
    }
    #[test]
    fn refuses_incomplete_ambiguous_or_executable_directives() {
        for text in [
            "\"x\" {",
            "x",
            "{ x y }",
            "x { y }",
            "x \"unterminated",
            "#include \"/other/file\"",
            "x y [$LINUX]",
            "x y\0",
        ] {
            assert!(parse(text).is_err(), "{text:?}");
        }
        assert!(parse("appid 1 AppID 2").unwrap().get("appid").is_err());
        assert!(parse(&"x { ".repeat(30)).is_err());
        assert!(parse(&format!("x \"{}\"", "a".repeat(4097))).is_err());
    }
}
