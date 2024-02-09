use std::{collections::HashSet, fs, path::PathBuf};

use crate::{
    does_directory_represent_valid_game_installation, does_directory_represent_valid_steam_prefix,
    managment::{CondensedModMetadata, ModList, ModListIssueCache, ModRuleDb, PackageId},
    parse_game_version,
    xml::{read_about_xml, read_modconfig_xml, write_modconfig_xml},
};
use anyhow::Error;
use egui::{Button, Image};
use egui_dnd::dnd;
use egui_file::FileDialog;
use egui_modal::Modal;
use homedir::get_my_home;

// TODO: Reorganize this and remove the code duplication
// FIXME: A lot of redundant data being held here!!
// TODO: Extract enough data that we don't carry about the About.xml for every mod. We are trying to save every cpu cycle and byte here
// TODO: We might add loading screens and stuff althrough its not exactly needed considering how fast our code is

#[derive(Default)]
pub struct RimManager {
    /// Path to the game installation
    pub game_path: Option<PathBuf>,
    /// Path to the game installation
    pub steam_path: Option<PathBuf>,
    /// File picker dialog to get to the installation
    pub game_path_picker_dialog: Option<FileDialog>,
    /// File picker dialog to get to the installation
    pub steam_path_picker_dialog: Option<FileDialog>,
    /// Paths including locations for mods
    pub mod_folder_paths: HashSet<PathBuf>,
    /// Mod being displayed in the sidebar
    pub currently_selected_mod: Option<PackageId>,
    /// List of mods that can be written or read into
    pub active_mod_list: ModList,
    pub inactive_mod_list: ModList,
    /// Search bar for inactive mods
    pub inactive_search: String,
    /// Search bar for active mods
    pub active_search: String,
    /// Rule stuff
    pub mod_rules: ModRuleDb,
    pub mod_list_issue_cache: ModListIssueCache,
}

impl RimManager {
    pub fn refresh_metadata(&mut self) -> Result<(), Error> {
        self.active_mod_list.0.clear();
        self.inactive_mod_list.0.clear();
        self.mod_list_issue_cache.0.clear();

        self.currently_selected_mod = None;

        // Grab the game version
        let mut version_file_path = self.game_path.clone().unwrap();
        version_file_path.extend(["Version.txt"]);
        let game_version_file = String::from_utf8(fs::read(version_file_path)?)?;
        let game_version = parse_game_version(&game_version_file)?;

        let mut scan_paths = Vec::new();

        // Base game data files
        scan_paths.push(self.game_path.clone().unwrap().join("Data"));
        // Normal Mod folder
        scan_paths.push(self.game_path.clone().unwrap().join("Mods"));
        // Steam mod folder
        if let Some(steam_prefix) = &self.steam_path {
            let path = steam_prefix
                .join("steamapps")
                .join("workshop")
                .join("content")
                .join("294100");

            if path.is_dir() {
                scan_paths.push(path);
            }
        }

        // Look in the directories to scan
        for scan_dir in self.mod_folder_paths.iter().chain(&scan_paths) {
            // Find the folders of the mods
            for mod_folder in scan_dir.read_dir()? {
                // Get all the folders we can read
                // TODO: Warn about folders we can't read? Can't imagine this being too much of a issue through
                if let Ok(mod_folder) = mod_folder.map(|folder| folder.path()) {
                    // Only interact with directories
                    if !mod_folder.is_dir() {
                        continue;
                    }

                    log::info!(
                        "Beginning inspection of mod located at: {}",
                        mod_folder.display()
                    );

                    if let Ok(about_file_xml) = read_about_xml(&mod_folder) {
                        if !about_file_xml.does_mod_support_this_version(game_version.clone()) {
                            log::info!("Skipping mod");
                            continue;
                        }

                        about_file_xml.load_dependency_information_for_version(
                            game_version.clone(),
                            &mut self.mod_rules,
                        );

                        self.inactive_mod_list.0.insert(
                            about_file_xml.package_id.clone(),
                            CondensedModMetadata {
                                displayable_name: about_file_xml
                                    .name
                                    .unwrap_or(about_file_xml.package_id.0.to_string()),
                                location: mod_folder,
                                description: about_file_xml.description,
                            },
                        );
                    } else {
                        log::warn!("Can't parse this mods About.xml!");
                    }
                }
            }
        }

        Ok(())
    }

