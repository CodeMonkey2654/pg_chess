//! Games browser and replay page.

use super::util::{
    event_target_value, format_accuracy, move_class_css, move_class_label, replay_error_message,
};
use super::GAMES_PAGE_SIZE;
use crate::api;
use crate::board::uci::parse_uci;
use crate::board::{BoardOrientation, ChessBoard};
use crate::format::format_num;
use crate::replay::position_at_ply;
use gambit_db_wasm::WasmPosition;
use gambit_proto::{GameDetail, GameListItem};
use leptos::prelude::*;

#[component]
pub fn GamesPanel(
    selected_source: ReadSignal<Option<i32>>,
    pending_game_id: ReadSignal<Option<i64>>,
    pending_ply: ReadSignal<Option<usize>>,
    set_pending_game_id: WriteSignal<Option<i64>>,
    set_pending_ply: WriteSignal<Option<usize>>,
) -> impl IntoView {
    let (player, set_player) = signal(String::new());
    let (games, set_games) = signal(Vec::<GameListItem>::new());
    let (total, set_total) = signal(0_i64);
    let (offset, set_offset) = signal(0_i64);
    let (search_cursor, set_search_cursor) = signal(None::<String>);
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
    let (analyze_loading, set_analyze_loading) = signal(false);
    let (analyze_error, set_analyze_error) = signal(None::<String>);

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
        let cursor_owned = if reset {
            None
        } else {
            search_cursor.get_untracked()
        };
        if reset {
            set_offset.set(0);
            set_search_cursor.set(None);
        }
        let sid = selected_source.get_untracked();
        set_search_loading.set(true);
        leptos::task::spawn_local(async move {
            match api::fetch_games(
                Some(&p),
                sid,
                next_offset,
                GAMES_PAGE_SIZE,
                cursor_owned.as_deref(),
            )
            .await {
                Ok(page) => {
                    if reset {
                        set_games.set(page.games);
                    } else {
                        set_games.update(|g| {
                            g.extend(page.games);
                            if g.len() > 200 {
                                g.drain(0..g.len() - 200);
                            }
                        });
                    }
                    set_total.set(page.total);
                    set_has_more.set(page.has_more);
                    set_search_cursor.set(page.next_cursor);
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

    let on_keydown = move |ev: leptos::ev::KeyboardEvent| match ev.key().as_str() {
        "ArrowLeft" => step_back(),
        "ArrowRight" => step_forward(),
        "f" | "F" => flip_board(),
        _ => {}
    };

    let run_analyze = move |_| {
        let id = match selected.get() {
            Some(id) => id,
            None => return,
        };
        set_analyze_loading.set(true);
        set_analyze_error.set(None);
        leptos::task::spawn_local(async move {
            match api::analyze_game(id, 12).await {
                Ok(_) => load_game(id, None),
                Err(e) => set_analyze_error.set(Some(e)),
            }
            set_analyze_loading.set(false);
        });
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
                    {move || {
                        let t = total.get();
                        if t < 0 {
                            format!("{}+ results", format_num(games.get().len() as i64))
                        } else {
                            format!("{} results", format_num(t))
                        }
                    }}
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
                {move || {
                    let eval_cp = detail.get().and_then(|d| {
                        let idx = ply_idx.get();
                        if idx == 0 {
                            d.plies.first().and_then(|p| p.eval_before)
                        } else {
                            d.plies.get(idx.saturating_sub(1)).and_then(|p| p.eval_after)
                        }
                    });
                    eval_cp.map(|cp| {
                        let pct = ((cp as f32 + 800.0) / 1600.0).clamp(0.0, 1.0) * 100.0;
                        view! {
                            <div class="eval-bar">
                                <div class="eval-bar-black" style:width=format!("{:.1}%", 100.0 - pct)></div>
                                <span class="eval-label mono">{format!("{cp:+}")}</span>
                            </div>
                        }
                    })
                }}
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
                                {d.analysis.clone().map(|a| view! {
                                    <div class="analysis-summary">
                                        <p class="mono">
                                            "Accuracy: "
                                            <span class="acc-white">{format_accuracy(a.accuracy_white)}</span>
                                            " / "
                                            <span class="acc-black">{format_accuracy(a.accuracy_black)}</span>
                                        </p>
                                        <p class="mono" style="color: var(--muted); font-size: 0.82rem;">
                                            {format!("Status: {} · Blunders: {} / {}",
                                                a.status,
                                                a.blunders_white.unwrap_or(0),
                                                a.blunders_black.unwrap_or(0)
                                            )}
                                        </p>
                                    </div>
                                })}
                                <button class="chip-btn" style="margin-bottom: 0.75rem;"
                                    on:click=run_analyze
                                    disabled=move || analyze_loading.get() || selected.get().is_none()>
                                    {move || if analyze_loading.get() {
                                        view! { <span><span class="loading-spinner"/>"Analyzing…"</span> }.into_any()
                                    } else {
                                        view! { "Analyze game" }.into_any()
                                    }}
                                </button>
                                {analyze_error.get().map(|e| view! {
                                    <div class="toast">{e}</div>
                                })}
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
                                    let class_name = p.move_class.as_deref().map(move_class_css).unwrap_or("");
                                    let class_label = p.move_class.as_deref().map(move_class_label).unwrap_or("");
                                    view! {
                                        <li
                                            class:active=move || ply_idx.get() == ply_num && !exploratory.get()
                                            class=class_name
                                            on:click=move |_| go_to_ply(ply_num)
                                        >
                                            <span class="move-num">{ply_num}</span>
                                            <span>{p.san.clone()}</span>
                                            {(!class_label.is_empty()).then(|| view! {
                                                <span class="move-tag">{class_label}</span>
                                            })}
                                            {p.cp_loss.map(|loss| view! {
                                                <span class="move-loss mono">{format!("+{loss}")}</span>
                                            })}
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
