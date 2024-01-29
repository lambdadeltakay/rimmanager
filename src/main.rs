mod ui;
mod xml;

use anyhow::Error;
use serde::{Deserialize, Deserializer, Serialize};
use std::{path::Path, str::FromStr};
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
            Box::<RimManager>::default()
        }),
    )
    .unwrap();
}
