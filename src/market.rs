use std::future::Future;

use chrono::{DateTime, RoundingError, TimeDelta, Utc};
use thiserror::Error;

// TODO Add `SellCompleted` and `PurchaseCompleted` events
#[derive(Clone, Debug, PartialEq)]
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

#[derive(Error, Debug)]
pub enum ImpossibleEvent {
    #[error("{event:?} reported during {market_time:?} market time")]
    MarketTimeSkip {
        event: Event,
        market_time: MarketTime,
    },
}

macro_rules! update_market_time {
    ($self:ident, $event:ident, $current_state:expr, $next_state:expr) => {
        if $self != &$current_state {
            Err(ImpossibleEvent::MarketTimeSkip {
                event: $event.clone(),
                market_time: $self.clone(),
            })
        } else {
            *$self = $next_state;
            Ok(())
        }
    };
}

impl MarketTime {
    pub fn update(&mut self, event: &Event) -> Result<(), ImpossibleEvent> {
        match event {
            Event::PreMarketStart => {
                update_market_time!(self, event, MarketTime::NotTrading, MarketTime::PreMarket)
            }
            Event::RegularMarketStart => {
                update_market_time!(self, event, MarketTime::PreMarket, MarketTime::Regular)
            }
            Event::RegularMarketEnd => {
                update_market_time!(self, event, MarketTime::Regular, MarketTime::PostMarket)
            }
            Event::PostMarketEnd => {
                update_market_time!(self, event, MarketTime::PostMarket, MarketTime::NotTrading)
            }
            _ => Ok(()),
        }
    }

    // TODO Consider implementing `is_trading`, `is_regular_market` and `is_extended_market`
}

pub trait Market {
    type Error;

    fn next_event(
        &mut self,
    ) -> impl Future<Output = Result<Option<(DateTime<Utc>, Event)>, Self::Error>> + Send;

    fn next_event_or_tick(
        &mut self,
        tick: TimeDelta,
    ) -> impl Future<Output = Result<(DateTime<Utc>, Event), Self::Error>> + Send;

    fn time(&self) -> DateTime<Utc>;

    fn price_at(
        &self,
        symbol: &str,
        time: DateTime<Utc>,
    ) -> impl Future<Output = Result<Option<f64>, Self::Error>>;

    fn current_price(
        &self,
        symbol: &str,
    ) -> impl Future<Output = Result<Option<f64>, Self::Error>> {
        self.price_at(symbol, self.time())
    }

    fn buy_at_market(
        &mut self,
        symbol: &str,
        quantity: u32,
    ) -> impl Future<Output = Result<(), Self::Error>>;
    fn sell_at_market(
        &mut self,
        symbol: &str,
        quantity: u32,
    ) -> impl Future<Output = Result<(), Self::Error>>;

    fn market_time(&self) -> MarketTime;
}
