//! Optional, cached provider images in the existing game-media gallery.
//! Images never establish ownership, change game identity or replace local art.
use crate::{
    http,
    model::{ArtworkReport, Edition},
    romart, romfacts, Core,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, fs, path::Path};

#[derive(Clone, Debug, Serialize, Deserialize)]
struct RemoteImage {
    kind: String,
    url: String,
}

fn image_list(json: &Value, id: u64) -> Vec<RemoteImage> {
    let Some(images) = json["data"]["images"][id.to_string()].as_array() else {
        return vec![];
    };
    let mut counts = BTreeMap::new();
    images
        .iter()
        .filter_map(|image| {
            let kind = match image["type"].as_str()? {
                "boxart" if image["side"] == "front" => "cover",
                "screenshot" => "screenshot",
                "fanart" => "fanart",
                "titlescreen" => "titlescreen",
                _ => return None,
            };
            let count = counts.entry(kind).or_insert(0);
            if *count >= if kind == "screenshot" { 2 } else { 1 } {
                return None;
            }
            let filename = image["filename"].as_str()?;
            if filename.starts_with('/')
                || filename
                    .split('/')
                    .any(|p| p.is_empty() || p == "." || p == "..")
                || !filename
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"/_.-".contains(&b))
                || ![".png", ".jpg", ".jpeg"]
                    .iter()
                    .any(|suffix| filename.ends_with(suffix))
            {
                return None;
            }
            *count += 1;
            Some(RemoteImage {
                kind: kind.into(),
                url: format!("https://cdn.thegamesdb.net/images/original/{filename}"),
            })
        })
        .collect()
}

fn provider_images(
    cache: &Path,
    key: &str,
    id: u64,
    force: bool,
    budget: &mut usize,
) -> Vec<RemoteImage> {
    let file = cache.join(format!("game-image-lists/thegamesdb-{id}.json"));
    if !force && romart::fresh(&file) {
        return romart::load(&file);
    }
    if *budget == 0 {
        return romart::load(&file);
    }
    *budget -= 1;
    let url = format!("https://api.thegamesdb.net/v1/Games/Images?apikey={}&games_id={id}&filter%5Btype%5D=boxart,screenshot,fanart,titlescreen",romart::encode(key));
    let Ok(json) = http::get_json(&url) else {
        return romart::load(&file);
    };
    if json["remaining_monthly_allowance"]
        .as_i64()
        .is_some_and(|left| left < 25)
    {
        *budget = 0;
    }
    let images = image_list(&json, id);
    if fs::create_dir_all(file.parent().unwrap()).is_ok() {
        romart::save(&file, &images);
    }
    // Only the public CDN URLs are cached, never API pages containing the key.
    images
}

fn save_gallery(
    core: &Core,
    edition: &Edition,
    image: &RemoteImage,
    bytes: &[u8],
) -> crate::Result<bool> {
    romart::validate_image(bytes)?;
    let id = format!("provider-images:{}", edition.id);
    let collections = core.media_collections(Some(&edition.id))?;
    if collections
        .iter()
        .flat_map(|collection| &collection.assets)
        .any(|asset| asset.source == image.url && asset.available)
    {
        return Ok(false);
    }
    let reference = image
        .url
        .starts_with("https://cdn.thegamesdb.net/")
        .then_some(edition);
    let ready = core.with_db(|db| {
        if reference.is_some() && !romfacts::reference_is_current(db, edition)? {
            return Ok(false);
        }
        if db.edition_by_id(&edition.id)?.is_none() {
            return Err(crate::AppError::new(
                "GAME_MEDIA",
                "Game was removed during refresh",
            ));
        }
        db.conn.execute(
            "INSERT OR IGNORE INTO game_media_collections(id,title) VALUES(?1,?2)",
            rusqlite::params![
                id,
                format!(
                    "Downloaded images · {}",
                    romart::plain_title(&edition.title)
                )
            ],
        )?;
        db.conn.execute(
            "INSERT OR IGNORE INTO game_media_links(collection_id,game_key) VALUES(?1,?2)",
            rusqlite::params![id, edition.id],
        )?;
        Ok(true)
    })?;
    if !ready {
        return Ok(false);
    }
    let before = core
        .media_collections(Some(&edition.id))?
        .iter()
        .find(|c| c.id == id)
        .map_or(0, |c| c.assets.len());
    let extension = romart::image_kind(bytes).unwrap_or("png");
    let label = match image.kind.as_str() {
        "screenshot" => "Screenshot",
        "fanart" => "Background artwork",
        _ => "Title screen",
    };
    let label = if image.kind == "screenshot" {
        let number = collections
            .iter()
            .find(|collection| collection.id == id)
            .map_or(1, |collection| {
                collection
                    .assets
                    .iter()
                    .filter(|asset| {
                        asset.title.starts_with("Screenshot ")
                            || asset.title.starts_with("Gameplay screenshot ")
                    })
                    .count()
                    + 1
            });
        format!("{label} {number}")
    } else {
        label.to_string()
    };
    core.media_store_checked(
        &id,
        &format!("{label}.{extension}"),
        &image.url,
        bytes,
        bytes.len() as u64,
        reference,
    )?;
    let after = core
        .media_collections(Some(&edition.id))?
        .iter()
        .find(|c| c.id == id)
        .map_or(0, |c| c.assets.len());
    Ok(after > before)
}

