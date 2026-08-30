//! The server's copy of the board: where it is kept, who may write it, and
//! keeping the two in step with whoever is editing the file by hand.
//!
//! The whole module is server-only; `wall.rs` gates it, so nothing in here
//! repeats the feature.

use std::io;
use std::num::NonZeroU32;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, PoisonError, RwLock};
use std::time::SystemTime;

use governor::{DefaultDirectRateLimiter, DefaultKeyedRateLimiter, Quota, RateLimiter};

use super::Grid;

/// Cells one address may set a minute: a row's worth.
pub const BUDGET: u32 = 80;

/// Cells everyone together may write a minute.
///
/// The address comes from a header. Behind Cloudflare that is trustworthy, but
/// anything reaching the origin directly could claim a new one per request and
/// never meet a per-address limit. This is the ceiling that stops it.
pub const CEILING: u32 = 600;

/// How many addresses to hold before sweeping the spent ones out.
///
/// The key comes from a header, so whoever chooses it must not also choose how
/// much is remembered.
const TRACKED: usize = 4096;

/// A quota of `per_minute` cells. Zero would come out as one a minute, but
/// neither of the two above is zero.
fn quota(per_minute: u32) -> Quota {
    Quota::per_minute(NonZeroU32::new(per_minute).unwrap_or(NonZeroU32::MIN))
}

/// The board, who has been writing to it, and where it is kept.
pub struct State {
    grid: RwLock<Grid>,
    /// What one address may write, and what everyone together may.
    mine: DefaultKeyedRateLimiter<String>,
    everyone: DefaultDirectRateLimiter,
    path: PathBuf,
    /// When the file last matched this. A newer one means somebody edited it,
    /// so it is read back before the next answer.
    synced: Mutex<Option<SystemTime>>,
    /// Held for a whole write, so only one is in flight at a time. See
    /// [`State::write`].
    writing: tokio::sync::Mutex<()>,
}

impl State {
    /// Loads the board, starting blank if the file is not there yet.
    #[must_use]
    pub fn load(path: PathBuf) -> Self {
        let grid =
            std::fs::read_to_string(&path).map_or_else(|_| Grid::blank(), |t| Grid::parse(&t));
        let state = Self {
            grid: RwLock::new(grid),
            mine: RateLimiter::keyed(quota(BUDGET)),
            everyone: RateLimiter::direct(quota(CEILING)),
            synced: Mutex::new(None),
            writing: tokio::sync::Mutex::new(()),
            path,
        };
        state.saved();
        state
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The file's modification time, or `None` if it cannot be read.
    fn touched(&self) -> Option<SystemTime> {
        std::fs::metadata(&self.path)
            .and_then(|meta| meta.modified())
            .ok()
    }

    /// Says the file now matches this. Called after writing it, or the next
    /// read sees a newer file and loads back what it just wrote.
    fn saved(&self) {
        *self.synced.lock().unwrap_or_else(PoisonError::into_inner) = self.touched();
    }

    /// Reads the file back if it has changed since this last wrote it.
    ///
    /// Editing it is how the board is moderated, so an edit has to land without
    /// a restart. Checked on read and on write rather than watched, so nothing
    /// has to be running in between.
    ///
    /// The reads are blocking, on a runtime that would rather they were not.
    /// It is a stat of one small local file per request, on a route nobody hits
    /// more than a few times a minute, so moving it off the runtime would cost
    /// more than it saves.
    fn refresh(&self) {
        let Some(touched) = self.touched() else {
            return;
        };
        let mut synced = self.synced.lock().unwrap_or_else(PoisonError::into_inner);
        if *synced == Some(touched) {
            return;
        }

        if let Ok(text) = std::fs::read_to_string(&self.path) {
            *self.grid.write().unwrap_or_else(PoisonError::into_inner) = Grid::parse(&text);
        }
        *synced = Some(touched);
    }

    /// The board as it stands, for anyone reading it.
    #[must_use]
    pub fn render(&self) -> String {
        self.refresh();
        // A poisoned lock still holds a perfectly good grid: whatever panicked
        // did so elsewhere, and losing the board over it would be worse.
        self.grid
            .read()
            .unwrap_or_else(PoisonError::into_inner)
            .render()
    }

    /// Whether `who` may write one more cell, counting it if so.
    ///
    /// Their own budget first, then everyone's. A refused check costs nothing,
    /// so that order leaves the ceiling for people who have not used their own
    /// budget up, rather than letting one address spend everyone's on writes
    /// that were going to be refused anyway.
    ///
    /// Asking about an address is what starts remembering it, so the table is
    /// swept rather than held down by the ceiling.
    pub fn allowed(&self, who: &str) -> bool {
        // Only once it has grown, and it only drops addresses whose budget has
        // fully refilled, so this cannot forget anyone still being limited.
        if self.mine.len() >= TRACKED {
            self.mine.retain_recent();
            self.mine.shrink_to_fit();
        }

        self.mine.check_key(&who.to_owned()).is_ok() && self.everyone.check().is_ok()
    }

    /// Sets one cell. Returns the board to write out when it changed, and
    /// `None` when nothing did.
    fn set(&self, x: usize, y: usize, byte: u8) -> Option<String> {
        // So the write lands on what the file says now, not on a copy from
        // before somebody edited it.
        self.refresh();
        let mut grid = self.grid.write().unwrap_or_else(PoisonError::into_inner);
        grid.set(x, y, byte).then(|| grid.render())
    }

    /// Sets one cell and writes the board out. Does nothing at all if the cell
    /// already held that character, so a repeat costs no disk.
    ///
    /// One writer at a time, because setting the cell and writing the file have
    /// to stay one step. Letting them race loses writes outright: each writer
    /// renders a board, lets the lock go, and the next one to arrive reads the
    /// file back over memory before setting its own cell, so whatever landed in
    /// between is gone. Sixteen writers filling the board at once kept about a
    /// quarter of what they wrote.
    ///
    /// # Errors
    ///
    /// Whatever stopped the write. The cell is set either way, so the board is
    /// right until a restart, which is more use than refusing the write.
    pub async fn write(&self, x: usize, y: usize, byte: u8) -> io::Result<()> {
        let _writing = self.writing.lock().await;

        let Some(text) = self.set(x, y, byte) else {
            return Ok(());
        };
        let written = tokio::fs::write(&self.path, &text).await;
        // Even when the write failed, so a half-written file is not read back
        // over a board that is still right. A later edit has a newer time
        // again, so moderating still works.
        self.saved();
        written
    }

    /// Writes the board out, creating the file if it is not there.
    ///
    /// Called once at startup, so a board that cannot be saved is an error in
    /// the journal rather than a surprise at the first write.
    ///
    /// # Errors
    ///
    /// Whatever stopped the write: usually the directory not being there, or
    /// the service not being allowed to write it.
    pub fn persist(&self) -> io::Result<()> {
        let written = std::fs::write(&self.path, self.render());
        self.saved();
        written
    }
}
