//! Gambit Studio UI components.

use crate::api;
use crate::api_types::{
    BenchResponse, FilesetView, GameDetail, GameListItem, JobStatus, OpeningMoveStat,
    PositionHit, SourceDetail, SourceListItem,
};
use crate::board::uci::parse_uci;
use crate::board::{BoardOrientation, ChessBoard};
use crate::brand::{HealthBadge, Logo};
use crate::format::format_num;
use crate::replay::{position_at_ply, ReplayError};
use gambit_db_wasm::WasmPosition;
use leptos::prelude::*;

const GAMES_PAGE_SIZE: i64 = 40;
const EXPLORER_PAGE_SIZE: i64 = 25;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Dashboard,
    Games,
    Explorer,
    Benchmarks,
}

fn parse_download_progress(msg: &str) -> Option<(f64, f64)> {
    let open = msg.find('(')?;
    let close = msg.find(')')?;
    let inner = msg.get(open + 1..close)?;
    let mut parts = inner.split('/');
    let current: f64 = parts.next()?.trim().split_whitespace().next()?.parse().ok()?;
    let total: f64 = parts.next()?.trim().split_whitespace().next()?.parse().ok()?;
    Some((current, total))
}

fn shard_status_label(status: &str) -> &'static str {
    match status {
        "complete" => "Done",
        "downloading" => "Downloading",
        "ingesting" => "Ingesting",
        "failed" => "Failed",
        _ => "Pending",
    }
}

fn event_target_value(ev: &leptos::ev::Event) -> String {
    use wasm_bindgen::JsCast;
    ev.target()
        .and_then(|t| t.dyn_into::<web_sys::HtmlInputElement>().ok())
        .map(|el| el.value())
        .unwrap_or_default()
}

fn fen_hash_local(fen: &str) -> Option<i64> {
    WasmPosition::from_fen(fen)
        .ok()
        .map(|p| p.zobrist_hash())
}

async fn resolve_position_hash(fen: &str) -> Result<i64, String> {
    if let Some(h) = fen_hash_local(fen) {
        return Ok(h);
    }
    api::hash_from_fen(fen).await
}

fn filesets_query_name(
    selected: Option<i32>,
    sources: &[SourceListItem],
    load_form_name: &str,
) -> String {
    selected
        .and_then(|id| {
            sources
                .iter()
                .find(|s| s.id == id)
                .map(|s| s.name.clone())
        })
        .unwrap_or_else(|| load_form_name.to_string())
}

fn replay_error_message(err: &ReplayError) -> String {
    match err {
        ReplayError::InvalidFen(f) => format!("Invalid start FEN: {f}"),
        ReplayError::InvalidMove { ply, uci } => format!("Invalid move at ply {ply}: {uci}"),
    }
}

fn spawn_job_polling(
    job_id: u64,
    set_job: WriteSignal<Option<JobStatus>>,
    set_filesets: WriteSignal<Vec<FilesetView>>,
    set_source_detail: WriteSignal<Option<SourceDetail>>,
    set_sources: WriteSignal<Vec<SourceListItem>>,
    set_sources_error: WriteSignal<Option<String>>,
    selected_source: ReadSignal<Option<i32>>,
    source_name: ReadSignal<String>,
    sources: ReadSignal<Vec<SourceListItem>>,
) {
    leptos::task::spawn_local(async move {
        let mut tick: u32 = 0;
        loop {
            let done = match api::fetch_job(job_id).await {
                Ok(j) => {
                    let finished = j.status == "complete" || j.status == "failed";
                    set_job.set(Some(j));
                    finished
                }
                Err(e) => {
                    set_job.set(None);
                    let _ = e;
                    true
                }
            };

            let name = filesets_query_name(
                selected_source.get_untracked(),
                &sources.get_untracked(),
                &source_name.get_untracked(),
            );
            if let Ok(list) = api::fetch_filesets_by_name(&name).await {
                set_filesets.set(list);
            }

            if tick % 5 == 0 {
                if let Some(id) = selected_source.get_untracked() {
                    if let Ok(detail) = api::fetch_source_detail(id).await {
                        set_source_detail.set(Some(detail));
                    }
                }
            }

            match api::fetch_sources().await {
                Ok(list) => {
                    set_sources.set(list);
                    set_sources_error.set(None);
                }
                Err(e) => set_sources_error.set(Some(e)),
            }

            if done {
                break;
            }
            tick = tick.wrapping_add(1);
            gloo_timers::future::TimeoutFuture::new(2000).await;
        }
    });
}

