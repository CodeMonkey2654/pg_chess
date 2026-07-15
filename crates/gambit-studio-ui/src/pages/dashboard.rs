//! Dashboard page.

use super::util::{event_target_value, shard_status_label};
use crate::format::format_num;
use crate::job_poll::parse_download_progress;
use gambit_proto::{FilesetView, JobStatus, SourceDetail, SourceListItem};
use leptos::prelude::*;

#[component]
pub(crate) fn JobProgressCard(job: JobStatus, live_source_games: Option<i64>) -> impl IntoView {
    let download_pct = parse_download_progress(&job.message).map(|(cur, tot)| {
        if tot > 0.0 {
            (cur / tot * 100.0).min(100.0)
        } else {
            0.0
        }
    });
    let shard_pct = if job.total_shards > 0 {
        ((job.current_shard.saturating_sub(1)) as f64 / job.total_shards as f64 * 100.0)
            .max(if job.games_loaded > 0 { 5.0 } else { 0.0 })
    } else {
        0.0
    };
    let pill_class = format!("status-pill {}", job.status);
    let is_running = job.status == "running";
    let download_done = job.message.contains("/ 30.") && job.message.contains("GiB)");
    let show_ingest_hint = is_running && job.games_loaded == 0 && download_done;

    view! {
        <div class="job-card">
            <div class="job-header">
                <span class="job-title">{format!("Ingest job #{}", job.id)}</span>
                <span class=pill_class>{job.status.clone()}</span>
            </div>
            <p class="job-message">{job.message.clone()}</p>
            {show_ingest_hint.then(|| view! {
                <p class="job-hint">"Download complete — now parsing and loading into PostgreSQL. This phase can take hours for a 30 GiB shard. Watch the source game count below for live progress."</p>
            })}
            {live_source_games.filter(|_| is_running).map(|n| view! {
                <p class="job-live mono">{format!("{} games in database for selected source (live)", format_num(n))}</p>
            })}
            <div class="job-stats">
                <div class="job-stat">
                    <span class="lbl">"Shard"</span>
                    <span class="val">{format!("{} / {}", job.current_shard, job.total_shards)}</span>
                </div>
                <div class="job-stat">
                    <span class="lbl">"Games loaded"</span>
                    <span class="val accent">{format_num(job.games_loaded as i64)}</span>
                </div>
                <div class="job-stat">
                    <span class="lbl">"Throughput"</span>
                    <span class="val">
                        {job.games_per_min
                            .map(|g| format!("{g:.0}/min"))
                            .unwrap_or_else(|| "—".into())}
                    </span>
                </div>
            </div>
            {download_pct.map(|pct| view! {
                <div class="progress-wrap">
                    <div class="progress-label">
                        <span>"Current shard download"</span>
                        <span class="mono">{format!("{pct:.0}%")}</span>
                    </div>
                    <div class="progress-track">
                        <div class="progress-fill" style=format!("width: {pct:.1}%")/>
                    </div>
                </div>
            })}
            {(is_running && download_pct.is_none()).then(|| view! {
                <div class="progress-wrap">
                    <div class="progress-label">
                        <span>"Overall progress"</span>
                        <span class="mono">{format!("{shard_pct:.0}%")}</span>
                    </div>
                    <div class="progress-track">
                        <div class="progress-fill indeterminate" style=format!("width: {shard_pct:.1}%")/>
                    </div>
                </div>
            })}
        </div>
    }
}

