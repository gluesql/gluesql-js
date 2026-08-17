#![deny(clippy::str_to_string)]

mod file;
mod page;
mod pager;
mod record;
mod store;
mod wal;

pub use {
    file::{RandomAccessFile, SyncFile},
    page::{PAGE_SIZE, PageId},
    pager::Pager,
    store::OpfsStorage,
};
