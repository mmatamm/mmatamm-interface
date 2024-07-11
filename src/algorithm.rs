use chrono::NaiveTime;

use crate::market::Market;

pub trait Algorithm {
    fn wake_ups() -> impl Iterator<Item = NaiveTime>;

    fn run<M: Market>(market: &mut M);
}