#[component]
pub fn DashboardPanel(
    sources: ReadSignal<Vec<SourceListItem>>,
    sources_loading: ReadSignal<bool>,
    sources_error: ReadSignal<Option<String>>,
    selected_source: ReadSignal<Option<i32>>,
    set_selected_source: WriteSignal<Option<i32>>,
    source_detail: ReadSignal<Option<SourceDetail>>,
    source_detail_error: ReadSignal<Option<String>>,
    filesets: ReadSignal<Vec<FilesetView>>,
    job: ReadSignal<Option<JobStatus>>,
    job_error: ReadSignal<Option<String>>,
    status_msg: ReadSignal<String>,
    load_year: ReadSignal<i32>,
    set_load_year: WriteSignal<i32>,
    source_name: ReadSignal<String>,
    set_source_name: WriteSignal<String>,
    on_sync: impl Fn(leptos::ev::MouseEvent) + 'static,
    on_load: impl Fn(leptos::ev::MouseEvent) + 'static,
    sync_loading: ReadSignal<bool>,
    load_loading: ReadSignal<bool>,
) -> impl IntoView {
    let completed_shards = move || {
        filesets
            .get()
            .iter()
            .filter(|f| f.status == "complete")
            .count()
    };

    view! {
        <div class="dash-grid">
            <section class="panel">
                <div class="panel-header">
                    <div>
                        <h2>"Data sources"</h2>
                        <p class="panel-desc">"Select a source to inspect its fileset shards"</p>
                    </div>
                </div>
                {move || {
                    if sources_loading.get() {
                        return view! {
                            <ul class="source-list">
                                {(0..3).map(|_| view! {
                                    <li class="source-card" style="opacity: 0.45; pointer-events: none;">
                                        <div>
                                            <strong><span class="loading-spinner"/>" Loading…"</strong>
                                        </div>
                                        <div class="source-stats">
                                            <div class="stat-pill">
                                                <span class="val">"—"</span>
                                                <span class="lbl">"games"</span>
                                            </div>
                                            <div class="stat-pill">
                                                <span class="val">"—"</span>
                                                <span class="lbl">"positions"</span>
                                            </div>
                                        </div>
                                    </li>
                                }).collect_view()}
                            </ul>
                        }.into_any();
                    }
                    let list = sources.get();
                    view! {
                        {sources_error.get().map(|err| view! {
                            <div class="toast error">{err}</div>
                        })}
                        {if list.is_empty() {
                            view! {
                                <div class="empty-state">
                                    <div class="empty-icon">"♟"</div>
                                    <p>"No sources yet — sync or load a fileset to begin"</p>
                                </div>
                            }.into_any()
                        } else {
                            view! {
                                <ul class="source-list">
                                    {list.into_iter().map(|s| {
                                        let id = s.id;
                                        let active = selected_source.get() == Some(id);
                                        let detail = source_detail.get();
                                        let show_stats = active && detail.as_ref().is_some_and(|d| d.id == id);
                                        view! {
                                            <li class="source-card" class:active=active
                                                on:click=move |_| set_selected_source.set(Some(id))>
                                                <div>
                                                    <strong>{s.name.clone()}</strong>
                                                    {s.description.clone().map(|d| view! {
                                                        <span style="display: block; font-size: 0.78rem; color: var(--muted); margin-top: 0.15rem;">{d}</span>
                                                    })}
                                                </div>
                                                {if show_stats {
                                                    let d = detail.unwrap();
                                                    view! {
                                                        <div class="source-stats">
                                                            <div class="stat-pill">
                                                                <span class="val">{format_num(d.games)}</span>
                                                                <span class="lbl">"games"</span>
                                                            </div>
                                                            <div class="stat-pill">
                                                                <span class="val">{format_num(d.positions)}</span>
                                                                <span class="lbl">"positions"</span>
                                                            </div>
                                                        </div>
                                                    }.into_any()
                                                } else if active {
                                                    view! {
                                                        <div class="source-stats">
                                                            <span class="mono" style="color: var(--muted); font-size: 0.78rem;">
                                                                <span class="loading-spinner"/>" Loading stats…"
                                                            </span>
                                                        </div>
                                                    }.into_any()
                                                } else {
                                                    view! { <span/> }.into_any()
                                                }}
                                            </li>
                                        }
                                    }).collect_view()}
                                </ul>
                            }.into_any()
                        }}
                        {source_detail_error.get().map(|e| view! {
                            <div class="toast error">{e}</div>
                        })}
                    }.into_any()
                }}
            </section>

            <section class="panel">
                <div class="panel-header">
                    <div>
                        <h2>"Load Lichess fileset"</h2>
                        <p class="panel-desc">"Sync the catalog, then ingest a full year of rated games"</p>
                    </div>
                </div>
                <div class="form-stack">
                    <div class="form-field">
                        <label>"Source name"</label>
                        <input prop:value=move || source_name.get()
                            on:input=move |ev| set_source_name.set(event_target_value(&ev)) />
                    </div>
                    <div class="form-field">
                        <label>"Year"</label>
                        <input type="number" prop:value=move || load_year.get().to_string()
                            on:input=move |ev| {
                                if let Ok(y) = event_target_value(&ev).parse() {
                                    set_load_year.set(y);
                                }
                            } />
                    </div>
                </div>
                <div class="actions">
                    <button on:click=on_sync disabled=move || sync_loading.get()>
                        {move || if sync_loading.get() {
                            view! { <span><span class="loading-spinner"/>"Syncing…"</span> }.into_any()
                        } else {
                            view! { "Sync catalog" }.into_any()
                        }}
                    </button>
                    <button class="primary" on:click=on_load disabled=move || load_loading.get()>
                        {move || if load_loading.get() {
                            view! { <span><span class="loading-spinner"/>"Starting…"</span> }.into_any()
                        } else {
                            view! { "Load full year" }.into_any()
                        }}
                    </button>
                </div>
                <p class="hint">"Target: ≥100,000 games/min ingest throughput"</p>
                {move || {
                    let msg = status_msg.get();
                    if msg.is_empty() { None } else { Some(view! { <div class="toast">{msg}</div> }) }
                }}
                {move || {
                    let live = source_detail.get().map(|d| d.games);
                    job.get().map(|j| view! { <JobProgressCard job=j live_source_games=live/> })
                }}
                {move || job_error.get().map(|e| view! { <div class="toast error">{e}</div> })}
            </section>

            <section class="panel dash-full">
                <div class="panel-header">
                    <div>
                        <h2>"Monthly shards"</h2>
                        <p class="panel-desc">
                            {move || format!("{} of {} shards complete", completed_shards(), filesets.get().len())}
                        </p>
                    </div>
                </div>
                {move || {
                    let shards = filesets.get();
                    if shards.is_empty() {
                        view! {
                            <div class="empty-state">
                                <div class="empty-icon">"📅"</div>
                                <p>"Sync the catalog to see monthly shards for your selected year"</p>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <div class="shard-grid">
                                {shards.into_iter().map(|f| {
                                    let status_class = format!("shard status-{}", f.status);
                                    view! {
                                        <div class=status_class>
                                            <span class="shard-label">{f.period_label.clone()}</span>
                                            <span class="shard-status">{shard_status_label(&f.status)}</span>
                                            <span class="shard-games mono">
                                                {format!("{} games", format_num(f.games_loaded))}
                                            </span>
                                            {f.error_message.clone().map(|e| {
                                                let title = e.clone();
                                                view! {
                                                    <small class="shard-error" title=title>{e}</small>
                                                }
                                            })}
                                        </div>
                                    }
                                }).collect_view()}
                            </div>
                        }.into_any()
                    }
                }}
            </section>
        </div>
    }
}
