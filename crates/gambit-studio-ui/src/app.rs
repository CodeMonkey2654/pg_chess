//! Gambit Studio UI shell.

use crate::api;
use crate::brand::{HealthBadge, Logo};
use crate::job_poll::{filesets_query_name, spawn_job_watching};
use crate::pages::{BenchPanel, DashboardPanel, ExplorerPanel, GamesPanel, Page};
use gambit_proto::{FilesetView, JobStatus, SourceDetail, SourceListItem};
use leptos::prelude::*;

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
    let (job_error, set_job_error) = signal(None::<String>);
    let (status_msg, set_status_msg) = signal(String::new());
    let (sync_loading, set_sync_loading) = signal(false);
    let (load_loading, set_load_loading) = signal(false);
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
                Ok(Some(j)) if j.status == "running" => {
                    set_job.set(Some(j.clone()));
                    spawn_job_watching(
                        j.id,
                        j.id == 0,
                        sn,
                        yr,
                        set_job,
                        set_filesets,
                        set_source_detail,
                        set_sources,
                        set_job_error,
                        selected_source,
                        source_name,
                        sources,
                    );
                }
                Ok(Some(j)) => set_job.set(Some(j)),
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
        set_sync_loading.set(true);
        leptos::task::spawn_local(async move {
            match api::sync_catalog(&source, year).await {
                Ok(r) => {
                    set_status_msg.set(format!("Synced {} filesets from Lichess catalog", r.synced))
                }
                Err(e) => set_status_msg.set(format!("Sync failed: {e}")),
            }
            refresh_sources();
            refresh_filesets();
            set_sync_loading.set(false);
        });
    };

    let on_load = move |_| {
        let source = source_name.get();
        let year = load_year.get();
        set_load_loading.set(true);
        leptos::task::spawn_local(async move {
            match api::load_year(&source, year).await {
                Ok(started) => {
                    set_status_msg.set(format!("Ingest job {} started", started.job_id));
                    spawn_job_watching(
                        started.job_id,
                        false,
                        source.clone(),
                        year,
                        set_job,
                        set_filesets,
                        set_source_detail,
                        set_sources,
                        set_job_error,
                        selected_source,
                        source_name,
                        sources,
                    );
                }
                Err(e) => set_status_msg.set(format!("Load failed: {e}")),
            }
            set_load_loading.set(false);
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
                <div class="page-panel" class:hidden=move || page.get() != Page::Dashboard>
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
                        job_error=job_error
                        status_msg=status_msg
                        load_year=load_year
                        set_load_year=set_load_year
                        source_name=source_name
                        set_source_name=set_source_name
                        on_sync=on_sync
                        on_load=on_load
                        sync_loading=sync_loading
                        load_loading=load_loading
                    />
                </div>
                <div class="page-panel" class:hidden=move || page.get() != Page::Games>
                    <GamesPanel
                        selected_source=selected_source
                        pending_game_id=pending_game_id
                        pending_ply=pending_ply
                        set_pending_game_id=set_pending_game_id
                        set_pending_ply=set_pending_ply
                    />
                </div>
                <div class="page-panel" class:hidden=move || page.get() != Page::Explorer>
                    <ExplorerPanel
                        set_page=set_page
                        set_pending_game_id=set_pending_game_id
                        set_pending_ply=set_pending_ply
                    />
                </div>
                <div class="page-panel" class:hidden=move || page.get() != Page::Benchmarks>
                    <BenchPanel/>
                </div>
            </main>
        </div>
    }
}