    pub fn create_mod_list_panel(
        &mut self,
        ctx: &egui::Context,
        is_active_list: bool,
        // The change and the changing problem
    ) -> Option<PackageId> {
        let mut currently_selected = None;

        let (list_name, searcher) = if is_active_list {
            ("active", &mut self.active_search)
        } else {
            ("inactive", &mut self.inactive_search)
        };

        // Mod manager panel
        egui::SidePanel::left(list_name.to_owned() + "_mod_list")
            .resizable(true)
            .show(ctx, |ui| {
                ui.label(list_name.to_owned() + " mods");

                ui.horizontal(|ui| {
                    ui.label("🔎");
                    ui.text_edit_singleline(searcher);
                });
                ui.separator();

                egui::ScrollArea::vertical()
                    .id_source(list_name.to_owned() + "_mod_scroll_area")
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            let mut mod_to_change = None;

                            let list_to_display = if is_active_list {
                                &self.active_mod_list
                            } else {
                                &self.inactive_mod_list
                            };

                            let drag_result = dnd(ui, list_name.to_owned() + "_mod_list").show(
                                list_to_display
                                    .0
                                    .iter()
                                    .map(|(package_id, package_metadeta)| {
                                        (package_id, &package_metadeta.displayable_name)
                                    })
                                    // Filter out items that don't match the search
                                    .filter(|(_, displayable_name)| {
                                        searcher.is_empty()
                                            || displayable_name
                                                .to_ascii_lowercase()
                                                .contains(&searcher.to_ascii_lowercase())
                                    }),
                                |ui, (item, displayable_name), handle, _| {
                                    ui.horizontal(|ui| {
                                        // Disallow drag and drop when the user is searching
                                        if searcher.is_empty() {
                                            handle.ui(ui, |ui| {
                                                ui.label("=");
                                            });
                                        }

                                        if ui.button("↔").clicked() {
                                            mod_to_change = Some(item.clone());
                                        }

                                        if is_active_list
                                            && self.mod_list_issue_cache.0.contains_key(item)
                                        {
                                            ui.label("🚫");
                                        }

                                        if ui
                                            .add(egui::Button::new(displayable_name).wrap(true))
                                            .clicked()
                                        {
                                            // Chance to cause failure
                                            mod_to_change = None;
                                            currently_selected = Some(item.clone());
                                        }
                                    });

                                    ui.end_row();
                                },
                            );

                            if let Some(drag_result) = drag_result.final_update() {
                                let my_list = if is_active_list {
                                    &mut self.active_mod_list
                                } else {
                                    &mut self.inactive_mod_list
                                };

                                // This looks strange and hacky but it creates a more natural dragging operation
                                match drag_result.from.cmp(&drag_result.to) {
                                    std::cmp::Ordering::Less => {
                                        my_list.0.move_index(drag_result.from, drag_result.to - 1)
                                    }
                                    std::cmp::Ordering::Equal => (),
                                    std::cmp::Ordering::Greater => {
                                        my_list.0.move_index(drag_result.from, drag_result.to)
                                    }
                                }

                                if is_active_list {
                                    self.active_mod_list.find_list_issues(
                                        &self.mod_rules,
                                        &mut self.mod_list_issue_cache,
                                    );
                                }
                            }

                            if let Some(mod_to_change) = mod_to_change {
                                let (other_list, my_list) = if is_active_list {
                                    (&mut self.inactive_mod_list, &mut self.active_mod_list)
                                } else {
                                    (&mut self.active_mod_list, &mut self.inactive_mod_list)
                                };

                                other_list.0.insert(
                                    mod_to_change.clone(),
                                    my_list.0.shift_remove(&mod_to_change).unwrap(),
                                );

                                self.active_mod_list.find_list_issues(
                                    &self.mod_rules,
                                    &mut self.mod_list_issue_cache,
                                );
                            }
                        });
                    });
            });

        currently_selected
    }
}

