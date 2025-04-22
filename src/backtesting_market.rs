pub mod fetcher;

use std::collections::{HashMap, LinkedList};
use std::error::Error as StdError;
use std::fmt::Display;

use chrono::{DateTime, Utc};
use fetcher::Fetcher;
use thiserror::Error;
use tokio::sync::RwLock;

use crate::market::{Event, ImpossibleEvent, Market, MarketTime};

pub struct BacktestingMarket<'a, F: Fetcher> {
    // /// A database client TODO better comment needed
    fetcher: &'a RwLock<F>,

    /// The current virtual time
    time: DateTime<Utc>,
    /// The current market time (e.g. pre-market, regular hours, etc...)
    market_time: MarketTime,
    /// All the following events. This does not include system events and ticks.
    events: LinkedList<(DateTime<Utc>, Event)>,

    // TODO seperate `cash` to `available_cash` and `locked_cash` (or some other name). =
    // available_cash will be subtracted from when submitting an order, and added to
    // locked_cash. Upon trade complete, this will be updated.
    /// The amount of cash on hand
    cash: f64,
    /// How many shares of each equity are owned, by symbol
    holdings: HashMap<String, u32>,
}

impl<'a, F: Fetcher> BacktestingMarket<'a, F> {
    pub async fn new(
        fetcher: &'a RwLock<F>,
        start: DateTime<Utc>,
        cash: f64,
    ) -> Result<Self, Error<F>> {
        Ok(BacktestingMarket {
            fetcher,

            time: start,
            market_time: MarketTime::Unknown,
            events: LinkedList::new(),

            cash,
            holdings: HashMap::new(),
        })
    }

    async fn next_system_event(&self) -> Result<Option<(DateTime<Utc>, Event)>, Error<F>> {
        let mut fetcher = self.fetcher.write().await;
        let event = match fetcher.query_system_event(&self.time).await {
            Ok(it) => it,
            Err(err) => return Err(FetcherError(err).into()),
        };

        Ok(event
            .map(|(event_type, timestamp)| (timestamp.and_utc(), Event::SystemEvent(event_type))))
    }

    async fn peek_next_event(&self) -> Result<Option<(DateTime<Utc>, Event)>, Error<F>> {
        let next_system_event = self.next_system_event().await?;
        let next_internal_event = self.events.front();

        match (next_system_event, next_internal_event) {
            (Some(next_sys), Some(next_int)) => {
                if next_sys.0 >= next_int.0 {
                    Ok(Some(next_int.clone()))
                } else {
                    Ok(Some(next_sys))
                }
            }
            (Some(next_sys), None) => Ok(Some(next_sys)),
            (None, Some(next_int)) => Ok(Some(next_int.clone())),
            (None, None) => Ok(None),
        }
    }
}

impl<F: Fetcher + Send + Sync + std::fmt::Debug + 'static> Market for BacktestingMarket<'_, F> {
    type Error = Error<F>;

    async fn next_event(&mut self) -> Result<Option<(DateTime<Utc>, Event)>, Self::Error> {
        match self.peek_next_event().await? {
            Some((time, event)) => {
                self.time = time;

                if let Event::SystemEvent(ref system_event) = event {
                    self.market_time.update(&system_event)?;
                }

                // TODO if the event is internal, pop it from the linked list

                Ok(Some((time, event)))
            }
            None => Ok(None),
        }
    }

    async fn next_event_until(
        &mut self,
        deadline: DateTime<Utc>,
    ) -> Result<(DateTime<Utc>, Event), Self::Error> {
        // // NOTE This duration_trunc takes about 13% of the time of this entire
        // function let next_tick = self.time.duration_trunc(tick).unwrap() +
        // tick;

        let event = if let Some((time, event)) = self.peek_next_event().await? {
            if time <= deadline {
                if let Event::SystemEvent(ref system_event) = event {
                    self.market_time.update(&system_event)?;
                }

                // TODO if the event is internal, pop it from the linked list
                (time, event)
            } else {
                (deadline, Event::Deadline)
            }
        } else {
            (deadline, Event::Deadline)
        };

        self.time = event.0;

        Ok(event)
    }

    fn time(&self) -> DateTime<Utc> {
        self.time
    }

    async fn price_at(&self, symbol: &str, time: DateTime<Utc>) -> Result<f64, Self::Error> {
        // TODO Remember the random value for a stock and deviate from it using
        // geometric Brownian motion (or some estimation of it). Assume the
        // price is in the middle of the bid/ask spread
        // TODO Verify the timestamps
        // TODO Implement speculative pre-fetching
        // TODO Avoid querying future prices
        // TODO Consider introducing a 15-minutes delay

        if time > self.time {
            return Err(Error::FutureQuery {
                future_time: time,
                current_time: self.time,
            });
        }

        // Return the last close price
        let query_price = match self.fetcher.write().await.query_price(&time, &symbol).await {
            Ok(it) => it,
            Err(err) => return Err(FetcherError(err).into()),
        };

        Ok(query_price.ok_or(Error::UnknownPrice(symbol.to_string()))?)
    }