#[component]
pub fn App() -> impl IntoView {
    let (page, set_page) = signal(Page::Dashboard);
    let (sources, set_sources) = signal(Vec::<SourceListItem>::new());
    let (sources_loading, set_sources_loading) = signal(true);
    let (sources_error, set_sources_error) = signal(None::<String>);
    let (selected_source, set_selected_source) = signal(None::<i32>);
    let (source_detail, set_source_detail) = signal(None::<SourceDetail>);
    let (source_detail_error, set_source_detail_error) = signal(None::<String>);
    let (filesets, set_filesets) = signal(Vec::<FilesetView>::new());
    let (job, set_job) = signal(None::<JobStatus>);
    let (status_msg, set_status_msg) = signal(String::new());
    let (load_year, set_load_year) = signal(2024_i32);
    let (source_name, set_source_name) = signal("lichess_standard_2024".to_string());
    let (healthy, set_healthy) = signal(true);
    let (pending_game_id, set_pending_game_id) = signal(None::<i64>);
    let (pending_ply, set_pending_ply) = signal(None::<usize>);

    let refresh_filesets = move || {
        let name = filesets_query_name(
            selected_source.get_untracked(),
            &sources.get_untracked(),
            &source_name.get_untracked(),
        );
        leptos::task::spawn_local(async move {
            match api::fetch_filesets_by_name(&name).await {
                Ok(list) => set_filesets.set(list),
                Err(e) => set_status_msg.set(format!("filesets error: {e}")),
            }
        });
    };

    let refresh_sources = move || {
        leptos::task::spawn_local(async move {
            set_sources_loading.set(true);
            match api::fetch_sources().await {
                Ok(list) => {
                    set_sources.set(list.clone());
                    set_sources_error.set(None);
                    if selected_source.get_untracked().is_none() {
                        if let Some(first) = list.first() {
                            set_selected_source.set(Some(first.id));
                        }
                    }
                }
                Err(e) => set_sources_error.set(Some(e)),
            }
            set_sources_loading.set(false);
        });
    };

    Effect::new(move |_| {
        set_sources_loading.set(true);
        set_sources_error.set(None);

        leptos::task::spawn_local(async move {
            match api::fetch_health().await {
                Ok(h) => set_healthy.set(h.database_ok),
                Err(_) => set_healthy.set(false),
            }
        });

        leptos::task::spawn_local(async move {
            match api::fetch_sources().await {
                Ok(list) => {
                    set_sources.set(list.clone());
                    set_sources_error.set(None);
                    if selected_source.get_untracked().is_none() {
                        if let Some(first) = list.first() {
                            set_selected_source.set(Some(first.id));
                        }
                    }
                }
                Err(e) => set_sources_error.set(Some(e)),
            }
            set_sources_loading.set(false);
        });

        let sn = source_name.get_untracked();
        let yr = load_year.get_untracked();
        leptos::task::spawn_local(async move {
            match api::fetch_active_job(&sn, yr).await {
                Ok(Some(j)) => {
                    set_job.set(Some(j.clone()));
                    spawn_job_polling(
                        j.id,
                        set_job,
                        set_filesets,
                        set_source_detail,
                        set_sources,
                        set_sources_error,
                        selected_source,
                        source_name,
                        sources,
                    );
                }
                Ok(None) => {}
                Err(e) => set_status_msg.set(format!("active job error: {e}")),
            }
        });
    });

    Effect::new(move |_| {
        if let Some(id) = selected_source.get() {
            leptos::task::spawn_local(async move {
                match api::fetch_source_detail(id).await {
                    Ok(d) => {
                        set_source_detail.set(Some(d));
                        set_source_detail_error.set(None);
                    }
                    Err(e) => set_source_detail_error.set(Some(e)),
                }
            });
        } else {
            set_source_detail.set(None);
            set_source_detail_error.set(None);
        }
    });

    Effect::new(move |_| {
        let _ = selected_source.get();
        let _ = source_name.get();
        refresh_filesets();
    });

    let on_sync = move |_| {
        let source = source_name.get();
        let year = load_year.get();
        leptos::task::spawn_local(async move {
            match api::sync_catalog(&source, year).await {
                Ok(r) => {
                    set_status_msg.set(format!("Synced {} filesets from Lichess catalog", r.synced))
                }
                Err(e) => set_status_msg.set(format!("Sync failed: {e}")),
            }
            refresh_sources();
            refresh_filesets();
        });
    };

    let on_load = move |_| {
        let source = source_name.get();
        let year = load_year.get();
        leptos::task::spawn_local(async move {
            match api::load_year(&source, year).await {
                Ok(started) => {
                    set_status_msg.set(format!("Ingest job {} started", started.job_id));
                    spawn_job_polling(
                        started.job_id,
                        set_job,
                        set_filesets,
                        set_source_detail,
                        set_sources,
                        set_sources_error,
                        selected_source,
                        source_name,
                        sources,
                    );
                }
                Err(e) => set_status_msg.set(format!("Load failed: {e}")),
            }
        });
    };

    view! {
        <div class="studio">
            <header class="header">
                <Logo/>
                <div class="header-right">
                    <HealthBadge healthy=healthy/>
                    <nav class="nav">
                        <button class:active=move || page.get() == Page::Dashboard
                            on:click=move |_| set_page.set(Page::Dashboard)>"Dashboard"</button>
                        <button class:active=move || page.get() == Page::Games
                            on:click=move |_| set_page.set(Page::Games)>"Games"</button>
                        <button class:active=move || page.get() == Page::Explorer
                            on:click=move |_| set_page.set(Page::Explorer)>"Explorer"</button>
                        <button class:active=move || page.get() == Page::Benchmarks
                            on:click=move |_| set_page.set(Page::Benchmarks)>"Benchmarks"</button>
                    </nav>
                </div>
            </header>

            <main class="main">
                {move || match page.get() {
                    Page::Dashboard => view! {
                        <DashboardPanel
                            sources=sources
                            sources_loading=sources_loading
                            sources_error=sources_error
                            selected_source=selected_source
                            set_selected_source=set_selected_source
                            source_detail=source_detail
                            source_detail_error=source_detail_error
                            filesets=filesets
                            job=job
                            status_msg=status_msg
                            load_year=load_year
                            set_load_year=set_load_year
                            source_name=source_name
                            set_source_name=set_source_name
                            on_sync=on_sync
                            on_load=on_load
                        />
                    }.into_any(),
                    Page::Games => view! {
                        <GamesPanel
                            pending_game_id=pending_game_id
                            pending_ply=pending_ply
                            set_pending_game_id=set_pending_game_id
                            set_pending_ply=set_pending_ply
                        />
                    }.into_any(),
                    Page::Explorer => view! {
                        <ExplorerPanel
                            set_page=set_page
                            set_pending_game_id=set_pending_game_id
                            set_pending_ply=set_pending_ply
                        />
                    }.into_any(),
                    Page::Benchmarks => view! { <BenchPanel/> }.into_any(),
                }}
            </main>
        </div>
    }
}

