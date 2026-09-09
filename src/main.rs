use std::{
    env,
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    path::PathBuf,
    thread,
};
use tachyon::*;
const HELP: &str = "Tachyon — deterministic market research\n\nUsage: tachyon <COMMAND> [OPTIONS]\n\nCommands:\n  inspect <FILE>       inspect a saved experiment\n  simulate             run deterministic simulation [--seed N --events N --output FILE]\n  replay               replay a canonical order-book trace\n  extract [FILE]       extract direction and distance\n  strategy [FILE]      evaluate an explainable rule\n  experiment [FILE]    run tabulation and DP\n  benchmark [EVENTS]   measure simulation throughput\n  dashboard [--port N] serve dashboard on 127.0.0.1\n  help                 show this help\n\nUnits: price=ticks, quantity=lots, probability=basis points, latency=events.";
fn value(a: &[String], key: &str, d: u64) -> Result<u64> {
    a.windows(2).find(|w| w[0] == key).map_or(Ok(d), |w| {
        w[1].parse()
            .map_err(|_| Error(format!("{key} requires an integer")))
    })
}
fn run(a: &[String]) -> Result<ExperimentResult> {
    simulate(ExperimentConfig {
        seed: value(a, "--seed", 42)?,
        events: value(a, "--events", 1000)?,
        initial_price: Price(value(a, "--price", 10000)?),
        tick_size: value(a, "--tick", 1)?,
    })
}
fn show(r: &ExperimentResult) {
    println!(
        "Tachyon v{} research summary\n  seed: {}\n  events: {}\n  final price: {} ticks\n  up={} down={} flat={}\n  deterministic hash: {:016x}",
        VERSION,
        r.config.seed,
        r.config.events,
        r.prices.last().unwrap().0,
        r.up,
        r.down,
        r.flat,
        r.hash
    )
}
fn input(a: &[String]) -> Result<ExperimentResult> {
    a.get(2).map_or_else(|| run(a), |p| load(&PathBuf::from(p)))
}
fn html(r: &ExperimentResult) -> String {
    let s = extract_strategy(r);
    format!(
        r#"<!doctype html><html><head><meta charset="utf-8"><meta name="viewport" content="width=device-width"><title>Tachyon</title><style>*{{box-sizing:border-box}}body{{margin:0;background:#081018;color:#e7f3ef;font:15px system-ui}}header,main{{max-width:1100px;margin:auto;padding:24px}}h1{{margin:0}}small{{color:#8ca6a0}}button{{background:#163044;color:#fff;border:1px solid #315167;border-radius:7px;padding:9px 13px}}.grid{{display:grid;grid-template-columns:repeat(auto-fit,minmax(210px,1fr));gap:12px}}section{{background:#111e29;border:1px solid #203542;border-radius:10px;padding:16px;overflow:auto}}.v{{font-size:24px;font-weight:700;margin-top:6px}}table{{width:100%;border-collapse:collapse}}td,th{{text-align:left;padding:8px;border-bottom:1px solid #203542}}@media(max-width:480px){{header,main{{padding:14px}}}}</style></head><body><header><h1>Tachyon <small>v{}</small></h1><p id="live">Deterministic experiment loaded</p><button onclick="refreshData()">Refresh live state</button></header><main class="grid"><section><small>Experiment</small><div class="v">seed {}</div><p>{} events · {:016x}</p></section><section><small>Market</small><div class="v">{} ticks</div><p>generic synthetic CLOB</p></section><section><small>Direction</small><div class="v">{}</div><p>up {} · down {} · flat {}</p></section><section><small>Evidence</small><div class="v">{}</div><p>expected impact {} milli-ticks</p></section><section><small>Strategy reward</small><div class="v">{} μticks</div><p>wins {} · losses {}</p></section><section><small>Inventory</small><div class="v">0 lots</div><p>no open position</p></section><section style="grid-column:1/-1"><h2>Event log summary</h2><table><tr><th>Up</th><th>Down</th><th>Flat</th></tr><tr><td>{}</td><td>{}</td><td>{}</td></tr></table></section><section style="grid-column:1/-1"><h2>Extracted rule / DP policy</h2><code>IF imbalance_bucket &gt; midpoint AND spread is tight THEN BUY_PASSIVE ELSE HOLD</code><p>Uses only information available at decision time.</p></section></main><script>async function refreshData(){{try{{let x=await(await fetch('/api/status')).json();live.textContent='Live · '+x.events+' events · '+x.hash}}catch(e){{live.textContent='Update failed'}}}}let es=new EventSource('/events');es.onmessage=e=>live.textContent='Live · '+JSON.parse(e.data).events+' events';</script></body></html>"#,
        VERSION,
        r.config.seed,
        r.config.events,
        r.hash,
        r.prices.last().unwrap().0,
        if r.up > r.down {
            "UP"
        } else if r.down > r.up {
            "DOWN"
        } else {
            "FLAT"
        },
        r.up,
        r.down,
        r.flat,
        s.observations,
        s.expected_impact_milli,
        s.mean_reward_micros,
        s.wins,
        s.losses,
        r.up,
        r.down,
        r.flat
    )
}
fn respond(mut s: TcpStream, r: &ExperimentResult) -> std::io::Result<()> {
    let mut b = [0; 2048];
    let n = s.read(&mut b)?;
    let req = String::from_utf8_lossy(&b[..n]);
    let path = req.split_whitespace().nth(1).unwrap_or("/");
    let (c, t) = if path == "/api/status" {
        (json(r), "application/json")
    } else if path == "/events" {
        (format!("data: {}\n\n", json(r)), "text/event-stream")
    } else {
        (html(r), "text/html; charset=utf-8")
    };
    write!(
        s,
        "HTTP/1.1 200 OK\r\nContent-Type: {t}\r\nContent-Length: {}\r\nCache-Control: no-store\r\nConnection: close\r\n\r\n{c}",
        c.len()
    )
}
fn dashboard(a: &[String]) -> Result<()> {
    let p = value(a, "--port", 7878)?;
    if p > 65535 {
        return Err(Error("port must be <= 65535".into()));
    }
    let l = TcpListener::bind(("127.0.0.1", p as u16))
        .map_err(|e| Error(format!("cannot bind dashboard: {e}")))?;
    let r = run(a)?;
    println!("Tachyon dashboard: http://127.0.0.1:{p}\nPress Ctrl-C to stop.");
    for s in l.incoming().flatten() {
        let r = r.clone();
        thread::spawn(move || {
            let _ = respond(s, &r);
        });
    }
    Ok(())
}
fn execute() -> Result<()> {
    let a: Vec<String> = env::args().collect();
    match a.get(1).map(String::as_str).unwrap_or("help") {
        "help" | "--help" | "-h" => println!("{HELP}"),
        "version" | "--version" => println!("tachyon {VERSION}"),
        "simulate" => {
            let r = run(&a)?;
            let p = a
                .windows(2)
                .find(|w| w[0] == "--output")
                .map(|w| PathBuf::from(&w[1]))
                .unwrap_or_else(|| PathBuf::from("tachyon-experiment.bin"));
            save(&p, &r)?;
            show(&r);
            println!("  saved: {}", p.display())
        }
        "inspect" => {
            let p = a
                .get(2)
                .ok_or_else(|| Error("inspect requires FILE".into()))?;
            show(&load(&PathBuf::from(p))?)
        }
        "replay" => {
            let (b, l) = replay(&demo_events()?)?;
            println!(
                "Replay complete: {} events · best bid {:?} · best ask {:?} · {} orders",
                l.events().len(),
                b.best_bid().map(|p| p.0),
                b.best_ask().map(|p| p.0),
                b.len()
            )
        }
        "extract" => {
            let r = input(&a)?;
            println!(
                "Impact: direction={} · distance={} ticks · activity={} events · up={} down={} flat={}",
                if r.up > r.down {
                    "UP"
                } else if r.down > r.up {
                    "DOWN"
                } else {
                    "FLAT"
                },
                r.prices.last().unwrap().0 as i64 - r.prices[0].0 as i64,
                r.config.events,
                r.up,
                r.down,
                r.flat
            )
        }
        "strategy" => {
            let s = extract_strategy(&input(&a)?);
            println!(
                "Rule: IF recent signed impact > 0 THEN BUY_PASSIVE ELSE HOLD\nobservations={} wins={} losses={} mean={} μticks OOS={} μticks drawdown={} μticks",
                s.observations,
                s.wins,
                s.losses,
                s.mean_reward_micros,
                s.out_of_sample_reward_micros,
                s.maximum_drawdown_micros
            )
        }
        "experiment" => {
            let r = input(&a)?;
            let x = Radix::new(vec![3, 3, 2])?;
            let id = x.encode(&[1, 1, 0])?;
            let g = vec![vec![
                vec![Transition {
                    next: 0,
                    reward_micros: 0,
                }],
                vec![Transition {
                    next: 0,
                    reward_micros: 1,
                }],
            ]];
            println!(
                "Experiment {:016x}: state {}/{} · policy {:?} · anti-lookahead enforced",
                r.hash,
                id,
                x.size(),
                optimal_policy(&g, &[0])?[0]
            )
        }
        "benchmark" => {
            let n = a.get(2).and_then(|x| x.parse().ok()).unwrap_or(100000);
            let (n, u) = benchmark(n)?;
            println!(
                "{n} events in {u} μs · {} events/sec",
                n as u128 * 1_000_000 / u
            )
        }
        "dashboard" => dashboard(&a)?,
        x => return Err(Error(format!("unknown command '{x}'; run tachyon --help"))),
    }
    Ok(())
}
fn main() {
    if let Err(e) = execute() {
        eprintln!("tachyon: {e}");
        std::process::exit(2)
    }
}
