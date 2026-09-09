use std::{collections::HashSet, fs};
use tachyon::*;

fn order(id: u64, account: u64, side: Side, price: u64, qty: u64, seq: u64) -> Order {
    Order::new(id, account, side, price, qty, seq).unwrap()
}
#[test]
fn order_rejects_zero() {
    assert!(Order::new(1, 1, Side::Bid, 10, 0, 1).is_err())
}
#[test]
fn event_log_is_strict() {
    let mut l = EventLog::default();
    l.append(MarketEvent::BookChange {
        sequence: Sequence(2),
    })
    .unwrap();
    assert!(
        l.append(MarketEvent::BookChange {
            sequence: Sequence(2)
        })
        .is_err()
    )
}
#[test]
fn price_and_fifo_priority() {
    let mut b = OrderBook::default();
    b.add(order(1, 1, Side::Ask, 101, 3, 1)).unwrap();
    b.add(order(2, 2, Side::Ask, 101, 4, 2)).unwrap();
    b.add(order(3, 3, Side::Ask, 102, 9, 3)).unwrap();
    let f = b.match_order(order(9, 9, Side::Bid, 102, 5, 4)).unwrap();
    assert_eq!(
        f.iter().map(|x| x.maker_order.0).collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert_eq!(f.iter().map(|x| x.quantity.0).sum::<u64>(), 5);
    b.validate().unwrap()
}
#[test]
fn duplicate_cancel_partial_full_and_empty_level() {
    let mut b = OrderBook::default();
    b.add(order(1, 1, Side::Bid, 99, 10, 1)).unwrap();
    assert!(b.add(order(1, 2, Side::Ask, 101, 1, 2)).is_err());
    let f = b.match_order(order(2, 2, Side::Ask, 99, 4, 3)).unwrap();
    assert_eq!(f[0].quantity.0, 4);
    b.match_order(order(3, 2, Side::Ask, 99, 6, 4)).unwrap();
    assert!(b.is_empty());
    assert!(b.cancel(OrderId(404)).is_err())
}
#[test]
fn duplicate_taker_fails_before_mutation() {
    let mut book = OrderBook::default();
    book.add(order(1, 1, Side::Bid, 99, 10, 1)).unwrap();
    book.add(order(2, 2, Side::Ask, 101, 10, 2)).unwrap();
    let snapshot = book.clone();
    assert!(book.match_order(order(1, 3, Side::Bid, 101, 5, 3)).is_err());
    assert_eq!(book, snapshot);
}
#[test]
fn best_prices_and_crossed_rejection() {
    let mut b = OrderBook::default();
    b.add(order(1, 1, Side::Bid, 98, 1, 1)).unwrap();
    b.add(order(2, 1, Side::Bid, 99, 1, 2)).unwrap();
    b.add(order(3, 2, Side::Ask, 102, 1, 3)).unwrap();
    b.add(order(4, 2, Side::Ask, 101, 1, 4)).unwrap();
    assert_eq!(b.best_bid(), Some(Price(99)));
    assert_eq!(b.best_ask(), Some(Price(101)));
    assert!(b.add(order(5, 3, Side::Bid, 101, 1, 5)).is_err())
}
#[test]
fn cancellation_removes_exact_order() {
    let mut b = OrderBook::default();
    b.add(order(1, 1, Side::Bid, 99, 2, 1)).unwrap();
    b.add(order(2, 1, Side::Bid, 99, 3, 2)).unwrap();
    assert_eq!(b.cancel(OrderId(1)).unwrap().quantity, Quantity(2));
    assert_eq!(b.bids[&Price(99)].total_quantity, 3)
}
#[test]
fn replacement_is_validated_and_atomic() {
    let mut b = OrderBook::default();
    b.add(order(1, 1, Side::Bid, 99, 2, 1)).unwrap();
    assert!(
        b.replace(OrderId(1), Price(100), Quantity(0), Sequence(2))
            .is_err()
    );
    assert_eq!(b.len(), 1)
}
#[test]
fn state_units_and_impact_are_exact() {
    let mut b = OrderBook::default();
    b.add(order(1, 1, Side::Bid, 99, 10, 1)).unwrap();
    b.add(order(2, 2, Side::Ask, 101, 30, 2)).unwrap();
    let a = MarketState::from_book(&b, Sequence(2));
    assert_eq!(a.spread_ticks, Some(2));
    assert_eq!(a.midpoint_half_ticks, Some(200));
    assert_eq!(a.depth_imbalance_bps, -5000);
    let mut c = a.clone();
    c.best_bid = Some(Price(101));
    c.best_ask = Some(Price(103));
    c.midpoint_half_ticks = Some(204);
    let i = extract_impact(
        &[a, c],
        0,
        ReferencePrice::Midpoint,
        ImpactTarget::NextPriceChange,
    )
    .unwrap();
    assert_eq!(
        (i.direction, i.price_distance_ticks, i.event_distance),
        (Direction::Up, 2, 1)
    )
}
#[test]
fn radix_is_bijective_and_bounded() {
    let r = Radix::new(vec![4, 3, 2]).unwrap();
    let mut ids = HashSet::new();
    for a in 0..4 {
        for b in 0..3 {
            for c in 0..2 {
                let t = [a, b, c];
                let id = r.encode(&t).unwrap();
                assert!(ids.insert(id));
                assert_eq!(r.decode(id).unwrap(), t)
            }
        }
    }
    assert!(r.encode(&[4, 0, 0]).is_err())
}
#[test]
fn table_matches_hand_fixture() {
    let mut t = ImpactTable::new(Radix::new(vec![2]).unwrap());
    for d in [-2, 0, 4] {
        t.observe(
            &[1],
            Impact {
                direction: if d > 0 {
                    Direction::Up
                } else if d < 0 {
                    Direction::Down
                } else {
                    Direction::Flat
                },
                price_distance_ticks: d,
                event_distance: 1,
                reference: ReferencePrice::Midpoint,
                target: ImpactTarget::NextPriceChange,
            },
        )
        .unwrap()
    }
    let c = &t.cells[1];
    assert_eq!(
        (c.count, c.up_count, c.down_count, c.flat_count),
        (3, 1, 1, 1)
    );
    assert_eq!(c.mean_milli(), 666);
    assert_eq!(c.sum_absolute_distance, 6);
    assert_eq!(c.sum_squared_distance, 20)
}
#[test]
fn dp_known_policy_tie_and_negative() {
    let g = vec![vec![
        vec![Transition {
            next: 0,
            reward_micros: -2,
        }],
        vec![Transition {
            next: 0,
            reward_micros: 3,
        }],
        vec![Transition {
            next: 0,
            reward_micros: 3,
        }],
    ]];
    assert_eq!(optimal_policy(&g, &[0]).unwrap(), vec![Action::BuyPassive])
}
#[test]
fn accounting_reconciles() {
    let f = Fill {
        maker_order: OrderId(1),
        buyer: AccountId(1),
        seller: AccountId(2),
        price: Price(100),
        quantity: Quantity(5),
    };
    let mut l = Ledger::default();
    l.apply(&f, 100, 0).unwrap();
    assert_eq!(l.accounts[&AccountId(1)].position, 5);
    assert_eq!(l.accounts[&AccountId(2)].position, -5);
    assert_eq!(l.accounts.values().map(|a| a.cash).sum::<i128>(), 0);
    assert_eq!(l.pnl(AccountId(1), Price(100)), -5)
}
#[test]
fn deterministic_property_traces() {
    for seed in 1..=64 {
        let a = simulate(ExperimentConfig {
            seed,
            events: 2000,
            initial_price: Price(10000),
            tick_size: 1,
        })
        .unwrap();
        let b = simulate(a.config.clone()).unwrap();
        assert_eq!(a, b, "seed {seed}");
        assert_eq!(a.up + a.down + a.flat, 2000)
    }
}
#[test]
fn persistence_roundtrip_and_corruption() {
    let p = std::env::temp_dir().join(format!("tachyon-contract-{}.bin", std::process::id()));
    let r = simulate(ExperimentConfig {
        seed: 8,
        events: 50,
        initial_price: Price(10),
        tick_size: 1,
    })
    .unwrap();
    save(&p, &r).unwrap();
    assert_eq!(load(&p).unwrap(), r);
    let mut d = fs::read(&p).unwrap();
    d[20] ^= 1;
    fs::write(&p, &d).unwrap();
    assert!(load(&p).is_err());
    d.truncate(9);
    fs::write(&p, &d).unwrap();
    assert!(load(&p).is_err());
    let _ = fs::remove_file(p);
}
#[test]
fn replay_exact_snapshot() {
    let (b, l) = replay(&demo_events().unwrap()).unwrap();
    assert_eq!(l.events().len(), 3);
    assert_eq!(b.best_bid(), Some(Price(99)));
    assert_eq!(b.best_ask(), Some(Price(101)));
    b.validate().unwrap()
}
#[test]
fn simulation_rejects_invalid_bounds() {
    assert!(
        simulate(ExperimentConfig {
            seed: 1,
            events: 0,
            initial_price: Price(1),
            tick_size: 1
        })
        .is_err()
    );
    assert!(
        simulate(ExperimentConfig {
            seed: 1,
            events: MAX_RECORDS as u64 + 1,
            initial_price: Price(1),
            tick_size: 1
        })
        .is_err()
    )
}
#[test]
fn market_models_are_explicit() {
    Instrument::Crypto {
        symbol: "BTC-USD".into(),
        tick_size: Price(1),
    }
    .validate()
    .unwrap();
    Instrument::Prediction {
        market: "Election".into(),
        outcome: "YES".into(),
        probability_bps: 5500,
    }
    .validate()
    .unwrap();
    assert_eq!(divergence(6000, 5500).unwrap(), 500);
    assert!(divergence(10001, 0).is_err())
}
#[test]
fn strategy_split_prevents_future_leakage() {
    let mut a = simulate(ExperimentConfig {
        seed: 5,
        events: 30,
        initial_price: Price(100),
        tick_size: 1,
    })
    .unwrap();
    let before = extract_strategy(&a);
    for p in &mut a.prices[21..] {
        p.0 += 1000
    }
    let after = extract_strategy(&a);
    assert_eq!(before.observations, after.observations);
    assert_eq!(before.mean_reward_micros, after.mean_reward_micros);
    assert_ne!(
        before.out_of_sample_reward_micros,
        after.out_of_sample_reward_micros
    )
}