    async fn buy_at_market(&mut self, symbol: &str, quantity: u32) -> Result<(), Self::Error> {
        // Ensure the market is open
        if !self.market_time.is_open() {
            return Err(Error::UntimelyTrade(symbol.to_string(), self.time));
        }

        if quantity == 0 {
            return Ok(());
        }

        // Calculate the transaction's cost
        // TODO include fees, bid and ask too
        let price_per_share = self.current_price(symbol).await?;
        let total_price = price_per_share * quantity as f64;

        // Ensure the cash is sufficient for it
        if total_price > self.cash {
            return Err(Error::InsufficientCash {
                quantity,
                symbol: symbol.to_string(),
                total_price,
                cash: self.cash,
            });
        }

        // Update the cash and the holdings
        self.cash -= total_price;

        if let Some(v) = self.holdings.get_mut(symbol) {
            *v += quantity;
        } else {
            self.holdings.insert(symbol.to_string(), quantity);
        }

        // TODO Add an event of PurchaseComplete
        // TODO The transaction might be canceled if it's at the end of the
        // day and there are no buyers/sellers

        Ok(())
    }

    async fn sell_at_market(&mut self, symbol: &str, quantity: u32) -> Result<(), Self::Error> {
        // Ensure the market is open
        if !self.market_time.is_open() {
            return Err(Error::UntimelyTrade(symbol.to_string(), self.time));
        }

        if quantity == 0 {
            return Ok(());
        }

        // Calculate the transaction's cost
        // TODO include fees, bid and ask too
        let price_per_share = self.current_price(symbol).await?;
        let total_price = price_per_share * quantity as f64;

        // Ensure there are enough shares of this stock
        let owned_shares_opt = self.holdings.get_mut(symbol);
        if owned_shares_opt.is_none() {
            return Err(Error::InsufficientShares {
                quantity,
                symbol: symbol.to_string(),
                owned: 0,
            });
        }

        if &quantity > owned_shares_opt.as_ref().unwrap() {
            return Err(Error::InsufficientShares {
                quantity,
                symbol: symbol.to_string(),
                owned: *owned_shares_opt.unwrap(),
            });
        }

        // Update the cash and the holdings
        self.cash += total_price;

        if let Some(v) = self.holdings.get_mut(symbol) {
            *v -= quantity
        } else {
            unreachable!()
        }

        // TODO Add an event of SellComplete
        // TODO The transaction might be canceled if it's at the end of the
        // day and there are no buyers/sellers

        Ok(())
    }

    fn market_time(&self) -> crate::market::MarketTime {
        self.market_time
    }

    fn cash(&self) -> f64 {
        self.cash
    }

    fn shares_of(&self, symbol: &str) -> u32 {
        if let Some(q) = self.holdings.get(symbol) {
            *q
        } else {
            0
        }
    }

    fn holdings(&self) -> impl IntoIterator<Item = (&String, &u32)> {
        &self.holdings
    }
}

/// To avoid conflicting From implementations, contain F::Error in a distinct
/// type
#[derive(Debug)]
pub struct FetcherError<F: Fetcher>(F::Error);

impl<F: Fetcher> Display for FetcherError<F> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl<F: Fetcher + std::fmt::Debug> StdError for FetcherError<F> {}

#[derive(Error, Debug)]
pub enum Error<F: Fetcher> {
    #[error("Fetching error")]
    DatabaseError(#[from] FetcherError<F>),

    #[error("Attempted to trade {0} at {1}, outside of trading hours")]
    UntimelyTrade(String, DateTime<Utc>),

    #[error("Attempted to trade {0} yet the price is unknown")]
    UnknownPrice(String),

    #[error("Cannot buy {quantity} shares of {symbol} for {total_price} with {cash} in cash")]
    InsufficientCash {
        quantity: u32,
        symbol: String,
        total_price: f64,
        cash: f64,
    },

    #[error("Cannot sell {quantity} shares of {symbol} because only {owned} shares are owned")]
    InsufficientShares {
        quantity: u32,
        symbol: String,
        owned: u32,
    },

    #[error(
        "Symbol '{symbol}' found in database, which is not of the expected kind, {expected_kind}"
    )]
    UnexpectedDatabaseSymbol {
        symbol: String,
        expected_kind: String,
    },

    #[error("Impossible event")]
    ImpossibleEvent(#[from] ImpossibleEvent),

    #[error("Tried to query data from {future_time} at {current_time}")]
    FutureQuery {
        future_time: DateTime<Utc>,
        current_time: DateTime<Utc>,
    },
}
