//! Wii U games in the decrypted folder layout (`code/`, `content/`, `meta/`).
//! Cemu boots the `.rpx` in `code/`; the title and ID live in `meta/meta.xml`.
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WiiuMeta {
    pub title: String,
    /// 16 hex digits, for example `0005000010145000`.
    pub title_id: String,
}

/// Read `meta/meta.xml` beside the `code/` folder that holds `rpx`.
pub fn meta(rpx: &Path) -> Option<WiiuMeta> {
    let code = rpx.parent()?;
    if code.file_name()? != "code" {
        return None;
    }
    let file = code.parent()?.join("meta").join("meta.xml");
    // meta.xml is a few KiB; refuse anything that is not.
    if std::fs::metadata(&file).ok()?.len() > 1024 * 1024 {
        return None;
    }
    parse(&std::fs::read_to_string(file).ok()?)
}

pub fn parse(xml: &str) -> Option<WiiuMeta> {
    let title_id = element(xml, "title_id")?.to_ascii_uppercase();
    if title_id.len() != 16 || !title_id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let title = element(xml, "longname_en")
        .or_else(|| element(xml, "shortname_en"))
        .map(|t| {
            unescape(&t)
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .filter(|t| !t.is_empty())?;
    Some(WiiuMeta { title, title_id })
}

fn element(xml: &str, name: &str) -> Option<String> {
    let open = format!("<{name}");
    let start = xml.find(&open)?;
    let body = start + xml[start..].find('>')? + 1;
    let end = body + xml[body..].find(&format!("</{name}>"))?;
    Some(xml[body..end].trim().to_string())
}

fn unescape(text: &str) -> String {
    text.replace("&#x0A;", " ")
        .replace("&#10;", " ")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    const META: &str = r#"<?xml version="1.0" encoding="utf-8"?>
<menu type="complex" access="777">
  <title_id type="hexBinary" length="8">0005000010145000</title_id>
  <longname_en type="string" length="512">Super Smash Bros.
for Wii U</longname_en>
  <shortname_en type="string" length="256">Super Smash Bros. for Wii U</shortname_en>
</menu>"#;

    #[test]
    fn reads_the_english_title_and_id() {
        assert_eq!(
            parse(META),
            Some(WiiuMeta {
                title: "Super Smash Bros. for Wii U".into(),
                title_id: "0005000010145000".into(),
            })
        );
    }

    #[test]
    fn only_an_rpx_in_code_next_to_meta_counts() {
        let t = tempfile::tempdir().unwrap();
        let game = t.path().join("Smash");
        std::fs::create_dir_all(game.join("code")).unwrap();
        std::fs::create_dir_all(game.join("meta")).unwrap();
        std::fs::write(game.join("meta/meta.xml"), META).unwrap();
        std::fs::write(game.join("code/cross_f.rpx"), b"rpx").unwrap();
        assert_eq!(
            meta(&game.join("code/cross_f.rpx")).unwrap().title_id,
            "0005000010145000"
        );
        assert_eq!(meta(&game.join("cross_f.rpx")), None);
        assert_eq!(parse("<title_id>12</title_id>"), None);
    }
}
