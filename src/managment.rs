use indexmap::IndexMap;
use serde::{Deserialize, Deserializer, Serialize};
use std::{
    collections::HashMap,
    fs,
    hash::Hash,
    path::{Path, PathBuf},
};

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

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum ModRelation {
    Before,
    After,
    Dependency,
    Incompatibility,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Default)]
pub struct ModRules {
    #[serde(default)]
    pub start_anchor: bool,
    #[serde(default)]
    pub end_anchor: bool,
    #[serde(default)]
    pub rules: HashMap<PackageId, ModRelation>,
}

impl ModRules {
    pub fn merge(&mut self, other: Self) {
        self.start_anchor = other.start_anchor;
        self.end_anchor = other.end_anchor;
        self.rules.extend(other.rules);
    }
}

#[derive(Default)]
pub struct ModListIssueCache(pub HashMap<PackageId, HashMap<PackageId, ModRelation>>);

pub struct CondensedModMetadata {
    pub displayable_name: String,
    pub location: PathBuf,
    pub description: String,
}

#[derive(Default)]
pub struct ModList(pub IndexMap<PackageId, CondensedModMetadata>);

impl ModList {
    pub fn autofix(
        &mut self,
        db: &ModRuleDb,
        inactive_list: &mut ModList,
        issue_cache: &mut ModListIssueCache,
    ) -> bool {
        let mut infinite_loop_checker = 100 + self.0.len() + inactive_list.0.len();
        let mut movement_reverse_tracker = HashMap::new();
        let mut index = 0;

        while !issue_cache.0.is_empty() {
            let package_id = self.0.get_index(index).unwrap().0.clone();

            if let Some(issues) = issue_cache.0.get(&package_id) {
                let (problem_package_id, relation) = issues.iter().next().unwrap();

                log::info!(
                    "Solving conflict for mod: {} and {}",
                    package_id.0,
                    problem_package_id.0
                );

                // Exit early as there is probably a circular dependency
                if infinite_loop_checker == 0 {
                    return false;
                }

                match relation {
                    // This ugly thing is to prevent indirect circular dependencies with 3 or more adjacent mods
                    ModRelation::Before | ModRelation::After => {
                        let movement_reverse_tracker = movement_reverse_tracker
                            .entry((package_id.clone(), problem_package_id.clone()))
                            .or_insert(false);

                        if *movement_reverse_tracker {
                            self.0.move_index(
                                self.0.get_index_of(&package_id).unwrap(),
                                self.0.get_index_of(problem_package_id).unwrap(),
                            );
                        } else {
                            self.0.move_index(
                                self.0.get_index_of(problem_package_id).unwrap(),
                                self.0.get_index_of(&package_id).unwrap(),
                            );
                        }

                        *movement_reverse_tracker = !*movement_reverse_tracker;
                    }
                    ModRelation::Dependency => {
                        if inactive_list.0.contains_key(problem_package_id) {
                            self.0.insert(
                                problem_package_id.clone(),
                                inactive_list.0.shift_remove(problem_package_id).unwrap(),
                            );
                        } else {
                            return false;
                        }
                    }
                    ModRelation::Incompatibility => {
                        return false;
                    }
                }

                self.find_list_issues(db, issue_cache);
            } else {
                index += 1;
                infinite_loop_checker -= 1;

                if index >= self.0.len() {
                    index = 0;
                }
            }
        }

        true
    }

    pub fn find_list_issues(&self, db: &ModRuleDb, issue_cache: &mut ModListIssueCache) {
        issue_cache.0.clear();

        for (_, db) in db.0.iter() {
            // Iter over the dbs
            // Iter over the rulesets in each db but only the ones in the list
            for (package_id, rule_entries, package_position) in
                db.iter().filter_map(|(package_id, rule_entries)| {
                    self.0
                        .get_index_of(package_id)
                        .map(|pos| (package_id, rule_entries, pos))
                })
            {
                // Add all the dependencies as problems and we will remove them later when the time comes
                issue_cache.0.entry(package_id.clone()).or_default().extend(
                    rule_entries.rules.iter().filter_map(|(package_id, rule)| {
                        if matches!(rule, ModRelation::Dependency) {
                            return Some((package_id.clone(), ModRelation::Dependency));
                        }

                        None
                    }),
                );

                for (problem_package_id, relation, problem_package_position) in rule_entries
                    .rules
                    .iter()
                    .filter_map(|(package_id, rule_entries)| {
                        self.0
                            .get_index_of(package_id)
                            .map(|pos| (package_id, rule_entries, pos))
                    })
                {
                    match relation {
                        ModRelation::Before => {
                            if package_position > problem_package_position {
                                issue_cache
                                    .0
                                    .entry(package_id.clone())
                                    .or_default()
                                    .insert(problem_package_id.clone(), relation.clone());
                            }
                        }
                        ModRelation::After => {
                            if package_position < problem_package_position {
                                issue_cache
                                    .0
                                    .entry(package_id.clone())
                                    .or_default()
                                    .insert(problem_package_id.clone(), relation.clone());
                            }
                        }
                        ModRelation::Dependency => {
                            // Remove the dependecy entry and do the after check
                            issue_cache
                                .0
                                .entry(package_id.clone())
                                .or_default()
                                .remove(problem_package_id);

                            if package_position < problem_package_position {
                                issue_cache
                                    .0
                                    .entry(package_id.clone())
                                    .or_default()
                                    .insert(problem_package_id.clone(), ModRelation::After);
                            }
                        }
                        ModRelation::Incompatibility => {
                            issue_cache
                                .0
                                .entry(package_id.clone())
                                .or_default()
                                .insert(problem_package_id.clone(), relation.clone());
                        }
                    }
                }
            }
        }

        // Remove empty entries
        issue_cache.0.retain(|_, issues| !issues.is_empty());
    }
}

#[derive(Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum ModdbType {
    ModBuiltRules,
    RuleFile(PathBuf),
}

#[derive(Default, Serialize, Deserialize)]
pub struct ModRuleDb(pub IndexMap<ModdbType, HashMap<PackageId, ModRules>>);

impl ModRuleDb {
    pub fn add_db(&mut self, path: &Path) -> Result<(), anyhow::Error> {
        let db_text = String::from_utf8(fs::read(path)?)?;
        let db = toml::from_str(&db_text)?;

        self.0.insert(ModdbType::RuleFile(path.to_owned()), db);

        Ok(())
    }
}