#[component]
fn JobProgressCard(job: JobStatus, live_source_games: Option<i64>) -> impl IntoView {
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
fn DashboardPanel(
    sources: ReadSignal<Vec<SourceListItem>>,
    sources_loading: ReadSignal<bool>,
    sources_error: ReadSignal<Option<String>>,
    selected_source: ReadSignal<Option<i32>>,
    set_selected_source: WriteSignal<Option<i32>>,
    source_detail: ReadSignal<Option<SourceDetail>>,
    source_detail_error: ReadSignal<Option<String>>,
    filesets: ReadSignal<Vec<FilesetView>>,
    job: ReadSignal<Option<JobStatus>>,
    status_msg: ReadSignal<String>,
    load_year: ReadSignal<i32>,
    set_load_year: WriteSignal<i32>,
    source_name: ReadSignal<String>,
    set_source_name: WriteSignal<String>,
    on_sync: impl Fn(leptos::ev::MouseEvent) + 'static,
    on_load: impl Fn(leptos::ev::MouseEvent) + 'static,
) -> impl IntoView {
    let completed_shards = move || {
        filesets.get().iter().filter(|f| f.status == "complete").count()
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
                    if let Some(err) = sources_error.get() {
                        return view! {
                            <div class="toast">{err.clone()}</div>
                        }.into_any();
                    }
                    let list = sources.get();
                    if list.is_empty() {
                        return view! {
                            <div class="empty-state">
                                <div class="empty-icon">"♟"</div>
                                <p>"No sources yet — sync or load a fileset to begin"</p>
                            </div>
                        }.into_any();
                    }
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
                        {source_detail_error.get().map(|e| view! {
                            <div class="toast">{e}</div>
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
                    <button on:click=on_sync>"Sync catalog"</button>
                    <button class="primary" on:click=on_load>"Load full year"</button>
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
                                            {f.error_message.clone().map(|e| view! {
                                                <small class="shard-error">{e}</small>
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

#[component]
fn GamesPanel(
    pending_game_id: ReadSignal<Option<i64>>,
    pending_ply: ReadSignal<Option<usize>>,
    set_pending_game_id: WriteSignal<Option<i64>>,
    set_pending_ply: WriteSignal<Option<usize>>,
) -> impl IntoView {
    let (player, set_player) = signal(String::new());
    let (games, set_games) = signal(Vec::<GameListItem>::new());
    let (total, set_total) = signal(0_i64);
    let (offset, set_offset) = signal(0_i64);
    let (has_more, set_has_more) = signal(false);
    let (search_loading, set_search_loading) = signal(false);
    let (search_error, set_search_error) = signal(None::<String>);
    let (selected, set_selected) = signal(None::<i64>);
    let (detail, set_detail) = signal(None::<GameDetail>);
    let (game_loading, set_game_loading) = signal(false);
    let (game_error, set_game_error) = signal(None::<String>);
    let (replay_error, set_replay_error) = signal(None::<String>);
    let (load_gen, set_load_gen) = signal(0_u32);
    let (fen, set_fen) = signal(gambit_db_wasm::start_fen());
    let (ply_idx, set_ply_idx) = signal(0_usize);
    let (exploratory, set_exploratory) = signal(false);
    let (last_move, set_last_move) = signal(None::<(String, String)>);
    let (orientation, set_orientation) = signal(BoardOrientation::WhiteBottom);
    let (in_check, set_in_check) = signal(false);
    let (fen_collapsed, set_fen_collapsed) = signal(true);
    let (searched, set_searched) = signal(false);

    let update_check = move |fen_str: &str| {
        if let Ok(pos) = WasmPosition::from_fen(fen_str) {
            set_in_check.set(pos.is_in_check());
        }
    };

    let go_to_ply = move |idx: usize| {
        if let Some(d) = detail.get() {
            match position_at_ply(&d, idx) {
                Ok((pos, last)) => {
                    set_replay_error.set(None);
                    set_fen.set(pos.to_fen());
                    set_ply_idx.set(idx);
                    set_exploratory.set(false);
                    set_last_move.set(last);
                    update_check(&pos.to_fen());
                }
                Err(e) => set_replay_error.set(Some(replay_error_message(&e))),
            }
        }
    };

    let do_search = move |reset: bool| {
        let p = player.get();
        set_searched.set(true);
        set_search_error.set(None);
        let next_offset = if reset { 0 } else { offset.get() };
        if reset {
            set_offset.set(0);
        }
        set_search_loading.set(true);
        leptos::task::spawn_local(async move {
            match api::fetch_games(Some(&p), None, next_offset, GAMES_PAGE_SIZE).await {
                Ok(page) => {
                    if reset {
                        set_games.set(page.games);
                    } else {
                        set_games.update(|g| g.extend(page.games));
                    }
                    set_total.set(page.total);
                    set_has_more.set(page.has_more);
                    set_offset.set(page.offset + page.limit);
                }
                Err(e) => {
                    set_search_error.set(Some(e));
                    if reset {
                        set_games.set(vec![]);
                        set_total.set(0);
                        set_has_more.set(false);
                    }
                }
            }
            set_search_loading.set(false);
        });
    };

    let search = move |_| do_search(true);

    let load_more = move |_| {
        if has_more.get() && !search_loading.get() {
            do_search(false);
        }
    };

    let load_game = move |id: i64, target_ply: Option<usize>| {
        set_load_gen.update(|g| *g += 1);
        let gen = load_gen.get_untracked();
        set_detail.set(None);
        set_selected.set(Some(id));
        set_game_loading.set(true);
        set_game_error.set(None);
        set_replay_error.set(None);
        leptos::task::spawn_local(async move {
            match api::fetch_game(id).await {
                Ok(g) => {
                    if load_gen.get_untracked() != gen {
                        return;
                    }
                    let start = g.start_fen.clone();
                    let mut target_fen = start.clone();
                    let mut target_ply_idx = 0_usize;
                    let mut target_last = None;
                    set_replay_error.set(None);
                    if let Some(ply) = target_ply {
                        match position_at_ply(&g, ply) {
                            Ok((pos, last)) => {
                                target_fen = pos.to_fen();
                                target_ply_idx = ply;
                                target_last = last;
                            }
                            Err(e) => {
                                set_replay_error.set(Some(replay_error_message(&e)));
                            }
                        }
                    }
                    set_fen.set(target_fen.clone());
                    set_ply_idx.set(target_ply_idx);
                    set_exploratory.set(false);
                    set_last_move.set(target_last);
                    set_detail.set(Some(g));
                    update_check(&target_fen);
                }
                Err(e) => {
                    if load_gen.get_untracked() != gen {
                        return;
                    }
                    set_game_error.set(Some(e));
                    set_detail.set(None);
                }
            }
            if load_gen.get_untracked() == gen {
                set_game_loading.set(false);
            }
        });
    };

    Effect::new(move |_| {
        if let Some(id) = pending_game_id.get() {
            let ply = pending_ply.get();
            set_pending_game_id.set(None);
            set_pending_ply.set(None);
            load_game(id, ply);
        }
    });

    let step_back = move || {
        if exploratory.get() {
            go_to_ply(ply_idx.get());
            return;
        }
        let idx = ply_idx.get();
        if idx > 0 {
            go_to_ply(idx - 1);
        } else if let Some(d) = detail.get() {
            set_replay_error.set(None);
            set_fen.set(d.start_fen.clone());
            set_ply_idx.set(0);
            set_exploratory.set(false);
            set_last_move.set(None);
            update_check(&d.start_fen);
        }
    };

    let step_forward = move || {
        if exploratory.get() {
            return;
        }
        if let Some(d) = detail.get() {
            let idx = ply_idx.get();
            if idx < d.plies.len() {
                go_to_ply(idx + 1);
            }
        }
    };

    let flip_board = move || {
        set_orientation.update(|o| *o = o.flip());
    };

    let reset_to_line = move |_| {
        go_to_ply(ply_idx.get());
    };

    let on_board_move = Callback::new(move |uci: String| {
        let fen_val = fen.get_untracked();
        if let Ok(pos) = WasmPosition::from_fen(&fen_val) {
            if let Ok(new_pos) = pos.apply_move(&uci) {
                let new_fen = new_pos.to_fen();
                if let Some(parsed) = parse_uci(&uci) {
                    set_last_move.set(Some((parsed.from, parsed.to)));
                }
                set_fen.set(new_fen.clone());
                set_exploratory.set(true);
                set_in_check.set(new_pos.is_in_check());
            }
        }
    });

    let on_keydown = move |ev: leptos::ev::KeyboardEvent| {
        match ev.key().as_str() {
            "ArrowLeft" => step_back(),
            "ArrowRight" => step_forward(),
            "f" | "F" => flip_board(),
            _ => {}
        }
    };

    view! {
        <section class="panel games-layout" tabindex="0" on:keydown=on_keydown>
            <div class="games-sidebar">
                <h2>"Game search"</h2>
                <p class="panel-desc">"Find games by player name"</p>
                <div class="form-row">
                    <input placeholder="e.g. Carlsen, Nakamura…" prop:value=move || player.get()
                        on:input=move |ev| set_player.set(event_target_value(&ev))
                        on:keydown=move |ev| {
                            if ev.key() == "Enter" { do_search(true); }
                        } />
                    <button on:click=search disabled=move || search_loading.get()>
                        {move || if search_loading.get() {
                            view! { <span><span class="loading-spinner"/>"Search"</span> }.into_any()
                        } else {
                            view! { "Search" }.into_any()
                        }}
                    </button>
                </div>
                <p class="mono" style="margin: 0.5rem 0 0; color: var(--muted); font-size: 0.82rem;">
                    {move || format!("{} results", format_num(total.get()))}
                </p>
                {search_error.get().map(|e| view! {
                    <div class="toast">{e}</div>
                })}
                {move || {
                    if search_loading.get() && games.get().is_empty() {
                        return view! {
                            <div class="empty-state">
                                <span class="loading-spinner"/>
                                <p>"Searching…"</p>
                            </div>
                        }.into_any();
                    }
                    let list = games.get();
                    if list.is_empty() {
                        let msg = if searched.get() {
                            "No games found — try a different name"
                        } else {
                            "Search for a player to browse games"
                        };
                        view! {
                            <div class="empty-state">
                                <div class="empty-icon">"♜"</div>
                                <p>{msg}</p>
                            </div>
                        }.into_any()
                    } else {
                        view! {
                            <ul class="game-list">
                                {list.into_iter().map(|g| {
                                    let id = g.id;
                                    let active = selected.get() == Some(id);
                                    view! {
                                        <li class:active=active on:click=move |_| load_game(id, None)>
                                            <strong>
                                                {format!(
                                                    "{} vs {}",
                                                    g.white.clone().unwrap_or_else(|| "?".into()),
                                                    g.black.clone().unwrap_or_else(|| "?".into())
                                                )}
                                            </strong>
                                            <span>{g.result.clone()}</span>
                                            <span class="mono">{g.game_date.clone().unwrap_or_default()}</span>
                                        </li>
                                    }
                                }).collect_view()}
                            </ul>
                            {has_more.get().then(|| view! {
                                <button class="text-btn" style="width: 100%; margin-top: 0.5rem;"
                                    on:click=load_more disabled=move || search_loading.get()>
                                    {move || if search_loading.get() {
                                        view! { <span><span class="loading-spinner"/>"Loading…"</span> }.into_any()
                                    } else {
                                        view! { "Load more" }.into_any()
                                    }}
                                </button>
                            })}
                        }.into_any()
                    }
                }}
            </div>

            <div class="games-board-hero">
                <div class="board-toolbar">
                    <span class="board-hint">"← → navigate · F flip"</span>
                    <div style="display: flex; gap: 0.5rem;">
                        <button class="icon-btn" on:click=move |_| flip_board() title="Flip board (F)">"⇅"</button>
                        {move || exploratory.get().then(|| view! {
                            <button class="chip-btn" on:click=reset_to_line>"Back to game line"</button>
                        })}
                    </div>
                </div>
                {move || if game_loading.get() {
                    view! {
                        <div class="empty-state" style="min-height: 320px;">
                            <span class="loading-spinner"/>
                            <p>"Loading game…"</p>
                        </div>
                    }.into_any()
                } else {
                    view! {
                        <ChessBoard
                            fen=fen
                            last_move=last_move
                            orientation=orientation
                            in_check=in_check
                            on_move=on_board_move
                            interactive=true
                        />
                    }.into_any()
                }}
                {game_error.get().map(|e| view! {
                    <div class="toast">{e}</div>
                })}
                {replay_error.get().map(|e| view! {
                    <div class="toast">{e}</div>
                })}
                <div class="transport">
                    <button class="icon-btn" on:click=move |_| step_back() title="Previous move (←)">"◀"</button>
                    <span class="mono ply-indicator">
                        {move || {
                            if detail.get().is_some() {
                                format!("ply {} / {}", ply_idx.get(), detail.get().map(|d| d.plies.len()).unwrap_or(0))
                            } else {
                                "—".to_string()
                            }
                        }}
                    </span>
                    <button class="icon-btn" on:click=move |_| step_forward() title="Next move (→)">"▶"</button>
                </div>
                <div class="fen-toggle" style="width: 100%; max-width: 520px; text-align: center;">
                    <button class="text-btn" on:click=move |_| set_fen_collapsed.update(|c| *c = !*c)>
                        {move || if fen_collapsed.get() { "Show FEN" } else { "Hide FEN" }}
                    </button>
                    {move || (!fen_collapsed.get()).then(|| view! {
                        <pre class="fen-display mono">{fen.get()}</pre>
                    })}
                </div>
            </div>

            <div class="games-meta">
                {move || if game_loading.get() {
                    view! {
                        <div class="empty-state">
                            <span class="loading-spinner"/>
                            <p>"Loading game…"</p>
                        </div>
                    }.into_any()
                } else {
                    match detail.get() {
                        Some(d) => view! {
                            <div class="game-info">
                                <h2 class="matchup">
                                    {format!(
                                        "{} vs {}",
                                        d.white.clone().unwrap_or_else(|| "?".into()),
                                        d.black.clone().unwrap_or_else(|| "?".into())
                                    )}
                                </h2>
                                <p class="result-badge">{d.result.clone()}</p>
                                {d.event.clone().map(|e| view! { <p class="event">{e}</p> })}
                            </div>
                            <h3>"Move list"</h3>
                            <ul class="movetext">
                                <li
                                    class:active=move || ply_idx.get() == 0 && !exploratory.get()
                                    on:click=move |_| go_to_ply(0)
                                >
                                    <span class="move-num">"0"</span>
                                    <span>"Start position"</span>
                                </li>
                                {d.plies.iter().enumerate().map(|(i, p)| {
                                    let ply_num = i + 1;
                                    view! {
                                        <li
                                            class:active=move || ply_idx.get() == ply_num && !exploratory.get()
                                            on:click=move |_| go_to_ply(ply_num)
                                        >
                                            <span class="move-num">{ply_num}</span>
                                            <span>{p.san.clone()}</span>
                                        </li>
                                    }
                                }).collect_view()}
                            </ul>
                        }.into_any(),
                        None => view! {
                            <div class="empty-state">
                                <div class="empty-icon">"♞"</div>
                                <p>"Select a game to replay moves on the board"</p>
                            </div>
                        }.into_any(),
                    }
                }}
            </div>
        </section>
    }
}

#[component]
fn ExplorerPanel(
    set_page: WriteSignal<Page>,
    set_pending_game_id: WriteSignal<Option<i64>>,
    set_pending_ply: WriteSignal<Option<usize>>,
) -> impl IntoView {
    let (fen, set_fen) = signal(gambit_db_wasm::start_fen());
    let (last_move, set_last_move) = signal(None::<(String, String)>);
    let (orientation, set_orientation) = signal(BoardOrientation::WhiteBottom);
    let (in_check, set_in_check) = signal(false);
    let (position_hash, set_position_hash) = signal(None::<i64>);
    let (hash_error, set_hash_error) = signal(None::<String>);
    let (opening_stats, set_opening_stats) = signal(Vec::<OpeningMoveStat>::new());
    let (stats_loading, set_stats_loading) = signal(false);
    let (stats_error, set_stats_error) = signal(None::<String>);
    let (pos_games, set_pos_games) = signal(Vec::<PositionHit>::new());
    let (pos_total, set_pos_total) = signal(0_i64);
    let (pos_offset, set_pos_offset) = signal(0_i64);
    let (pos_has_more, set_pos_has_more) = signal(false);
    let (pos_loading, set_pos_loading) = signal(false);
    let (pos_error, set_pos_error) = signal(None::<String>);
    let (fen_collapsed, set_fen_collapsed) = signal(false);

    let update_check = move |fen_str: &str| {
        if let Ok(pos) = WasmPosition::from_fen(fen_str) {
            set_in_check.set(pos.is_in_check());
        }
    };

    Effect::new(move |_| {
        let f = fen.get();
        set_pos_offset.set(0);
        set_hash_error.set(None);
        if let Some(h) = fen_hash_local(&f) {
            set_position_hash.set(Some(h));
        } else {
            set_position_hash.set(None);
            leptos::task::spawn_local(async move {
                match resolve_position_hash(&f).await {
                    Ok(h) => set_position_hash.set(Some(h)),
                    Err(e) => {
                        set_hash_error.set(Some(e));
                        set_position_hash.set(None);
                    }
                }
            });
        }
    });

    Effect::new(move |_| {
        if let Some(h) = position_hash.get() {
            set_stats_loading.set(true);
            set_stats_error.set(None);
            leptos::task::spawn_local(async move {
                match api::fetch_opening_stats(h).await {
                    Ok(stats) => {
                        set_opening_stats.set(stats);
                        set_stats_error.set(None);
                    }
                    Err(e) => {
                        set_opening_stats.set(vec![]);
                        set_stats_error.set(Some(e));
                    }
                }
                set_stats_loading.set(false);
            });
        } else {
            set_opening_stats.set(vec![]);
        }
    });

    Effect::new(move |_| {
        let h = position_hash.get();
        let offset = pos_offset.get();
        if let Some(hash) = h {
            set_pos_loading.set(true);
            set_pos_error.set(None);
            leptos::task::spawn_local(async move {
                match api::fetch_games_by_position(hash, offset, EXPLORER_PAGE_SIZE).await {
                    Ok(page) => {
                        if offset == 0 {
                            set_pos_games.set(page.hits);
                        } else {
                            set_pos_games.update(|g| g.extend(page.hits));
                        }
                        set_pos_total.set(page.total);
                        set_pos_has_more.set(page.has_more);
                    }
                    Err(e) => {
                        set_pos_error.set(Some(e));
                        if offset == 0 {
                            set_pos_games.set(vec![]);
                            set_pos_total.set(0);
                            set_pos_has_more.set(false);
                        }
                    }
                }
                set_pos_loading.set(false);
            });
        } else {
            set_pos_games.set(vec![]);
            set_pos_total.set(0);
            set_pos_has_more.set(false);
        }
    });

    let on_board_move = Callback::new(move |uci: String| {
        let fen_val = fen.get_untracked();
        if let Ok(pos) = WasmPosition::from_fen(&fen_val) {
            if let Ok(new_pos) = pos.apply_move(&uci) {
                let new_fen = new_pos.to_fen();
                if let Some(parsed) = parse_uci(&uci) {
                    set_last_move.set(Some((parsed.from, parsed.to)));
                }
                set_fen.set(new_fen);
                set_in_check.set(new_pos.is_in_check());
            }
        }
    });

    let apply_stat_move = move |uci: String| {
        let fen_val = fen.get_untracked();
        if let Ok(pos) = WasmPosition::from_fen(&fen_val) {
            if let Ok(new_pos) = pos.apply_move(&uci) {
                let new_fen = new_pos.to_fen();
                if let Some(parsed) = parse_uci(&uci) {
                    set_last_move.set(Some((parsed.from, parsed.to)));
                }
                set_fen.set(new_fen);
                set_in_check.set(new_pos.is_in_check());
            }
        }
    };

    let reset_position = move |_| {
        let start = gambit_db_wasm::start_fen();
        set_fen.set(start.clone());
        set_last_move.set(None);
        update_check(&start);
    };

    let flip_board = move |_| {
        set_orientation.update(|o| *o = o.flip());
    };

    let load_more_games = move |_| {
        if pos_has_more.get() && !pos_loading.get() {
            set_pos_offset.update(|o| *o += EXPLORER_PAGE_SIZE);
        }
    };

    let jump_to_game = move |hit: PositionHit| {
        set_pending_game_id.set(Some(hit.game_id));
        set_pending_ply.set(Some(hit.ply as usize));
        set_page.set(Page::Games);
    };

    view! {
        <section class="panel games-layout">
            <div class="games-sidebar">
                <h2>"Opening stats"</h2>
                <p class="panel-desc">"Moves from this position in the database"</p>
                {position_hash.get().map(|h| view! {
                    <p class="mono" style="margin: 0 0 0.75rem; color: var(--muted); font-size: 0.75rem;">
                        {format!("hash {h}")}</p>
                })}
                {hash_error.get().map(|e| view! { <div class="toast">{e}</div> })}
                {stats_error.get().map(|e| view! { <div class="toast">{e}</div> })}
                {move || {
                    if stats_loading.get() && opening_stats.get().is_empty() {
                        return view! {
                            <div class="empty-state">
                                <span class="loading-spinner"/>
                                <p>"Loading stats…"</p>
                            </div>
                        }.into_any();
                    }
                    let stats = opening_stats.get();
                    if stats.is_empty() {
                        return view! {
                            <div class="empty-state">
                                <div class="empty-icon">"♟"</div>
                                <p>"No database moves at this position yet"</p>
                            </div>
                        }.into_any();
                    }
                    view! {
                        <table class="bench-table">
                            <thead>
                                <tr>
                                    <th>"Move"</th>
                                    <th>"Games"</th>
                                    <th>"W"</th>
                                    <th>"D"</th>
                                    <th>"B"</th>
                                </tr>
                            </thead>
                            <tbody>
                                {stats.into_iter().map(|row| {
                                    let uci = row.move_uci.clone();
                                    view! {
                                        <tr style="cursor: pointer;" on:click=move |_| apply_stat_move(uci.clone())>
                                            <td class="mono">{row.move_uci.clone()}</td>
                                            <td class="mono">{format_num(row.count)}</td>
                                            <td class="mono">{format_num(row.white_wins)}</td>
                                            <td class="mono">{format_num(row.draws)}</td>
                                            <td class="mono">{format_num(row.black_wins)}</td>
                                        </tr>
                                    }
                                }).collect_view()}
                            </tbody>
                        </table>
                    }.into_any()
                }}
            </div>

            <div class="games-board-hero">
                <div class="board-toolbar">
                    <span class="board-hint">"Click squares to explore · stats rows apply moves"</span>
                    <div style="display: flex; gap: 0.5rem;">
                        <button class="icon-btn" on:click=reset_position title="Start position">"⌂"</button>
                        <button class="icon-btn" on:click=flip_board title="Flip board">"⇅"</button>
                    </div>
                </div>
                <ChessBoard
                    fen=fen
                    last_move=last_move
                    orientation=orientation
                    in_check=in_check
                    on_move=on_board_move
                    interactive=true
                />
                <div class="fen-toggle" style="width: 100%; max-width: 520px; text-align: center;">
                    <button class="text-btn" on:click=move |_| set_fen_collapsed.update(|c| *c = !*c)>
                        {move || if fen_collapsed.get() { "Show FEN" } else { "Hide FEN" }}
                    </button>
                    {move || (!fen_collapsed.get()).then(|| view! {
                        <pre class="fen-display mono">{fen.get()}</pre>
                    })}
                </div>
            </div>

            <div class="games-meta">
                <h2>"Games at position"</h2>
                <p class="panel-desc mono" style="margin-bottom: 0.75rem;">
                    {move || format!("{} games", format_num(pos_total.get()))}
                </p>
                {pos_error.get().map(|e| view! { <div class="toast">{e}</div> })}
                {move || {
                    if pos_loading.get() && pos_games.get().is_empty() {
                        return view! {
                            <div class="empty-state">
                                <span class="loading-spinner"/>
                                <p>"Loading games…"</p>
                            </div>
                        }.into_any();
                    }
                    let hits = pos_games.get();
                    if hits.is_empty() {
                        return view! {
                            <div class="empty-state">
                                <div class="empty-icon">"♜"</div>
                                <p>"No games reach this position in the database"</p>
                            </div>
                        }.into_any();
                    }
                    view! {
                        <ul class="game-list">
                            {hits.into_iter().map(|hit| {
                                view! {
                                    <li on:click=move |_| jump_to_game(hit.clone())>
                                        <strong>
                                            {format!(
                                                "{} vs {}",
                                                hit.white.clone().unwrap_or_else(|| "?".into()),
                                                hit.black.clone().unwrap_or_else(|| "?".into())
                                            )}
                                        </strong>
                                        <span class="mono">{format!("ply {}", hit.ply)}</span>
                                    </li>
                                }
                            }).collect_view()}
                        </ul>
                        {pos_has_more.get().then(|| view! {
                            <button class="text-btn" style="width: 100%; margin-top: 0.5rem;"
                                on:click=load_more_games disabled=move || pos_loading.get()>
                                {move || if pos_loading.get() {
                                    view! { <span><span class="loading-spinner"/>"Loading…"</span> }.into_any()
                                } else {
                                    view! { "Load more" }.into_any()
                                }}
                            </button>
                        })}
                    }.into_any()
                }}
            </div>
        </section>
    }
}

#[component]
fn BenchPanel() -> impl IntoView {
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
