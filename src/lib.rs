#![feature(async_iterator)]

mod algorithm;
pub mod market;
pub mod questdb_market;

#[cfg(test)]
mod tests;

pub use algorithm::Algorithm;

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn it_works() {
//         let result = add(2, 2);
//         assert_eq!(result, 4);
//     }
// }
