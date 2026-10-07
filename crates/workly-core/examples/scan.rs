//! Scan timing for scripts/perf.sh: `cargo run --release --example scan -- <workspace>`.

use std::time::Instant;

fn main() {
    let root = std::env::args().nth(1).expect("usage: scan <workspace>");
    let start = Instant::now();
    let mut ws = workly_core::Workspace::open(&root).expect("open workspace");
    let open = start.elapsed();
    let mut runs: Vec<f64> = (0..20)
        .map(|_| {
            let t = Instant::now();
            ws.rescan();
            t.elapsed().as_secs_f64() * 1000.0
        })
        .collect();
    runs.sort_by(f64::total_cmp);
    let idx = &ws.index;
    println!("{} tasks, {} projects, {} errors", idx.tasks.len(), idx.projects.len(), idx.errors.len());
    println!("open (first scan): {:.1} ms", open.as_secs_f64() * 1000.0);
    println!("rescan: median {:.1} ms, max {:.1} ms (20 runs)", runs[10], runs[19]);
}
