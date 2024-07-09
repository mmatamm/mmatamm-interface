use std::future::Future;

use chrono::{DateTime, RoundingError, TimeDelta, Utc};

use crate::Event;

pub trait Market {
    fn next_event(&mut self) -> impl Future<Output = Option<(DateTime<Utc>, Event)>> + Send;

    fn next_event_or_tick(
        &mut self,
        tick: TimeDelta,
    ) -> impl Future<Output = Result<Option<(DateTime<Utc>, Event)>, RoundingError>> + Send;

    fn price_at(&self, symbol: &str, time: DateTime<Utc>) -> Option<f64>;

    fn buy_at_market(&self, symbol: &str, quantity: u32);
    fn sell_at_market(&self, symbol: &str, quantity: u32);

    fn in_regular_hours(&self) -> bool;
    fn in_pre_market_hours(&self) -> bool;
    fn in_post_market_hours(&self) -> bool;
    fn in_extended_hours(&self) -> bool {
        self.in_pre_market_hours() || self.in_post_market_hours()
    }
}