pub fn run(core: &Core, edition_id: Option<&str>, report: &mut ArtworkReport) {
    let Ok(snapshot) = core.snapshot() else {
        return;
    };
    let key = core.key("thegamesdb").ok().flatten();
    let names: BTreeMap<String, String> = romart::load(&core.paths.cache_dir.join("romnames.json"));
    let misses_file = core.paths.cache_dir.join("game-images-misses.json");
    let mut misses: BTreeMap<String, u64> = romart::load(&misses_file);
    let now = romart::now_secs();
    let mut downloaded = 0usize;
    let mut budget = 30;
    for edition in snapshot
        .editions
        .iter()
        .filter(|e| !e.missing && edition_id.is_none_or(|id| id == e.id))
    {
        if downloaded >= 64 * 1024 * 1024 {
            break;
        }
        let collections = core
            .media_collections(Some(&edition.id))
            .unwrap_or_default();
        let existing: Vec<_> = collections
            .iter()
            .flat_map(|c| &c.assets)
            .filter(|a| a.available)
            .map(|a| a.source.as_str())
            .collect();
        let mut images = edition
            .info
            .as_ref()
            .and_then(romfacts::tgdb_id)
            .zip(key.as_deref())
            .map(|(id, key)| {
                provider_images(
                    &core.paths.cache_dir,
                    key,
                    id,
                    edition_id.is_some(),
                    &mut budget,
                )
            })
            .unwrap_or_default();
        if let Some(system) = romart::source_for(&edition.system) {
            let name = names
                .get(&edition.id)
                .map(String::as_str)
                .or_else(|| system.dat.is_none().then_some(edition.title.as_str()));
            if let Some(name) = name {
                for (kind, folder) in [
                    ("screenshot", "Named_Snaps"),
                    ("titlescreen", "Named_Titles"),
                ] {
                    if !images.iter().any(|image| image.kind == kind) {
                        if let Some(url) =
                            romart::indexed_thumbnail(&core.paths.cache_dir, system, name, folder)
                        {
                            images.push(RemoteImage {
                                kind: kind.into(),
                                url,
                            });
                        }
                    }
                }
            }
        }
        for image in images {
            if downloaded >= 64 * 1024 * 1024 {
                break;
            }
            if image.kind == "cover"
                && edition
                    .cover_path
                    .as_ref()
                    .is_some_and(|path| path.is_file())
            {
                continue;
            }
            if existing.contains(&image.url.as_str())
                || (edition_id.is_none()
                    && misses
                        .get(&image.url)
                        .is_some_and(|when| now.saturating_sub(*when) < 7 * 24 * 3600))
            {
                continue;
            }
            let Ok(bytes) = http::get_bytes(&image.url) else {
                misses.insert(image.url, now);
                continue;
            };
            downloaded += bytes.len();
            let result = if image.kind == "cover" {
                let directory = core.paths.cache_dir.join("art");
                fs::create_dir_all(&directory).ok();
                romart::store_missing(core, &directory, edition, &bytes, "thegamesdb").inspect(
                    |stored| {
                        if *stored {
                            report.fetched += 1;
                        }
                    },
                )
            } else {
                save_gallery(core, edition, &image, &bytes).inspect(|added| {
                    if *added {
                        report.media_added += 1;
                    }
                })
            };
            if result.is_err() {
                misses.insert(image.url, now);
                report.failed += 1;
            } else {
                misses.remove(&image.url);
            }
        }
    }
    romart::save(&misses_file, &misses);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stale_provider_downloads_cannot_reapply_a_replaced_match() {
        let temp = tempfile::tempdir().unwrap();
        let core = Core::open_isolated(Some(temp.path().join("library")), false).unwrap();
        core.with_db(|db| {
            db.conn.execute("INSERT INTO editions(id,system,scan_title,imported_at) VALUES('game','ps1','My game','2026-01-01')", [])?;
            romfacts::upsert(db, "game", &crate::model::GameInfo {
                source: "thegamesdb:7".into(), ..Default::default()
            })
        }).unwrap();
        let previous = core.snapshot().unwrap().editions.remove(0);
        core.with_db(|db| {
            romfacts::upsert(
                db,
                "game",
                &crate::model::GameInfo {
                    source: "thegamesdb:7+thegamesdb:8".into(),
                    ..Default::default()
                },
            )
        })
        .unwrap();
        assert!(!core
            .with_db(|db| romfacts::reference_is_current(db, &previous))
            .unwrap());
        let mut png = std::io::Cursor::new(vec![]);
        image::DynamicImage::new_rgb8(8, 8)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let bytes = png.into_inner();
        let remote = RemoteImage {
            kind: "screenshot".into(),
            url: "https://cdn.thegamesdb.net/images/original/screenshots/7.jpg".into(),
        };
        assert!(!save_gallery(&core, &previous, &remote, &bytes).unwrap());
        assert!(core.media_collections(Some("game")).unwrap().is_empty());
        fs::create_dir_all(&core.paths.cache_dir).unwrap();
        assert!(!romart::store_missing(
            &core,
            &core.paths.cache_dir,
            &previous,
            &bytes,
            "thegamesdb"
        )
        .unwrap());
        assert!(core.snapshot().unwrap().editions[0].cover_path.is_none());
        let collection = core.media_create("Gallery", Some("game")).unwrap();
        assert!(core
            .media_store_checked(
                &collection,
                "Stale.png",
                &remote.url,
                bytes.as_slice(),
                bytes.len() as u64,
                Some(&previous)
            )
            .is_err());
        assert!(core.media_collections(Some("game")).unwrap()[0]
            .assets
            .is_empty());
    }

    #[test]
    fn provider_gallery_is_idempotent_and_keeps_local_images_and_editions() {
        let temp = tempfile::tempdir().unwrap();
        let core = Core::open_isolated(Some(temp.path().join("library")), false).unwrap();
        core.with_db(|db| { db.conn.execute("INSERT INTO editions(id,system,scan_title,imported_at,favorite) VALUES('game','ps1','My game','2026-01-01',1)",[])?; Ok(()) }).unwrap();
        let edition = core.snapshot().unwrap().editions.remove(0);
        let user = core
            .media_create("My own screenshots", Some("game"))
            .unwrap();
        let mut png = std::io::Cursor::new(vec![]);
        image::DynamicImage::new_rgb8(8, 8)
            .write_to(&mut png, image::ImageFormat::Png)
            .unwrap();
        let bytes = png.into_inner();
        core.media_store(
            &user,
            "My screenshot.png",
            "local screenshot",
            bytes.as_slice(),
            bytes.len() as u64,
        )
        .unwrap();
        let user_cover = core.paths.cache_dir.join("my-cover.png");
        fs::write(&user_cover, &bytes).unwrap();
        core.with_db(|db| {
            db.upsert_artwork(
                &edition.id,
                "cover",
                &user_cover.to_string_lossy(),
                "manual",
                "2026-01-01",
            )
        })
        .unwrap();
        assert!(
            !romart::store_missing(&core, &core.paths.cache_dir, &edition, &bytes, "provider")
                .unwrap()
        );
        assert_eq!(
            core.snapshot().unwrap().editions[0].cover_path.as_ref(),
            Some(&user_cover)
        );
        let remote = RemoteImage {
            kind: "screenshot".into(),
            url: "https://cdn.thegamesdb.net/images/original/screenshots/7.jpg".into(),
        };
        assert!(save_gallery(&core, &edition, &remote, &bytes).unwrap());
        assert!(!save_gallery(&core, &edition, &remote, &bytes).unwrap());
        assert!(save_gallery(
            &core,
            &edition,
            &RemoteImage {
                kind: "screenshot".into(),
                url: "https://cdn.thegamesdb.net/images/original/screenshots/8.jpg".into()
            },
            b"invalid image bytes"
        )
        .is_err());
        let collections = core.media_collections(Some("game")).unwrap();
        assert_eq!(collections.len(), 2);
        assert!(collections
            .iter()
            .any(|collection| collection.id == user
                && collection.assets[0].source == "local screenshot"));
        assert_eq!(
            collections
                .iter()
                .map(|collection| collection.assets.len())
                .sum::<usize>(),
            2
        );
        let current = core.snapshot().unwrap().editions.remove(0);
        assert_eq!(current.id, edition.id);
        assert_eq!(current.title, edition.title);
        assert!(current.favorite);
    }

    #[test]
    fn image_lists_are_game_bound_bounded_and_do_not_trust_provider_urls() {
        let json = serde_json::json!({"data":{"base_url":{"original":"https://host.invalid/"},"images":{"7":[
            {"type":"screenshot","filename":"screenshots/7-1.jpg"},
            {"type":"screenshot","filename":"screenshots/7-2.jpg"},
            {"type":"screenshot","filename":"screenshots/7-3.jpg"},
            {"type":"fanart","filename":"../private.png"},
            {"type":"fanart","filename":"https://host.invalid/x.png"},
            {"type":"fanart","filename":"fanart/7-1.png"},
            {"type":"boxart","side":"back","filename":"boxart/back/7.jpg"},
            {"type":"boxart","side":"front","filename":"boxart/front/7.jpg"}
        ],"8":[{"type":"screenshot","filename":"screenshots/wrong-game.jpg"}]}}});
        let images = image_list(&json, 7);
        assert_eq!(images.len(), 4);
        assert!(images.iter().all(|image| image
            .url
            .starts_with("https://cdn.thegamesdb.net/images/original/")));
        assert!(!images
            .iter()
            .any(|image| image.url.contains("wrong-game") || image.url.contains("..")));
    }
}
