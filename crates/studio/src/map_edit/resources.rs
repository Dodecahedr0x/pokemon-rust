//! The [`Env`] of a games dir: its stamps and the catalog's usual collisions.

use super::behavior::{cell_collision, warp_cells};
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

    fn usual_collision(&self, d: &Doc, x: i32, y: i32) -> Option<u8> {
        cell_collision(&self.catalog, d, &warp_cells(d), x, y).map(|u| u.0)
    }
}
