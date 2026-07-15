//! Job streaming and dashboard helpers.

use crate::api;
use gambit_proto::{FilesetView, JobStatus, SourceDetail, SourceListItem};
use leptos::prelude::*;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

const SIDE_REFRESH_INTERVAL: Duration = Duration::from_secs(3);

pub fn parse_download_progress(msg: &str) -> Option<(f64, f64)> {
    let open = msg.find('(')?;
    let close = msg.find(')')?;
    let inner = msg.get(open + 1..close)?;
    let mut parts = inner.split('/');
    let current: f64 = parts.next()?.split_whitespace().next()?.parse().ok()?;
    let total: f64 = parts.next()?.split_whitespace().next()?.parse().ok()?;
    Some((current, total))
}

pub fn filesets_query_name(
    selected: Option<i32>,
    sources: &[SourceListItem],
    load_form_name: &str,
) -> String {
    selected
        .and_then(|id| sources.iter().find(|s| s.id == id).map(|s| s.name.clone()))
        .unwrap_or_else(|| load_form_name.to_string())
}

#[allow(clippy::too_many_arguments)]
pub fn spawn_job_watching(
    job_id: u64,
    reconstructed: bool,
    poll_source: String,
    poll_year: i32,
    set_job: WriteSignal<Option<JobStatus>>,
    set_filesets: WriteSignal<Vec<FilesetView>>,
    set_source_detail: WriteSignal<Option<SourceDetail>>,
    set_sources: WriteSignal<Vec<SourceListItem>>,
    set_job_error: WriteSignal<Option<String>>,
    selected_source: ReadSignal<Option<i32>>,
    source_name: ReadSignal<String>,
    sources: ReadSignal<Vec<SourceListItem>>,
) {
    leptos::task::spawn_local(async move {
        let last_refresh = Rc::new(Cell::new(Instant::now() - SIDE_REFRESH_INTERVAL));
        let prev_status = Rc::new(RefCell::new(String::new()));
        let prev_shard = Rc::new(Cell::new(0u32));

        let source_name_for_stream = if reconstructed || job_id == 0 {
            Some(poll_source.clone())
        } else {
            None
        };
        let year_for_stream = if reconstructed || job_id == 0 {
            Some(poll_year)
        } else {
            None
        };

        let refresh_side_data = |status: &JobStatus, force: bool| {
            let changed = status.status != *prev_status.borrow()
                || status.current_shard != Cell::get(prev_shard.as_ref());
            let due = last_refresh.get().elapsed() >= SIDE_REFRESH_INTERVAL;
            if !force && !changed && !due {
                return;
            }
            last_refresh.set(Instant::now());
            *prev_status.borrow_mut() = status.status.clone();
            prev_shard.set(status.current_shard);

            let name = filesets_query_name(
                selected_source.get_untracked(),
                &sources.get_untracked(),
                &source_name.get_untracked(),
            );
            leptos::task::spawn_local(async move {
                if let Ok(list) = api::fetch_filesets_by_name(&name).await {
                    set_filesets.set(list);
                }
            });

            let terminal = status.status == "complete" || status.status == "failed";
            if terminal {
                if let Some(id) = selected_source.get_untracked() {
                    leptos::task::spawn_local(async move {
                        if let Ok(detail) = api::fetch_source_detail(id).await {
                            set_source_detail.set(Some(detail));
                        }
                    });
                }
            }
            let _ = set_sources;
        };

        let result = api::watch_job(job_id, source_name_for_stream, year_for_stream, |status| {
            let changed = *prev_status.borrow() != status.status
                || Cell::get(prev_shard.as_ref()) != status.current_shard
                || status.games_loaded > 0;
            set_job.set(Some(status.clone()));
            refresh_side_data(&status, changed);
            status.status != "complete" && status.status != "failed"
        })
        .await;

        if let Err(e) = result {
            set_job_error.set(Some(e));
        }
    });
}