impl eframe::App for RimManager {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Modal for when a the user tries to save a mod list without the core mod
        let missing_core_on_modlist_modal = alert_box(
            ctx,
            "Your mod-list must contain the Core module (ludeon.rimworld)",
        );

        // Modal for when a the user tries to save a mod list without the core mod
        let mod_list_unresolved_issues_modal = alert_box(
            ctx,
            "Your mod list has mistakes which must be resolved before saving",
        );

        let invalid_game_path_modal = alert_box(
            ctx,
            "The path you selected does not represent a valid RimWorld installation!",
        );

        let invalid_steam_path_modal = alert_box(
            ctx,
            "The path you selected does not represent a valid Steam prefix!",
        );

        let unfixable_modlist_modal = alert_box(
            ctx,
            "The mod list has dependencies not installed, incompatible mods in the active list, or a direct circular dependency. Aborting sorting",
        );

        egui::TopBottomPanel::top("manager").show(ctx, |ui| {
            ui.horizontal(|ui| {
                egui::Grid::new("button_grid").striped(true).show(ui, |ui| {
                    // Only enable the ability to scan installation once the user sets a game path
                    if ui
                        .add_enabled(self.game_path.is_some(), Button::new("Scan installation"))
                        .clicked()
                    {
                        self.refresh_metadata().unwrap();
                    }

                    ui.end_row();

                    if ui
                        .add_enabled(self.game_path.is_some(), Button::new("Load mod ordering"))
                        .clicked()
                    {
                        let mod_ordering = read_modconfig_xml().unwrap();
                        self.refresh_metadata().unwrap();
                        // Active mod list will be empty by here

                        // Check for mods in our known mods and add them
                        for mod_id in &mod_ordering.active_mods.list {
                            if self.inactive_mod_list.0.contains_key(mod_id) {
                                self.active_mod_list.0.insert(
                                    mod_id.clone(),
                                    self.inactive_mod_list.0.shift_remove(mod_id).unwrap(),
                                );
                            }
                        }

                        self.active_mod_list
                            .find_list_issues(&self.mod_rules, &mut self.mod_list_issue_cache);
                    }

                    ui.end_row();

                    if ui
                        .add_enabled(self.game_path.is_some(), Button::new("Save mod ordering"))
                        .clicked()
                    {
                        if !self
                            .active_mod_list
                            .0
                            .contains_key(&PackageId("ludeon.rimworld".to_owned()))
                        {
                            missing_core_on_modlist_modal.open();
                        } else if !self.mod_list_issue_cache.0.is_empty() {
                            mod_list_unresolved_issues_modal.open();
                        } else {
                            let mut mod_config_data = read_modconfig_xml().unwrap();
                            mod_config_data.active_mods.list =
                                self.active_mod_list.0.keys().cloned().collect();

                            write_modconfig_xml(&mod_config_data).unwrap();
                        }
                    }

                    if ui
                        .add_enabled(
                            !self.mod_list_issue_cache.0.is_empty(),
                            Button::new("Fix mod ordering"),
                        )
                        .clicked()
                        && !self.active_mod_list.autofix(
                            &self.mod_rules,
                            &mut self.inactive_mod_list,
                            &mut self.mod_list_issue_cache,
                        )
                    {
                        unfixable_modlist_modal.open();
                    }

                    ui.end_row();
                });

                egui::Grid::new("picker_grid").striped(true).show(ui, |ui| {
                    if ui.button("Game Path").clicked() {
                        let mut folder_picker =
                            FileDialog::select_folder(Some(get_my_home().unwrap().unwrap()))
                                .show_new_folder(false)
                                .title("Pick a valid RimWorld installation");
                        folder_picker.open();
                        self.game_path_picker_dialog = Some(folder_picker);
                    }

                    if let Some(path) = &self.game_path {
                        ui.label(path.to_string_lossy());
                    }

                    ui.end_row();

                    if ui
                        .add_enabled(
                            self.game_path.is_some(),
                            egui::Button::new("Steam Prefix Path"),
                        )
                        .clicked()
                    {
                        let mut folder_picker =
                            FileDialog::select_folder(Some(get_my_home().unwrap().unwrap()))
                                .show_new_folder(false)
                                .title("Pick a valid Steam prefix!");
                        folder_picker.open();
                        self.steam_path_picker_dialog = Some(folder_picker);
                    }

                    if let Some(path) = &self.steam_path {
                        ui.label(path.to_string_lossy());
                    }

                    ui.end_row();
                });
            });
        });

