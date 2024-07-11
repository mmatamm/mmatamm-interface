use std::future::Future;

use chrono::{DateTime, RoundingError, TimeDelta, Utc};

// TODO Add `SellCompleted` and `PurchaseCompleted` events
#[derive(Debug, PartialEq)]
pub enum Event {
    Tick,
    PreMarketStart,
    RegularMarketStart,
    RegularMarketEnd,
    PostMarketEnd,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MarketTime {
    NotTrading,
    PreMarket,
    Regular,
    PostMarket,
}

// TODO Use errors instead of panics
impl MarketTime {
    pub fn update(&mut self, event: &Event) {
        match event {
            Event::PreMarketStart => {
                assert!(self == &MarketTime::NotTrading);
                *self = MarketTime::PreMarket;
            }
            Event::RegularMarketStart => {
                assert!(self == &MarketTime::PreMarket);
                *self = MarketTime::Regular;
            }
            Event::RegularMarketEnd => {
                assert!(self == &MarketTime::Regular);
                *self = MarketTime::PostMarket;
            }
            Event::PostMarketEnd => {
                assert!(self == &MarketTime::PostMarket);
                *self = MarketTime::NotTrading;
            }
            _ => {}
        }
    }

    // TODO Consider implementing `is_trading`, `is_regular_market` and `is_extended_market`
}

pub trait Market {
    fn next_event(&mut self) -> impl Future<Output = Option<(DateTime<Utc>, Event)>> + Send;

    fn next_event_or_tick(
        &mut self,
        tick: TimeDelta,
    ) -> impl Future<Output = Result<Option<(DateTime<Utc>, Event)>, RoundingError>> + Send;

    fn time(&self) -> DateTime<Utc>;

    fn price_at(&self, symbol: &str, time: DateTime<Utc>) -> Option<f64>;

    fn current_price(&self, symbol: &str) -> Option<f64> {
        self.price_at(symbol, self.time())
    }

    fn buy_at_market(&mut self, symbol: &str, quantity: u32);
    fn sell_at_market(&mut self, symbol: &str, quantity: u32);

    fn market_time(&self) -> MarketTime;
}
