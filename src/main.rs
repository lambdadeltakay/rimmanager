mod ui;
mod xml;

use anyhow::Error;
use serde::{Deserialize, Deserializer, Serialize};
use std::{fs, path::Path, str::FromStr};
use ui::RimManager;
use versions::Version;

/// Forces the PackageIds to be lowercase
fn deserialize_package_id<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    let original_string: String = Deserialize::deserialize(deserializer)?;
    Ok(original_string.to_lowercase())
}

#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Hash, Clone, PartialOrd, Ord)]
pub struct PackageId(#[serde(deserialize_with = "deserialize_package_id")] pub String);

pub fn parse_game_version(raw: &str) -> Result<Version, Error> {
    let version = raw.split(' ').next().unwrap();
    Ok(Version::from_str(version).unwrap())
}

/// Basic check for if this game directory is valid
pub fn does_directory_represent_valid_game_installation(game_dir: &Path) -> bool {
    game_dir.is_dir() && game_dir.join("Version.txt").is_file() && game_dir.join("Data").is_dir()
}

/// Basic check for if this steam prefix is valid
pub fn does_directory_represent_valid_steam_prefix(steam_dir: &Path) -> bool {
    steam_dir.is_dir() && steam_dir.join("steamapps").is_dir()
}

fn main() {
    env_logger::init();

    let options = eframe::NativeOptions {
        vsync: true,
        follow_system_theme: true,
        ..Default::default()
    };

    eframe::run_native(
        "RimManager",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);

            let mut font_db = fontdb::Database::new();
            font_db.load_system_fonts();

            let query = fontdb::Query {
                families: &[fontdb::Family::SansSerif],
                ..fontdb::Query::default()
            };

            // FIXME: Note that I can't get this to work on Linux
            if let Some(id) = font_db.query(&query) {
                let (src, _) = font_db.face_source(id).unwrap();

                if let fontdb::Source::File(path) = &src {
                    let mut fonts = egui::FontDefinitions::default();

                    let font_data = fs::read(path).unwrap();
                    let font_system_sans_serif = "System Sans Serif";

                    fonts.font_data.insert(
                        font_system_sans_serif.to_owned(),
                        egui::FontData::from_owned(font_data),
                    );

                    fonts
                        .families
                        .entry(egui::FontFamily::Proportional)
                        .or_default()
                        .insert(0, font_system_sans_serif.to_owned());
                    
                    cc.egui_ctx.set_fonts(fonts);
                }
            }

            Box::<RimManager>::default()
        }),
    )
    .unwrap();
}
