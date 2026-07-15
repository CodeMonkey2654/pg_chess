//! Query benchmark page.

use crate::api;
use crate::format::format_num;
use gambit_proto::BenchResponse;
use leptos::prelude::*;

#[component]
pub fn BenchPanel() -> impl IntoView {
    let (results, set_results) = signal(None::<BenchResponse>);
    let (running, set_running) = signal(false);
    let (bench_error, set_bench_error) = signal(None::<String>);

    let run = move |_| {
        set_running.set(true);
        set_bench_error.set(None);
        leptos::task::spawn_local(async move {
            match api::run_bench().await {
                Ok(r) => {
                    set_results.set(Some(r));
                    set_bench_error.set(None);
                }
                Err(e) => {
                    set_results.set(None);
                    set_bench_error.set(Some(e));
                }
            }
            set_running.set(false);
        });
    };

    view! {
        <section class="panel">
            <div class="panel-header">
                <div>
                    <h2>"Query benchmarks"</h2>
                    <p class="panel-desc">"Measure PostgreSQL query latency against your loaded data"</p>
                </div>
            </div>
            <p class="bench-intro">
                "Fourteen queries that mirror real Studio workflows — player search, position lookup, opening explorer, and more. Latency here is what users feel in the browser."
            </p>
            <button class="bench-run-btn" on:click=run disabled=move || running.get()>
                {move || if running.get() {
                    view! { <span><span class="loading-spinner"/>"Running 14 benchmarks…"</span> }.into_any()
                } else {
                    view! { "Run benchmark suite" }.into_any()
                }}
            </button>
            {bench_error.get().map(|e| view! {
                <div class="toast">{e}</div>
            })}
            {move || results.get().map(|r| {
                let max_ms = r.results.iter().map(|x| x.latency_ms).fold(0.0_f64, f64::max).max(1.0);
                let total_ms: f64 = r.results.iter().map(|x| x.latency_ms).sum();
                view! {
                    <div class="bench-summary mono">
                        {format!("{} queries · {:.0} ms total", r.results.len(), total_ms)}
                    </div>
                    <table class="bench-table">
                        <thead>
                            <tr>
                                <th>"Benchmark"</th>
                                <th>"Latency"</th>
                                <th>"Rows"</th>
                            </tr>
                        </thead>
                        <tbody>
                            {r.results.iter().map(|row| {
                                let pct = (row.latency_ms / max_ms * 100.0).min(100.0);
                                let speed_class = if row.latency_ms < 50.0 { "fast" } else { "slow" };
                                view! {
                                    <tr>
                                        <td>
                                            <div class="bench-name">{row.title.clone()}</div>
                                            <div class="bench-desc">{row.description.clone()}</div>
                                        </td>
                                        <td>
                                            <div class="bench-bar-wrap">
                                                <div class="bench-bar">
                                                    <div class="bench-bar-fill" style=format!("width: {pct:.1}%")/>
                                                </div>
                                                <span class=format!("bench-latency {speed_class}")>
                                                    {format!("{:.1} ms", row.latency_ms)}
                                                </span>
                                            </div>
                                        </td>
                                        <td class="mono">{format_num(row.rows)}</td>
                                    </tr>
                                }
                            }).collect_view()}
                        </tbody>
                    </table>
                }
            })}
        </section>
    }
}
