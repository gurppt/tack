//! Named toolbar content only; layout and active tools belong to their existing owners.
use crate::{preferences::Preferences, toolbar::Config};
use serde::{Deserialize, Serialize};
use tack_assets::AssetError;
pub const MAX_SETS: usize = 16;
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Set {
    pub name: String,
    pub actions: Vec<String>,
}
fn name_valid(name: &str) -> bool {
    !name.trim().is_empty() && name.len() <= 128 && !name.chars().any(char::is_control)
}
impl Preferences {
    pub fn normalize_sets(&mut self) -> Result<(), AssetError> {
        self.toolbar.normalize()?;
        if self.toolbar_sets.is_empty() {
            self.toolbar_sets.push(Set {
                name: "DEFAULT".into(),
                actions: self.toolbar.actions.clone(),
            });
            self.active_toolbar_set = 0;
        }
        if self.toolbar_sets.len() > MAX_SETS || self.active_toolbar_set >= self.toolbar_sets.len()
        {
            return Err("Toolbar set count or active index out of bounds".into());
        }
        let mut names = std::collections::BTreeSet::new();
        for set in &mut self.toolbar_sets {
            set.name = set.name.trim().into();
            if !name_valid(&set.name) || !names.insert(set.name.to_lowercase()) {
                return Err("Toolbar sets require unique names (1..128 bytes)".into());
            }
            let mut config = Config {
                actions: set.actions.clone(),
                ..Config::default()
            };
            config.normalize()?;
            set.actions = config.actions;
        }
        self.sync_active_set();
        Ok(())
    }
    pub fn sync_active_set(&mut self) {
        if let Some(set) = self.toolbar_sets.get_mut(self.active_toolbar_set) {
            set.actions = self.toolbar.actions.clone();
        }
    }
    pub fn toolbar_set_name(&self) -> &str {
        self.toolbar_sets
            .get(self.active_toolbar_set)
            .map_or("DEFAULT", |s| s.name.as_str())
    }
    pub fn cycle_toolbar_set(&mut self, next: bool) -> Result<(), AssetError> {
        self.normalize_sets()?;
        self.active_toolbar_set = (self.active_toolbar_set
            + if next { 1 } else { self.toolbar_sets.len() - 1 })
            % self.toolbar_sets.len();
        self.toolbar.actions = self.toolbar_sets[self.active_toolbar_set].actions.clone();
        Ok(())
    }
    pub fn new_toolbar_set(&mut self, duplicate: bool) -> Result<(), AssetError> {
        self.normalize_sets()?;
        if self.toolbar_sets.len() == MAX_SETS {
            return Err("Maximum 16 toolbar sets".into());
        }
        let name = (1..=MAX_SETS + 1)
            .map(|i| format!("SET {i}"))
            .find(|n| {
                !self
                    .toolbar_sets
                    .iter()
                    .any(|s| s.name.eq_ignore_ascii_case(n))
            })
            .ok_or("Set name unavailable")?;
        let actions = if duplicate {
            self.toolbar.actions.clone()
        } else {
            Vec::new()
        };
        self.toolbar_sets.push(Set {
            name,
            actions: actions.clone(),
        });
        self.active_toolbar_set = self.toolbar_sets.len() - 1;
        self.toolbar.actions = actions;
        Ok(())
    }
    pub fn rename_toolbar_set(&mut self, name: &str) -> Result<(), AssetError> {
        self.normalize_sets()?;
        if !name_valid(name)
            || self.toolbar_sets.iter().enumerate().any(|(i, s)| {
                i != self.active_toolbar_set && s.name.to_lowercase() == name.trim().to_lowercase()
            })
        {
            return Err("Choose a unique non-empty set name (128 bytes maximum)".into());
        }
        self.toolbar_sets[self.active_toolbar_set].name = name.trim().into();
        Ok(())
    }
    pub fn delete_toolbar_set(&mut self) -> Result<(), AssetError> {
        self.normalize_sets()?;
        if self.toolbar_sets.len() == 1 {
            return Err("Keep at least one toolbar set".into());
        }
        self.toolbar_sets.remove(self.active_toolbar_set);
        self.active_toolbar_set = self.active_toolbar_set.min(self.toolbar_sets.len() - 1);
        self.toolbar.actions = self.toolbar_sets[self.active_toolbar_set].actions.clone();
        Ok(())
    }
}
