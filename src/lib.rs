// TODO some are not necessary anymore
#![feature(async_iterator)]
#![feature(sync_unsafe_cell)]
#![feature(let_chains)]
#![feature(btree_cursors)]

mod algorithm;
pub mod market;

#[cfg(test)]
mod tests;

pub use algorithm::Algorithm;
