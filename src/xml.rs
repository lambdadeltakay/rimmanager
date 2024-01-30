use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::Hash;
use std::path::{Path, PathBuf};

use crate::ui::ModRelation;
use crate::PackageId;
use anyhow::Error;
use homedir::get_my_home;
use indexmap::IndexSet;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_with::formats::CommaSeparator;
use serde_with::serde_as;
use serde_with::DisplayFromStr;
use serde_with::StringWithSeparator;
use versions::{Chunks, Version};

// This folder contains literal XML to rust structures. As such it is not pretty nor fun to use
// Note that quick-xml produces a xml files that RimWorld nor RimSort can parse if no mods are added
// So we must force the user to at least include the mod for the base game

pub fn deserialize_from_xml<T: DeserializeOwned>(string: &str) -> Result<T, Error> {
    Ok(quick_xml::de::from_str(string)?)
}

pub fn serialize_to_xml<T: Serialize>(data: &T) -> Result<String, Error> {
    // quick-xml doesn't add encoding when using serde
    Ok(format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>{}",
        quick_xml::se::to_string(data)?
    ))
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ActiveMods {
    #[serde(default, rename = "li")]
    pub list: IndexSet<PackageId>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct KnownExpansions {
    #[serde(default, rename = "li")]
    pub list: IndexSet<PackageId>,
}

#[serde_as]
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
/// ModsConfig.xml
pub struct ModsConfigData {
    pub version: String,
    pub active_mods: ActiveMods,
    pub known_expansions: KnownExpansions,
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct SupportedVersions {
    #[serde(default, rename = "li")]
    #[serde_as(as = "HashSet<DisplayFromStr>")]
    pub list: HashSet<Version>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ModDependencyInfo {
    /// Id for the mod
    pub package_id: PackageId,
    /// Name for the mod
    pub display_name: String,
    /// Link to the steam workshop for a mod (?)
    pub steam_workshop_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ModDependencies {
    #[serde(default, rename = "li")]
    pub list: Vec<ModDependencyInfo>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct ModDependenciesByVersion {
    #[serde(default, rename = "li")]
    #[serde_as(as = "HashMap<DisplayFromStr, _>")]
    pub map: HashMap<Version, Vec<ModDependencyInfo>>,
}

#[derive(Debug, Deserialize)]
pub struct LoadAfter {
    #[serde(default, rename = "li")]
    pub list: HashSet<PackageId>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct LoadAfterByVersion {
    #[serde(default, rename = "li")]
    #[serde_as(as = "HashMap<DisplayFromStr, _>")]
    pub map: HashMap<Version, HashSet<PackageId>>,
}

#[derive(Debug, Deserialize)]
pub struct LoadBefore {
    #[serde(default, rename = "li")]
    pub list: HashSet<PackageId>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct LoadBeforeByVersion {
    #[serde(default, rename = "li")]
    #[serde_as(as = "HashMap<DisplayFromStr, HashSet<_>>")]
    pub map: HashMap<Version, HashSet<PackageId>>,
}

#[derive(Debug, Deserialize)]
pub struct IncompatibleWith {
    #[serde(default, rename = "li")]
    pub list: HashSet<PackageId>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
pub struct IncompatibleWithByVersion {
    #[serde(default, rename = "li")]
    #[serde_as(as = "HashMap<DisplayFromStr, HashSet<_>>")]
    pub map: HashMap<Version, HashSet<PackageId>>,
}

#[derive(Debug, Deserialize)]
pub struct Authors {
    #[serde(default, rename = "li")]
    pub list: HashSet<String>,
}

#[serde_as]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
/// About.xml
/// This is a mess to try to handle all the horrible edge cases for peoples
/// Hand written xml
pub struct ModMetaData {
    /// A displayable name for the mod
    /// Base game data files don't include this (sigh)
    pub name: Option<String>,
    /// These two fields are mutually exclusive but I won't get mad about it...
    #[serde_as(as = "Option<StringWithSeparator::<CommaSeparator, String>>")]
    pub author: Option<HashSet<String>>,
    pub authors: Option<Authors>,
    // Base files don't contain this
    pub description: Option<String>,
    /// Versions of RimWorld this mod can be run with
    pub supported_versions: Option<SupportedVersions>,
    /// The package id the author made up
    pub package_id: PackageId,
    /// Dependency graph stuff
    load_before: Option<LoadBefore>,
    load_before_by_version: Option<LoadBeforeByVersion>,
    force_load_before: Option<LoadBefore>,

    load_after: Option<LoadAfter>,
    load_after_by_version: Option<LoadAfterByVersion>,
    force_load_after: Option<LoadAfter>,

    mod_dependencies: Option<ModDependencies>,
    mod_dependencies_by_version: Option<ModDependenciesByVersion>,

    incompatible_with: Option<IncompatibleWith>,
    incompatible_with_by_version: Option<IncompatibleWithByVersion>,
}

impl ModMetaData {
    pub fn get_mod_authors(&self) -> HashSet<String> {
        let mut real_authors = HashSet::new();

        if let Some(author) = self.author.as_ref() {
            real_authors.extend(author.clone());
        }

        if let Some(authors) = self.authors.as_ref() {
            real_authors.extend(authors.list.iter().cloned());
        }

        real_authors
    }

    pub fn does_mod_support_this_version(&self, version: Version) -> bool {
        let relevant_version = Version {
            epoch: None,
            chunks: Chunks(vec![
                version.chunks.0[0].clone(),
                version.chunks.0[1].clone(),
            ]),
            release: None,
            meta: None,
        };

        // Base game data files don't include this
        if let Some(supported_versions) = &self.supported_versions {
            return supported_versions.list.contains(&relevant_version);
        }

        true
    }

    pub fn get_dependency_information_for_version(
        &self,
        version: Version,
    ) -> HashMap<PackageId, ModRelation> {
        let mut data = HashMap::new();

        let relevant_version = Version {
            epoch: None,
            chunks: Chunks(vec![
                version.chunks.0[0].clone(),
                version.chunks.0[1].clone(),
            ]),
            release: None,
            meta: None,
        };

        if !self.does_mod_support_this_version(version) {
            return data;
        }

        if let Some(load_before) = &self.load_before {
            data.extend(
                load_before
                    .list
                    .iter()
                    .cloned()
                    .map(|id| (id, ModRelation::Before)),
            );
        }

        if let Some(load_before_by_version) = &self.load_before_by_version {
            if let Some(load_before_by_version) = load_before_by_version.map.get(&relevant_version)
            {
                data.extend(
                    load_before_by_version
                        .iter()
                        .cloned()
                        .map(|id| (id, ModRelation::Before)),
                );
            }
        }

        if let Some(force_load_before) = &self.force_load_before {
            data.extend(
                force_load_before
                    .list
                    .iter()
                    .cloned()
                    .map(|id| (id, ModRelation::Before)),
            );
        }

        if let Some(load_after) = &self.load_after {
            data.extend(
                load_after
                    .list
                    .iter()
                    .cloned()
                    .map(|id| (id, ModRelation::After)),
            );
        }

        if let Some(load_after_by_version) = &self.load_after_by_version {
            if let Some(load_after_by_version) = load_after_by_version.map.get(&relevant_version) {
                data.extend(
                    load_after_by_version
                        .iter()
                        .cloned()
                        .map(|id| (id, ModRelation::After)),
                );
            }
        }

        if let Some(force_load_after) = &self.force_load_after {
            data.extend(
                force_load_after
                    .list
                    .iter()
                    .cloned()
                    .map(|id| (id, ModRelation::After)),
            );
        }

        if let Some(mod_dependencies) = &self.mod_dependencies {
            data.extend(
                mod_dependencies
                    .list
                    .iter()
                    .map(|info| (info.package_id.clone(), ModRelation::Dependency)),
            );
        }

        if let Some(mod_dependencies_by_version) = &self.mod_dependencies_by_version {
            if let Some(mod_dependencies_by_version) =
                &mod_dependencies_by_version.map.get(&relevant_version)
            {
                data.extend(
                    mod_dependencies_by_version
                        .iter()
                        .map(|info| (info.package_id.clone(), ModRelation::Dependency)),
                );
            }
        }

        if let Some(incompatible_with) = &self.incompatible_with {
            data.extend(
                incompatible_with
                    .list
                    .iter()
                    .cloned()
                    .map(|id| (id, ModRelation::Incompatibility)),
            );
        }

        if let Some(incompatible_with_by_version) = &self.incompatible_with_by_version {
            if let Some(incompatible_with_by_version) =
                incompatible_with_by_version.map.get(&relevant_version)
            {
                data.extend(
                    incompatible_with_by_version
                        .iter()
                        .cloned()
                        .map(|id| (id, ModRelation::Incompatibility)),
                );
            }
        }

        data
    }
}

pub fn read_about_xml(mod_location: &Path) -> Result<ModMetaData, Error> {
    let mut about_file_location = mod_location.to_path_buf();
    about_file_location.extend(["About", "About.xml"]);

    if !about_file_location.is_file() {
        // TODO: Error out about a invalid About.xml
        panic!()
    }

    let about_file_data = fs::read(about_file_location)?;
    // I'm gonna take the assumption all RimWorld xml files are in utf8 without checking because so far that seems to be the case
    // TODO: Check if this is always true
    let about_file_string = String::from_utf8(about_file_data)?;
    let about_xml = deserialize_from_xml(&about_file_string)?;

    Ok(about_xml)
}

fn resolve_modconfig_xml_path() -> PathBuf {
    #[cfg(target_os = "linux")]
    let base_path = get_my_home()
        .unwrap()
        .unwrap()
        .join(".config")
        .join("unity3d")
        .join("Ludeon Studios")
        .join("RimWorld by Ludeon Studios")
        .join("Config");

    #[cfg(target_os = "macos")]
    let base_path = get_my_home()
        .unwrap()
        .unwrap()
        .join("Library")
        .join("Application Support")
        .join("RimWorld")
        .join("Config");

    #[cfg(target_os = "windows")]
    let base_path = get_my_home()
        .unwrap()
        .unwrap()
        .join("AppData")
        .join("LocalLow")
        .join("Ludeon Studios")
        .join("RimWorld by Ludeon Studios")
        .join("Config");

    base_path.join("ModsConfig.xml")
}

pub fn read_modconfig_xml() -> Result<ModsConfigData, Error> {
    let modconfig_xml_path = resolve_modconfig_xml_path();

    let modconfig_xml_data = fs::read(modconfig_xml_path)?;
    let modconfig_xml_string = String::from_utf8(modconfig_xml_data)?;
    let modconfig_xml = deserialize_from_xml(&modconfig_xml_string)?;

    Ok(modconfig_xml)
}

pub fn write_modconfig_xml(config: &ModsConfigData) -> Result<(), Error> {
    let modconfig_xml_path = resolve_modconfig_xml_path();

    fs::write(modconfig_xml_path, serialize_to_xml(config)?.as_bytes())?;

    Ok(())
}
