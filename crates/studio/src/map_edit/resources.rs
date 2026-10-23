//! The [`Env`] of a games dir: its stamps and the catalog's usual collisions.

use super::behavior::usual_collisions;
use super::catalog::Catalog;
use super::games::GamesDir;
use super::model::{Doc, Env};
use super::stamp::Stamp;
use anyhow::Result;
use std::sync::Arc;

#[derive(Debug)]
pub struct Resources {
    pub games: GamesDir,
    pub catalog: Catalog,
}

impl Resources {
    pub fn new(games: GamesDir, catalog: Catalog) -> Arc<Resources> {
        Arc::new(Resources { games, catalog })
    }
}

impl Env for Resources {
    fn stamp(&self, name: &str) -> Result<Stamp> {
        self.games.load_stamp(name)
    }

    fn usual_collisions(&self, d: &Doc) -> Vec<Option<u8>> {
        usual_collisions(&self.catalog, d)
            .into_iter()
            .map(|u| u.map(|u| u.0))
            .collect()
    }
}
