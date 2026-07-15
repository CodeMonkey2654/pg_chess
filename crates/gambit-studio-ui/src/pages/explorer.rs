//! Opening explorer page.

use super::util::{fen_hash_local, resolve_position_hash};
use super::{Page, EXPLORER_PAGE_SIZE};
use crate::api;
use crate::board::uci::parse_uci;
use crate::board::{BoardOrientation, ChessBoard};
use crate::format::format_num;
use gambit_db_wasm::WasmPosition;
use gambit_proto::{OpeningMoveStat, PositionHit};
use leptos::prelude::*;

#[component]
pub fn ExplorerPanel(
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
    let (debounced_fen, set_debounced_fen) = signal(gambit_db_wasm::start_fen());
    let (debounce_seq, set_debounce_seq) = signal(0_u32);
    let (fetch_gen, set_fetch_gen) = signal(0_u32);
    let (pos_eval_cp, set_pos_eval_cp) = signal(None::<i32>);
    let (pos_eval_source, set_pos_eval_source) = signal(None::<String>);
    let (eval_loading, set_eval_loading) = signal(false);
    let (eval_error, set_eval_error) = signal(None::<String>);

    let update_check = move |fen_str: &str| {
        if let Ok(pos) = WasmPosition::from_fen(fen_str) {
            set_in_check.set(pos.is_in_check());
        }
    };

    Effect::new(move |_| {
        let f = fen.get();
        set_debounce_seq.update(|s| *s += 1);
        let seq = debounce_seq.get_untracked();
        leptos::task::spawn_local(async move {
            gloo_timers::future::TimeoutFuture::new(300).await;
            if debounce_seq.get_untracked() == seq {
                set_debounced_fen.set(f);
            }
        });
    });

    Effect::new(move |_| {
        let f = debounced_fen.get();
        set_pos_offset.set(0);
        set_hash_error.set(None);
        set_fetch_gen.update(|g| *g += 1);
        if let Some(h) = fen_hash_local(&f) {
            set_position_hash.set(Some(h));
        } else {
            set_position_hash.set(None);
            let gen = fetch_gen.get_untracked();
            leptos::task::spawn_local(async move {
                match resolve_position_hash(&f).await {
                    Ok(h) => {
                        if fetch_gen.get_untracked() == gen {
                            set_position_hash.set(Some(h));
                        }
                    }
                    Err(e) => {
                        if fetch_gen.get_untracked() == gen {
                            set_hash_error.set(Some(e));
                            set_position_hash.set(None);
                        }
                    }
                }
            });
        }
    });

    Effect::new(move |_| {
        let h = position_hash.get();
        let f = debounced_fen.get();
        if let Some(hash) = h {
            let gen = fetch_gen.get_untracked();
            set_stats_loading.set(true);
            set_stats_error.set(None);
            set_eval_loading.set(true);
            set_eval_error.set(None);
            leptos::task::spawn_local(async move {
                let stats_fut = api::fetch_opening_stats(hash);
                let eval_fut = api::fetch_position_eval(&f, hash, 10);
                let (stats_res, eval_res) = futures::join!(stats_fut, eval_fut);
                if fetch_gen.get_untracked() != gen {
                    return;
                }
                match stats_res {
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
                match eval_res {
                    Ok(ev) => {
                        set_pos_eval_cp.set(Some(ev.eval_cp));
                        set_pos_eval_source.set(Some(ev.source));
                        set_eval_error.set(None);
                    }
                    Err(e) => set_eval_error.set(Some(e)),
                }
                set_eval_loading.set(false);
            });
        } else {
            set_opening_stats.set(vec![]);
            set_pos_eval_cp.set(None);
            set_pos_eval_source.set(None);
        }
    });

    Effect::new(move |_| {
        let h = position_hash.get();
        let offset = pos_offset.get();
        if let Some(hash) = h {
            let gen = fetch_gen.get_untracked();
            set_pos_loading.set(true);
            set_pos_error.set(None);
            leptos::task::spawn_local(async move {
                match api::fetch_games_by_position(hash, offset, EXPLORER_PAGE_SIZE, None).await {
                    Ok(page) => {
                        if fetch_gen.get_untracked() != gen {
                            return;
                        }
                        if offset == 0 {
                            set_pos_games.set(page.hits);
                        } else {
                            set_pos_games.update(|g| g.extend(page.hits));
                        }
                        set_pos_total.set(page.total);
                        set_pos_has_more.set(page.has_more);
                    }
                    Err(e) => {
                        if fetch_gen.get_untracked() != gen {
                            return;
                        }
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
                {move || {
                    if eval_loading.get() && pos_eval_cp.get().is_none() {
                        return view! {
                            <div class="eval-bar" style="opacity: 0.5;">
                                <span class="eval-label mono">"eval…"</span>
                            </div>
                        }.into_any();
                    }
                    pos_eval_cp.get().map(|cp| {
                        let pct = ((cp as f64 + 500.0) / 1000.0 * 100.0).clamp(5.0, 95.0);
                        let src = pos_eval_source.get().unwrap_or_default();
                        view! {
                            <div class="eval-bar">
                                <div class="eval-bar-black" style:width=format!("{:.1}%", 100.0 - pct)></div>
                                <span class="eval-label mono">{format!("{cp:+} · {src}")}</span>
                            </div>
                        }
                    }).into_any()
                }}
                {eval_error.get().map(|e| view! { <div class="toast error">{e}</div> })}
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
