use chrono::{DateTime, Utc};
use thiserror::Error;

#[derive(Clone, Debug, PartialEq)]
#[repr(C)]
pub enum SystemEvent {
    PreMarketStart,
    RegularMarketStart,
    RegularMarketEnd,
    PostMarketEnd,
}

// TODO Add `SellCompleted` and `PurchaseCompleted` events
#[derive(Clone, Debug, PartialEq)]
#[repr(C)]
pub enum Event {
    Deadline,
    SystemEvent(SystemEvent),
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[repr(C)]
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
        event: SystemEvent,
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
    pub fn update(&mut self, event: &SystemEvent) -> Result<(), ImpossibleEvent> {
        match event {
            SystemEvent::PreMarketStart => {
                update_market_time!(self, event, MarketTime::NotTrading, MarketTime::PreMarket)
            }
            SystemEvent::RegularMarketStart => {
                update_market_time!(self, event, MarketTime::PreMarket, MarketTime::Regular)
            }
            SystemEvent::RegularMarketEnd => {
                update_market_time!(self, event, MarketTime::Regular, MarketTime::PostMarket)
            }
            SystemEvent::PostMarketEnd => {
                update_market_time!(self, event, MarketTime::PostMarket, MarketTime::NotTrading)
            }
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
    type Error: Into<anyhow::Error>;

    fn next_event(&mut self) -> Result<Option<(DateTime<Utc>, Event)>, Self::Error>;

    fn next_event_until(
        &mut self,
        deadline: DateTime<Utc>,
    ) -> Result<(DateTime<Utc>, Event), Self::Error>;

    fn time(&self) -> DateTime<Utc>;

    fn price_at(&self, symbol: &str, time: DateTime<Utc>) -> Result<f32, Self::Error>;

    fn current_price(&self, symbol: &str) -> Result<f32, Self::Error> {
        self.price_at(symbol, self.time())
    }

    fn buy_at_market(&mut self, symbol: &str, quantity: u32) -> Result<(), Self::Error>;
    fn sell_at_market(&mut self, symbol: &str, quantity: u32) -> Result<(), Self::Error>;

    fn market_time(&self) -> MarketTime;

    fn cash(&self) -> f32;

    fn shares_of(&self, symbol: &str) -> u32;

    fn holdings(&self) -> Vec<(&String, &u32)>;

    fn net_worth(&self) -> Result<f32, Self::Error> {
        let gross_holdings_worth: f32 = self
            .holdings()
            .into_iter()
            .map(|(symbol, quantity)| Ok(self.current_price(symbol)? * (*quantity as f32)))
            .try_fold(0.0, |acc: f32, p| Ok(acc + p?))?;

        Ok(gross_holdings_worth + self.cash())
    }
}