        let change_mod_active = self.create_mod_list_panel(ctx, true);
        let change_mod_inactive = self.create_mod_list_panel(ctx, false);

        // In reality both of these are not pressable at the same time but...
        // I can't figure out how to express this better so...

        if change_mod_active.is_some() {
            self.currently_selected_mod = change_mod_active;
        }

        if change_mod_inactive.is_some() {
            self.currently_selected_mod = change_mod_inactive;
        }

        // Mod info panel
        egui::CentralPanel::default().show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.vertical(|ui| {
                    if let Some(selected_mod) = &self.currently_selected_mod {
                        ui.label(selected_mod.0.as_str());

                        let mod_info = if let Some(path) = self.active_mod_list.0.get(selected_mod)
                        {
                            path
                        } else if let Some(path) = self.inactive_mod_list.0.get(selected_mod) {
                            path
                        } else {
                            unreachable!();
                        };

                        // Work around for windows users not casing files correctly
                        let image_path = mod_info.location.join("About").join("Preview.png");
                        let other_possible_image_path =
                            mod_info.location.join("About").join("preview.png");

                        let actual_path = [image_path, other_possible_image_path]
                            .into_iter()
                            .find(|path| path.is_file());

                        // There is really no nice looking way to do this since RimWorld mods have all different images sizes
                        if let Some(actual_path) = actual_path {
                            ui.add(
                                Image::from_uri(
                                    "file://".to_string() + actual_path.to_string_lossy().as_ref(),
                                )
                                .max_height(500.0),
                            );
                        }

                        ui.separator();

                        ui.label("Dependency issues");

                        egui::Grid::new("problem_grid")
                            .striped(true)
                            .show(ui, |ui| {
                                if let Some(problem_mod) = &self.currently_selected_mod {
                                    if let Some(problems) =
                                        self.mod_list_issue_cache.0.get(problem_mod)
                                    {
                                        for (problem_id, problem_relation) in problems {
                                            ui.label(problem_id.clone().0);
                                            ui.separator();
                                            ui.label(format!("{:?}", problem_relation));
                                            ui.end_row();
                                        }
                                    }
                                }
                            });

                        ui.separator();

                        ui.label("Description");
                        // Mods use a special steam specific markdown
                        // I'm not writing a parser for that lmaooo
                        ui.label(&mod_info.description);
                    }
                });
            });
        });

        // Open the game picker if the user chooses it
        if let Some(game_installation_picker) = &mut self.game_path_picker_dialog {
            if game_installation_picker.show(ctx).selected() {
                if let Some(file) = game_installation_picker.path() {
                    if does_directory_represent_valid_game_installation(file) {
                        self.game_path = Some(file.to_path_buf());
                        self.game_path_picker_dialog = None;
                        self.refresh_metadata().unwrap();
                    } else {
                        invalid_game_path_modal.open();
                    }
                }
            }
        }

        // Open the steam picker if the user chooses it
        if let Some(steam_prefix_picker) = &mut self.steam_path_picker_dialog {
            if steam_prefix_picker.show(ctx).selected() {
                if let Some(file) = steam_prefix_picker.path() {
                    if does_directory_represent_valid_steam_prefix(file) {
                        self.steam_path = Some(file.to_path_buf());
                        self.steam_path_picker_dialog = None;
                        self.refresh_metadata().unwrap();
                    } else {
                        invalid_steam_path_modal.open();
                    }
                }
            }
        }
    }
}

pub fn alert_box(ctx: &egui::Context, body: &str) -> Modal {
    let alert_box = Modal::new(ctx, body);

    alert_box.show(|ui| {
        alert_box.title(ui, "Non Fatal Error");
        alert_box.frame(ui, |ui| {
            alert_box.body(ui, body);
        });
        if alert_box.was_outside_clicked() {
            alert_box.close();
        }
    });

    alert_box
}
