//! Deterministic, standard-library-first market research primitives.
use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    fmt::{Display, Formatter},
    fs,
    path::Path,
};
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const MAGIC: &[u8; 8] = b"TACHYON\0";
pub const FORMAT_VERSION: u16 = 1;
pub const MAX_RECORDS: usize = 1_000_000;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error(pub String);
impl Display for Error {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for Error {}
pub type Result<T> = std::result::Result<T, Error>;
macro_rules! unit{($($n:ident),+)=>{$(#[derive(Debug,Clone,Copy,Default,PartialEq,Eq,PartialOrd,Ord,Hash)]pub struct $n(pub u64);)+}}
unit!(Price, Quantity, OrderId, AccountId, Sequence);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Side {
    Bid,
    Ask,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Down = -1,
    Flat = 0,
    Up = 1,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    pub id: OrderId,
    pub account: AccountId,
    pub side: Side,
    pub price: Price,
    pub quantity: Quantity,
    pub sequence: Sequence,
}
impl Order {
    pub fn new(
        id: u64,
        account: u64,
        side: Side,
        price: u64,
        quantity: u64,
        sequence: u64,
    ) -> Result<Self> {
        if quantity == 0 {
            return Err(Error("order quantity must be positive lots".into()));
        }
        Ok(Self {
            id: OrderId(id),
            account: AccountId(account),
            side,
            price: Price(price),
            quantity: Quantity(quantity),
            sequence: Sequence(sequence),
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarketEvent {
    Add(Order),
    Cancel {
        sequence: Sequence,
        order_id: OrderId,
    },
    Replace {
        sequence: Sequence,
        order_id: OrderId,
        new_price: Price,
        new_quantity: Quantity,
    },
    Trade {
        sequence: Sequence,
        price: Price,
        quantity: Quantity,
        aggressor: Side,
    },
    BookChange {
        sequence: Sequence,
    },
    Fill {
        sequence: Sequence,
        order_id: OrderId,
        price: Price,
        quantity: Quantity,
    },
    Information {
        sequence: Sequence,
        code: u32,
        value: i64,
    },
    Settlement {
        sequence: Sequence,
        instrument: u64,
        probability_bps: u16,
    },
}
impl MarketEvent {
    pub fn sequence(&self) -> Sequence {
        match self {
            Self::Add(o) => o.sequence,
            Self::Cancel { sequence, .. }
            | Self::Replace { sequence, .. }
            | Self::Trade { sequence, .. }
            | Self::BookChange { sequence }
            | Self::Fill { sequence, .. }
            | Self::Information { sequence, .. }
            | Self::Settlement { sequence, .. } => *sequence,
        }
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EventLog {
    events: Vec<MarketEvent>,
}
impl EventLog {
    pub fn append(&mut self, e: MarketEvent) -> Result<()> {
        if self
            .events
            .last()
            .is_some_and(|x| x.sequence() >= e.sequence())
        {
            return Err(Error("event sequence must increase strictly".into()));
        }
        self.events.push(e);
        Ok(())
    }
    pub fn events(&self) -> &[MarketEvent] {
        &self.events
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PriceLevel {
    pub total_quantity: u64,
    pub orders: VecDeque<Order>,
}
impl PriceLevel {
    fn push(&mut self, o: Order) -> Result<()> {
        self.total_quantity = self
            .total_quantity
            .checked_add(o.quantity.0)
            .ok_or_else(|| Error("level quantity overflow".into()))?;
        self.orders.push_back(o);
        Ok(())
    }
    fn remove(&mut self, id: OrderId) -> Result<Order> {
        let i = self
            .orders
            .iter()
            .position(|o| o.id == id)
            .ok_or_else(|| Error("indexed order missing".into()))?;
        let o = self.orders.remove(i).unwrap();
        self.total_quantity -= o.quantity.0;
        Ok(o)
    }
    fn valid(&self) -> bool {
        !self.orders.is_empty()
            && self.orders.iter().map(|o| o.quantity.0).sum::<u64>() == self.total_quantity
            && self.orders.iter().all(|o| o.quantity.0 > 0)
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fill {
    pub maker_order: OrderId,
    pub buyer: AccountId,
    pub seller: AccountId,
    pub price: Price,
    pub quantity: Quantity,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OrderBook {
    pub bids: BTreeMap<Price, PriceLevel>,
    pub asks: BTreeMap<Price, PriceLevel>,
    index: HashMap<OrderId, (Side, Price)>,
}
impl OrderBook {
    pub fn best_bid(&self) -> Option<Price> {
        self.bids.last_key_value().map(|(p, _)| *p)
    }
    pub fn best_ask(&self) -> Option<Price> {
        self.asks.first_key_value().map(|(p, _)| *p)
    }
    pub fn len(&self) -> usize {
        self.index.len()
    }
    pub fn is_empty(&self) -> bool {
        self.index.is_empty()
    }
    pub fn add(&mut self, o: Order) -> Result<()> {
        if self.index.contains_key(&o.id) {
            return Err(Error(format!("duplicate active order {}", o.id.0)));
        }
        let cross = match o.side {
            Side::Bid => self.best_ask().is_some_and(|p| o.price >= p),
            Side::Ask => self.best_bid().is_some_and(|p| o.price <= p),
        };
        if cross {
            return Err(Error("resting order crosses book; use match_order".into()));
        }
        let levels = if o.side == Side::Bid {
            &mut self.bids
        } else {
            &mut self.asks
        };
        levels.entry(o.price).or_default().push(o.clone())?;
        self.index.insert(o.id, (o.side, o.price));
        Ok(())
    }
    pub fn cancel(&mut self, id: OrderId) -> Result<Order> {
        let (side, price) = self
            .index
            .remove(&id)
            .ok_or_else(|| Error(format!("unknown order {}", id.0)))?;
        let levels = if side == Side::Bid {
            &mut self.bids
        } else {
            &mut self.asks
        };
        let level = levels
            .get_mut(&price)
            .ok_or_else(|| Error("order index corrupt".into()))?;
        let o = level.remove(id)?;
        if level.orders.is_empty() {
            levels.remove(&price);
        }
        Ok(o)
    }
    pub fn replace(
        &mut self,
        id: OrderId,
        price: Price,
        quantity: Quantity,
        sequence: Sequence,
    ) -> Result<()> {
        if quantity.0 == 0 {
            return Err(Error("replacement quantity must be positive".into()));
        }
        let old = self.cancel(id)?;
        let new = Order {
            price,
            quantity,
            sequence,
            ..old.clone()
        };
        if let Err(e) = self.add(new) {
            let _ = self.add(old);
            return Err(e);
        }
        Ok(())
    }
    pub fn match_order(&mut self, mut taker: Order) -> Result<Vec<Fill>> {
        if taker.quantity.0 == 0 {
            return Err(Error("taker quantity must be positive".into()));
        }
        let mut out = vec![];
        while taker.quantity.0 > 0 {
            let Some(price) = (if taker.side == Side::Bid {
                self.best_ask()
            } else {
                self.best_bid()
            }) else {
                break;
            };
            if match taker.side {
                Side::Bid => taker.price < price,
                Side::Ask => taker.price > price,
            } {
                break;
            }
            let levels = if taker.side == Side::Bid {
                &mut self.asks
            } else {
                &mut self.bids
            };
            let level = levels.get_mut(&price).unwrap();
            while taker.quantity.0 > 0 && !level.orders.is_empty() {
                let maker = level.orders.front_mut().unwrap();
                let q = maker.quantity.0.min(taker.quantity.0);
                maker.quantity.0 -= q;
                taker.quantity.0 -= q;
                level.total_quantity -= q;
                let (buyer, seller) = if taker.side == Side::Bid {
                    (taker.account, maker.account)
                } else {
                    (maker.account, taker.account)
                };
                out.push(Fill {
                    maker_order: maker.id,
                    buyer,
                    seller,
                    price,
                    quantity: Quantity(q),
                });
                if maker.quantity.0 == 0 {
                    let x = level.orders.pop_front().unwrap();
                    self.index.remove(&x.id);
                }
            }
            if level.orders.is_empty() {
                levels.remove(&price);
            }
        }
        if taker.quantity.0 > 0 {
            self.add(taker)?
        }
        Ok(out)
    }
    pub fn validate(&self) -> Result<()> {
        if self
            .best_bid()
            .zip(self.best_ask())
            .is_some_and(|(b, a)| b >= a)
        {
            return Err(Error("crossed resting book".into()));
        }
        if self
            .bids
            .values()
            .chain(self.asks.values())
            .any(|l| !l.valid())
        {
            return Err(Error("invalid price level".into()));
        }
        let n: usize = self
            .bids
            .values()
            .chain(self.asks.values())
            .map(|l| l.orders.len())
            .sum();
        if n != self.index.len() {
            return Err(Error("order index cardinality mismatch".into()));
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferencePrice {
    Midpoint,
    Microprice,
    BestBid,
    BestAsk,
    LastTrade,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImpactTarget {
    NextPriceChange,
    NextEvents(u64),
    FirstMoveTicks(u64),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Impact {
    pub direction: Direction,
    pub price_distance_ticks: i64,
    pub event_distance: u64,
    pub reference: ReferencePrice,
    pub target: ImpactTarget,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MarketState {
    pub best_bid: Option<Price>,
    pub best_ask: Option<Price>,
    pub spread_ticks: Option<u64>,
    pub midpoint_half_ticks: Option<u64>,
    pub microprice_half_ticks: Option<u64>,
    pub bid_depth_lots: u64,
    pub ask_depth_lots: u64,
    pub depth_imbalance_bps: i32,
    pub recent_buy_flow_lots: u64,
    pub recent_sell_flow_lots: u64,
    pub cancel_pressure_bid_lots: u64,
    pub cancel_pressure_ask_lots: u64,
    pub queue_ahead_lots: u64,
    pub inventory_lots: i64,
    pub last_trade: Option<Price>,
    pub sequence: Sequence,
}
impl MarketState {
    pub fn from_book(b: &OrderBook, s: Sequence) -> Self {
        let bd = b.bids.values().map(|l| l.total_quantity).sum();
        let ad = b.asks.values().map(|l| l.total_quantity).sum();
        let total = bd + ad;
        let (bp, ap) = (b.best_bid(), b.best_ask());
        Self {
            best_bid: bp,
            best_ask: ap,
            spread_ticks: bp.zip(ap).map(|(x, y)| y.0 - x.0),
            midpoint_half_ticks: bp.zip(ap).map(|(x, y)| x.0 + y.0),
            microprice_half_ticks: bp.zip(ap).and_then(|(x, y)| {
                if total == 0 {
                    None
                } else {
                    Some(
                        ((y.0 as u128 * bd as u128 + x.0 as u128 * ad as u128) * 2 / total as u128)
                            as u64,
                    )
                }
            }),
            bid_depth_lots: bd,
            ask_depth_lots: ad,
            depth_imbalance_bps: if total == 0 {
                0
            } else {
                ((bd as i128 - ad as i128) * 10000 / total as i128) as i32
            },
            sequence: s,
            ..Self::default()
        }
    }
    fn reference(&self, r: ReferencePrice) -> Result<i64> {
        match r {
            ReferencePrice::Midpoint => self.midpoint_half_ticks,
            ReferencePrice::Microprice => self.microprice_half_ticks,
            ReferencePrice::BestBid => self.best_bid.map(|p| p.0 * 2),
            ReferencePrice::BestAsk => self.best_ask.map(|p| p.0 * 2),
            ReferencePrice::LastTrade => self.last_trade.map(|p| p.0 * 2),
        }
        .map(|x| x as i64)
        .ok_or_else(|| Error("reference price unavailable".into()))
    }
}
pub fn extract_impact(
    states: &[MarketState],
    start: usize,
    reference: ReferencePrice,
    target: ImpactTarget,
) -> Result<Impact> {
    let initial = states
        .get(start)
        .ok_or_else(|| Error("impact start out of range".into()))?
        .reference(reference)?;
    for (i, s) in states.iter().enumerate().skip(start + 1) {
        let d = (i - start) as u64;
        let ticks = (s.reference(reference)? - initial) / 2;
        let hit = match target {
            ImpactTarget::NextPriceChange => ticks != 0,
            ImpactTarget::NextEvents(n) => d == n,
            ImpactTarget::FirstMoveTicks(k) => ticks.unsigned_abs() >= k,
        };
        if hit {
            return Ok(Impact {
                direction: if ticks > 0 {
                    Direction::Up
                } else if ticks < 0 {
                    Direction::Down
                } else {
                    Direction::Flat
                },
                price_distance_ticks: ticks,
                event_distance: d,
                reference,
                target,
            });
        }
    }
    Err(Error("impact target not reached".into()))
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Radix {
    dims: Vec<usize>,
    size: usize,
}
impl Radix {
    pub fn new(dims: Vec<usize>) -> Result<Self> {
        if dims.is_empty() || dims.contains(&0) {
            return Err(Error("radix dimensions must be nonzero".into()));
        }
        let size = dims
            .iter()
            .try_fold(1usize, |a, d| a.checked_mul(*d))
            .ok_or_else(|| Error("state space overflow".into()))?;
        Ok(Self { dims, size })
    }
    pub fn size(&self) -> usize {
        self.size
    }
    pub fn encode(&self, t: &[usize]) -> Result<usize> {
        if t.len() != self.dims.len() {
            return Err(Error("tuple arity mismatch".into()));
        }
        let mut id = 0;
        for (v, r) in t.iter().zip(&self.dims) {
            if v >= r {
                return Err(Error("bucket out of range".into()));
            }
            id = id * r + v
        }
        Ok(id)
    }
    pub fn decode(&self, mut id: usize) -> Result<Vec<usize>> {
        if id >= self.size {
            return Err(Error("state id out of range".into()));
        }
        let mut o = vec![0; self.dims.len()];
        for i in (0..o.len()).rev() {
            o[i] = id % self.dims[i];
            id /= self.dims[i]
        }
        Ok(o)
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ImpactCell {
    pub count: u64,
    pub up_count: u64,
    pub down_count: u64,
    pub flat_count: u64,
    pub sum_signed_distance: i128,
    pub sum_absolute_distance: u128,
    pub sum_squared_distance: u128,
    pub min_distance: i64,
    pub max_distance: i64,
}
impl ImpactCell {
    pub fn observe(&mut self, i: Impact) {
        if self.count == 0 {
            self.min_distance = i.price_distance_ticks;
            self.max_distance = i.price_distance_ticks
        }
        self.count += 1;
        match i.direction {
            Direction::Up => self.up_count += 1,
            Direction::Down => self.down_count += 1,
            Direction::Flat => self.flat_count += 1,
        }
        let d = i.price_distance_ticks;
        self.sum_signed_distance += d as i128;
        self.sum_absolute_distance += d.unsigned_abs() as u128;
        self.sum_squared_distance += (d as i128 * d as i128) as u128;
        self.min_distance = self.min_distance.min(d);
        self.max_distance = self.max_distance.max(d)
    }
    pub fn mean_milli(&self) -> i64 {
        if self.count == 0 {
            0
        } else {
            (self.sum_signed_distance * 1000 / self.count as i128) as i64
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImpactTable {
    pub radix: Radix,
    pub cells: Vec<ImpactCell>,
}
impl ImpactTable {
    pub fn new(r: Radix) -> Self {
        Self {
            cells: vec![ImpactCell::default(); r.size()],
            radix: r,
        }
    }
    pub fn observe(&mut self, t: &[usize], i: Impact) -> Result<()> {
        let id = self.radix.encode(t)?;
        self.cells[id].observe(i);
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Action {
    Hold,
    BuyPassive,
    SellPassive,
    BuyAggressive,
    SellAggressive,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Transition {
    pub next: usize,
    pub reward_micros: i64,
}
pub fn optimal_policy(graph: &[Vec<Vec<Transition>>], terminal: &[i64]) -> Result<Vec<Action>> {
    let actions = [
        Action::Hold,
        Action::BuyPassive,
        Action::SellPassive,
        Action::BuyAggressive,
        Action::SellAggressive,
    ];
    if graph.len() != terminal.len() {
        return Err(Error("DP size mismatch".into()));
    }
    let mut out = vec![Action::Hold; graph.len()];
    for (s, choices) in graph.iter().enumerate() {
        let mut best = None;
        for (a, edges) in choices.iter().enumerate() {
            if a >= actions.len() {
                return Err(Error("too many actions".into()));
            }
            for e in edges {
                let v = e.reward_micros
                    + *terminal
                        .get(e.next)
                        .ok_or_else(|| Error("DP edge out of range".into()))?;
                if best.is_none_or(|(x, ac)| v > x || (v == x && actions[a] < ac)) {
                    best = Some((v, actions[a]))
                }
            }
        }
        if let Some((_, a)) = best {
            out[s] = a
        }
    }
    Ok(out)
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XorShift64 {
    state: u64,
}
impl XorShift64 {
    pub fn new(seed: u64) -> Self {
        Self {
            state: if seed == 0 { 0x9e3779b97f4a7c15 } else { seed },
        }
    }
    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
    pub fn range(&mut self, n: u64) -> u64 {
        if n == 0 { 0 } else { self.next_u64() % n }
    }
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Account {
    pub cash: i128,
    pub position: i64,
    pub fees: i64,
    pub rebates: i64,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Ledger {
    pub accounts: HashMap<AccountId, Account>,
}
impl Ledger {
    pub fn apply(&mut self, f: &Fill, fee_bps: u64, rebate_bps: u64) -> Result<()> {
        let n = f.price.0 as i128 * f.quantity.0 as i128;
        let q = i64::try_from(f.quantity.0).map_err(|_| Error("quantity out of range".into()))?;
        let fee = (n * fee_bps as i128 / 10000) as i64;
        let rebate = (n * rebate_bps as i128 / 10000) as i64;
        let b = self.accounts.entry(f.buyer).or_default();
        b.position += q;
        b.cash -= n;
        b.fees += fee;
        let s = self.accounts.entry(f.seller).or_default();
        s.position -= q;
        s.cash += n;
        s.rebates += rebate;
        Ok(())
    }
    pub fn pnl(&self, id: AccountId, mark: Price) -> i128 {
        self.accounts.get(&id).map_or(0, |a| {
            a.cash + a.position as i128 * mark.0 as i128 - a.fees as i128 + a.rebates as i128
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperimentConfig {
    pub seed: u64,
    pub events: u64,
    pub initial_price: Price,
    pub tick_size: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExperimentResult {
    pub config: ExperimentConfig,
    pub prices: Vec<Price>,
    pub up: u64,
    pub down: u64,
    pub flat: u64,
    pub hash: u64,
}
fn hash(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf29ce484222325;
    for b in bytes {
        h ^= *b as u64;
        h = h.wrapping_mul(0x100000001b3)
    }
    h
}
pub fn simulate(c: ExperimentConfig) -> Result<ExperimentResult> {
    if c.events == 0 || c.events as usize > MAX_RECORDS || c.tick_size == 0 {
        return Err(Error(
            "events and tick size must be positive and bounded".into(),
        ));
    }
    let mut rng = XorShift64::new(c.seed);
    let mut p = c.initial_price.0;
    let mut prices = vec![Price(p)];
    let (mut up, mut down, mut flat) = (0, 0, 0);
    for _ in 0..c.events {
        match rng.range(5) {
            0 | 1 => {
                p = p.saturating_sub(c.tick_size);
                down += 1
            }
            2 => flat += 1,
            _ => {
                p = p
                    .checked_add(c.tick_size)
                    .ok_or_else(|| Error("price overflow".into()))?;
                up += 1
            }
        }
        prices.push(Price(p))
    }
    let mut bytes = vec![];
    for p in &prices {
        bytes.extend_from_slice(&p.0.to_le_bytes())
    }
    let h = hash(&bytes);
    Ok(ExperimentResult {
        config: c,
        prices,
        up,
        down,
        flat,
        hash: h,
    })
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrategyReport {
    pub observations: u64,
    pub expected_impact_milli: i64,
    pub fill_rate_bps: u64,
    pub wins: u64,
    pub losses: u64,
    pub mean_reward_micros: i64,
    pub maximum_drawdown_micros: u64,
    pub out_of_sample_reward_micros: i64,
}
pub fn extract_strategy(r: &ExperimentResult) -> StrategyReport {
    let split = (r.prices.len() * 2 / 3).max(2);
    let rewards: Vec<i64> = r.prices[..split]
        .windows(2)
        .map(|w| w[1].0 as i64 - w[0].0 as i64)
        .collect();
    let sum: i64 = rewards.iter().sum();
    let (mut eq, mut peak, mut dd) = (0i64, 0i64, 0u64);
    for x in &rewards {
        eq += x;
        peak = peak.max(eq);
        dd = dd.max((peak - eq) as u64)
    }
    let oos = r.prices[split - 1..]
        .windows(2)
        .map(|w| w[1].0 as i64 - w[0].0 as i64)
        .sum::<i64>();
    StrategyReport {
        observations: rewards.len() as u64,
        expected_impact_milli: sum * 1000 / rewards.len().max(1) as i64,
        fill_rate_bps: 10000,
        wins: rewards.iter().filter(|x| **x > 0).count() as u64,
        losses: rewards.iter().filter(|x| **x < 0).count() as u64,
        mean_reward_micros: sum * 1_000_000 / rewards.len().max(1) as i64,
        maximum_drawdown_micros: dd * 1_000_000,
        out_of_sample_reward_micros: oos * 1_000_000,
    }
}
fn take(data: &[u8], p: &mut usize) -> Result<u64> {
    let b = data
        .get(*p..*p + 8)
        .ok_or_else(|| Error("truncated file".into()))?;
    *p += 8;
    Ok(u64::from_le_bytes(b.try_into().unwrap()))
}
pub fn save(path: &Path, r: &ExperimentResult) -> Result<()> {
    let mut b = vec![];
    for x in [
        r.config.seed,
        r.config.events,
        r.config.initial_price.0,
        r.config.tick_size,
        r.prices.len() as u64,
    ] {
        b.extend_from_slice(&x.to_le_bytes())
    }
    for p in &r.prices {
        b.extend_from_slice(&p.0.to_le_bytes())
    }
    b.extend_from_slice(&r.hash.to_le_bytes());
    let mut o = MAGIC.to_vec();
    o.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    o.extend_from_slice(&(b.len() as u64).to_le_bytes());
    o.extend_from_slice(&b);
    o.extend_from_slice(&hash(&b).to_le_bytes());
    fs::write(path, o).map_err(|e| Error(format!("cannot save {}: {e}", path.display())))
}
pub fn load(path: &Path) -> Result<ExperimentResult> {
    let d = fs::read(path).map_err(|e| Error(format!("cannot read {}: {e}", path.display())))?;
    if d.get(..8) != Some(MAGIC) {
        return Err(Error("invalid file magic".into()));
    }
    if d.len() < 26 {
        return Err(Error("truncated file".into()));
    }
    let v = u16::from_le_bytes(d[8..10].try_into().unwrap());
    if v != FORMAT_VERSION {
        return Err(Error(format!("unknown format version {v}")));
    }
    let mut p = 10;
    let n = take(&d, &mut p)? as usize;
    if n > MAX_RECORDS * 8 + 48 {
        return Err(Error("file exceeds allocation bound".into()));
    }
    let b = d
        .get(p..p + n)
        .ok_or_else(|| Error("truncated body".into()))?;
    let c = d
        .get(p + n..p + n + 8)
        .ok_or_else(|| Error("missing checksum".into()))?;
    if hash(b) != u64::from_le_bytes(c.try_into().unwrap()) {
        return Err(Error("checksum mismatch".into()));
    }
    let mut q = 0;
    let seed = take(b, &mut q)?;
    let events = take(b, &mut q)?;
    let initial = take(b, &mut q)?;
    let tick = take(b, &mut q)?;
    let count = take(b, &mut q)? as usize;
    if count > MAX_RECORDS + 1 {
        return Err(Error("record count exceeds bound".into()));
    }
    let mut prices = vec![];
    for _ in 0..count {
        prices.push(Price(take(b, &mut q)?))
    }
    let h = take(b, &mut q)?;
    if q != b.len() {
        return Err(Error("trailing data".into()));
    }
    let (mut up, mut down, mut flat) = (0, 0, 0);
    for w in prices.windows(2) {
        if w[1] > w[0] {
            up += 1
        } else if w[1] < w[0] {
            down += 1
        } else {
            flat += 1
        }
    }
    Ok(ExperimentResult {
        config: ExperimentConfig {
            seed,
            events,
            initial_price: Price(initial),
            tick_size: tick,
        },
        prices,
        up,
        down,
        flat,
        hash: h,
    })
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Instrument {
    Crypto {
        symbol: String,
        tick_size: Price,
    },
    Prediction {
        market: String,
        outcome: String,
        probability_bps: u16,
    },
}
impl Instrument {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Crypto { symbol, tick_size } if !symbol.is_empty() && tick_size.0 > 0 => Ok(()),
            Self::Prediction {
                market,
                outcome,
                probability_bps,
            } if !market.is_empty() && !outcome.is_empty() && *probability_bps <= 10000 => Ok(()),
            _ => Err(Error("invalid instrument units or identity".into())),
        }
    }
}
pub fn divergence(a: u16, b: u16) -> Result<i32> {
    if a > 10000 || b > 10000 {
        Err(Error("probability outside 0..=10000 bps".into()))
    } else {
        Ok(a as i32 - b as i32)
    }
}
pub fn replay(events: &[MarketEvent]) -> Result<(OrderBook, EventLog)> {
    let (mut b, mut l) = (OrderBook::default(), EventLog::default());
    for e in events.iter().cloned() {
        match &e {
            MarketEvent::Add(o) => b.add(o.clone())?,
            MarketEvent::Cancel { order_id, .. } => {
                b.cancel(*order_id)?;
            }
            MarketEvent::Replace {
                order_id,
                new_price,
                new_quantity,
                sequence,
            } => b.replace(*order_id, *new_price, *new_quantity, *sequence)?,
            _ => {}
        }
        l.append(e)?;
        b.validate()?
    }
    Ok((b, l))
}
pub fn demo_events() -> Result<Vec<MarketEvent>> {
    Ok(vec![
        MarketEvent::Add(Order::new(1, 1, Side::Bid, 99, 10, 1)?),
        MarketEvent::Add(Order::new(2, 2, Side::Ask, 101, 12, 2)?),
        MarketEvent::BookChange {
            sequence: Sequence(3),
        },
    ])
}
pub fn benchmark(n: u64) -> Result<(u64, u128)> {
    let s = std::time::Instant::now();
    std::hint::black_box(simulate(ExperimentConfig {
        seed: 42,
        events: n,
        initial_price: Price(10000),
        tick_size: 1,
    })?);
    Ok((n, s.elapsed().as_micros().max(1)))
}
pub fn json(r: &ExperimentResult) -> String {
    format!(
        "{{\"version\":\"{}\",\"seed\":{},\"events\":{},\"final_price_ticks\":{},\"up\":{},\"down\":{},\"flat\":{},\"hash\":\"{:016x}\"}}",
        VERSION,
        r.config.seed,
        r.config.events,
        r.prices.last().map_or(0, |p| p.0),
        r.up,
        r.down,
        r.flat,
        r.hash
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn deterministic() {
        let c = ExperimentConfig {
            seed: 7,
            events: 100,
            initial_price: Price(100),
            tick_size: 1,
        };
        assert_eq!(simulate(c.clone()).unwrap(), simulate(c).unwrap())
    }
    #[test]
    fn radix_roundtrip() {
        let r = Radix::new(vec![3, 4, 2]).unwrap();
        for i in 0..r.size() {
            assert_eq!(r.encode(&r.decode(i).unwrap()).unwrap(), i)
        }
    }
}
