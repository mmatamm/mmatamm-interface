use chrono::{DateTime, Utc};

use crate::market::{Event, Market, MarketTime};

#[inline]
fn convert_result_to_anyhow<T, E>(result: Result<T, E>) -> Result<T, anyhow::Error>
where
    E: Into<anyhow::Error>,
{
    result.map_err(|e| e.into())
}

pub struct AnyhowMarket<'a, M>(&'a mut M)
where
    M: Market,
    M::Error: Into<anyhow::Error>;

impl<'a, M> AnyhowMarket<'a, M>
where
    M: Market,
    M::Error: Into<anyhow::Error>,
{
    pub fn new(market: &'a mut M) -> Self {
        Self(market)
    }
}

impl<M> Market for AnyhowMarket<'_, M>
where
    M: Market,
    M::Error: Into<anyhow::Error>,
{
    type Error = anyhow::Error;

    fn next_event(&mut self) -> anyhow::Result<Option<(DateTime<Utc>, Event)>> {
        convert_result_to_anyhow(self.0.next_event())
    }

    fn next_event_until(
        &mut self,
        deadline: DateTime<Utc>,
    ) -> anyhow::Result<(DateTime<Utc>, Event)> {
        convert_result_to_anyhow(self.0.next_event_until(deadline))
    }

    fn price_at(&self, symbol: &str, time: DateTime<Utc>) -> anyhow::Result<f32> {
        convert_result_to_anyhow(self.0.price_at(symbol, time))
    }

    fn buy_at_market(&mut self, symbol: &str, quantity: u32) -> anyhow::Result<()> {
        convert_result_to_anyhow(self.0.buy_at_market(symbol, quantity))
    }

    fn sell_at_market(&mut self, symbol: &str, quantity: u32) -> anyhow::Result<()> {
        convert_result_to_anyhow(self.0.sell_at_market(symbol, quantity))
    }

    fn current_price(&self, symbol: &str) -> anyhow::Result<f32> {
        convert_result_to_anyhow(self.0.current_price(symbol))
    }

    fn net_worth(&self) -> anyhow::Result<f32> {
        convert_result_to_anyhow(self.0.net_worth())
    }

    fn time(&self) -> DateTime<Utc> {
        self.0.time()
    }

    fn market_time(&self) -> MarketTime {
        self.0.market_time()
    }

    fn cash(&self) -> f32 {
        self.0.cash()
    }

    fn shares_of(&self, symbol: &str) -> u32 {
        self.0.shares_of(symbol)
    }

    fn holdings(&self) -> Vec<(&String, &u32)> {
        self.0.holdings()
    }
}
