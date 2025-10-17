use std::ffi::{c_char, CStr};

use anyhow;
use chrono::DateTime;

use crate::market::{Event, Market, MarketTime};

pub struct CApiMarket<'a> {
    market: &'a mut dyn Market<Error = anyhow::Error>,
    error: Option<anyhow::Error>,
}

impl<'a> CApiMarket<'a> {
    pub fn new(market: &'a mut dyn Market<Error = anyhow::Error>) -> Self {
        Self {
            market: market,
            error: None,
        }
    }

    pub fn result(self) -> anyhow::Result<()> {
        match self.error {
            Some(e) => Err(e),
            None => Ok(()),
        }
    }
}

// TODO Consider using ms instead of seconds

#[no_mangle]
pub extern "C" fn mmatamm_next_event(
    market: &mut CApiMarket,
    event: &mut Event,
    time: &mut i64,
    event_written: &mut bool,
) -> bool {
    match market.market.next_event() {
        Ok(event_opt) => {
            if let Some((t, e)) = event_opt {
                *event = e;
                *time = t.timestamp();
                *event_written = true;
            } else {
                *event_written = false;
            }

            true
        }
        Err(err) => {
            market.error = Some(err);
            false
        }
    }
}

#[no_mangle]
pub extern "C" fn mmatamm_next_event_until(
    market: &mut CApiMarket,
    deadline: i64,
    event: &mut Event,
    time: &mut i64,
) -> bool {
    match market
        .market
        .next_event_until(DateTime::from_timestamp(deadline, 0).unwrap())
    {
        Ok((t, e)) => {
            *event = e;
            *time = t.timestamp();

            true
        }
        Err(err) => {
            market.error = Some(err);
            false
        }
    }
}

#[no_mangle]
pub extern "C" fn mmatamm_time(market: &mut CApiMarket) -> i64 {
    market.market.time().timestamp()
}

#[no_mangle]
pub unsafe extern "C" fn mmatamm_price_at(
    market: &mut CApiMarket,
    symbol: *const c_char,
    time: i64,
    price: &mut f32,
) -> bool {
    let symbol_str = CStr::from_ptr(symbol).to_str().unwrap();

    match market
        .market
        .price_at(symbol_str, DateTime::from_timestamp(time, 0).unwrap())
    {
        Ok(p) => {
            *price = p;

            true
        }
        Err(err) => {
            market.error = Some(err);
            false
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn mmatamm_current_price(
    market: &mut CApiMarket,
    symbol: *const c_char,
    price: &mut f32,
) -> bool {
    let symbol_str = CStr::from_ptr(symbol).to_str().unwrap();

    match market.market.current_price(symbol_str) {
        Ok(p) => {
            *price = p;

            true
        }
        Err(err) => {
            market.error = Some(err);
            false
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn mmatamm_buy_at_market(
    market: &mut CApiMarket,
    symbol: *const c_char,
    quantity: u32,
) -> bool {
    let symbol_str = CStr::from_ptr(symbol).to_str().unwrap();

    if let Err(err) = market.market.buy_at_market(symbol_str, quantity) {
        market.error = Some(err);
        false
    } else {
        true
    }
}

#[no_mangle]
pub unsafe extern "C" fn mmatamm_sell_at_market(
    market: &mut CApiMarket,
    symbol: *const c_char,
    quantity: u32,
) -> bool {
    let symbol_str = CStr::from_ptr(symbol).to_str().unwrap();

    match market.market.sell_at_market(symbol_str, quantity) {
        Ok(_) => true,
        Err(err) => {
            market.error = Some(err);
            false
        }
    }
}

#[no_mangle]
pub extern "C" fn mmatamm_market_time(market: &mut CApiMarket) -> MarketTime {
    market.market.market_time()
}

#[no_mangle]
pub extern "C" fn mmatamm_cash(market: &mut CApiMarket) -> f32 {
    market.market.cash()
}

#[no_mangle]
pub unsafe extern "C" fn mmatamm_shares_of(
    market: &mut CApiMarket,
    symbol: *const c_char,
    quantity: &mut u32,
) -> bool {
    let symbol_str = CStr::from_ptr(symbol).to_str().unwrap();

    *quantity = market.market.shares_of(symbol_str);

    true
}

// TODO Write interface to `holdings` (holdings_count and get_holding(i))

#[no_mangle]
pub extern "C" fn mmatamm_net_worth(market: &mut CApiMarket, worth: &mut f32) -> bool {
    match market.market.net_worth() {
        Ok(w) => {
            *worth = w;

            true
        }
        Err(err) => {
            market.error = Some(err);
            false
        }
    }
}
