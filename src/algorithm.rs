use std::future::Future;

use chrono::NaiveTime;

use crate::market::Market;

pub trait Algorithm {
    fn wake_ups() -> impl Iterator<Item = NaiveTime>;

    fn run<M: Market>(&mut self, market: &mut M) -> impl Future<Output = Result<(), M::Error>>;
}
