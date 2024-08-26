use std::future::Future;

use chrono::{DateTime, TimeDelta, Utc};
use futures::future::try_join_all;
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
    Unknown,
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
        if $self == &$current_state || $self == &MarketTime::Unknown {
            *$self = $next_state;
            Ok(())
        } else {
            Err(ImpossibleEvent::MarketTimeSkip {
                event: $event.clone(),
                market_time: $self.clone(),
            })
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

    /// Determines if the market is currently open.
    ///
    /// # Returns
    ///
    /// * `true` if the market is open (Pre-Market, Regular, or Post-Market)
    /// * `false` if the market is closed (any other state)
    pub fn is_open(&self) -> bool {
        self == &MarketTime::PreMarket
            || self == &MarketTime::Regular
            || self == &MarketTime::PostMarket
    }
}

pub trait Market: Sync {
    type Error: Send;

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
    ) -> impl Future<Output = Result<f64, Self::Error>> + Send;

    fn current_price(&self, symbol: &str) -> impl Future<Output = Result<f64, Self::Error>> + Send {
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

    fn cash(&self) -> f64;

    fn shares_of(&self, symbol: &str) -> u32;

    fn holdings(&self) -> impl IntoIterator<Item = (&String, &u32)>;

    fn net_worth(&self) -> impl std::future::Future<Output = Result<f64, Self::Error>> + Send {
        async {
            let individual_holding_worth =
                try_join_all(self.holdings().into_iter().map(|(symbol, quantity)| async {
                    Ok(self.current_price(symbol).await? * (*quantity as f64))
                }))
                .await?;
            let gross_holdings_worth: f64 = individual_holding_worth.iter().sum();

            Ok(gross_holdings_worth + self.cash())
        }
    }
}
