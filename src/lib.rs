#![feature(async_iterator)]
#![feature(sync_unsafe_cell)]
#![feature(let_chains)]
#![feature(btree_cursors)]

mod algorithm;
pub mod backtesting_market;
pub mod market;

#[cfg(test)]
mod tests;

pub use algorithm::Algorithm;

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn it_works() {
//         let result = add(2, 2);
//         assert_eq!(result, 4);
//     }
// }
