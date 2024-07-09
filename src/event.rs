#[derive(Debug, PartialEq)]
pub enum Event {
    Tick,
    PreMarketStart,
    RegularMarketStart,
    RegularMarketEnd,
    PostMarketEnd,
}
