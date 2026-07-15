//! Interactive animated chess board.

use super::fen::{
    algebraic_to_index, fen_to_squares, index_to_algebraic, is_white_piece, king_square,
};
use super::piece::PieceSvg;
use super::uci::{castling_rook_move, matching_uci, parse_uci, promotion_options};
use gambit_db_wasm::WasmPosition;
use leptos::prelude::*;
use wasm_bindgen::JsCast;

#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum BoardOrientation {
    #[default]
    WhiteBottom,
    BlackBottom,
}

impl BoardOrientation {
    pub fn flip(self) -> Self {
        match self {
            Self::WhiteBottom => Self::BlackBottom,
            Self::BlackBottom => Self::WhiteBottom,
        }
    }

    fn white_bottom(self) -> bool {
        matches!(self, Self::WhiteBottom)
    }
}

#[derive(Clone)]
struct ActiveAnimation {
    uci: String,
    piece: char,
    rook: Option<(String, String, char)>,
}

/// Visual 8×8 board with animations and interactive play.
#[component]
pub fn ChessBoard(
    fen: ReadSignal<String>,
    last_move: ReadSignal<Option<(String, String)>>,
    orientation: ReadSignal<BoardOrientation>,
    in_check: ReadSignal<bool>,
    on_move: Callback<String>,
    interactive: bool,
) -> impl IntoView {
    let (selected, set_selected) = signal(None::<String>);
    let (legal_targets, set_legal_targets) = signal(Vec::<String>::new());
    let (promotion_pick, set_promotion_pick) = signal(None::<(String, String, Vec<char>)>);
    let (animation, set_animation) = signal(None::<ActiveAnimation>);
    let (shake, set_shake) = signal(false);
    let (dragging, set_dragging) = signal(None::<(String, char)>);
    let (did_drag, set_did_drag) = signal(false);

    // Trigger animation when last_move changes.
    Effect::new(move |_| {
        let fen_val = fen.get();
        let lm = last_move.get();
        if let Some((from, to)) = lm {
            let squares = fen_to_squares(&fen_val);
            let from_idx = algebraic_to_index(&from);
            let piece = from_idx.and_then(|i| squares[i]);
            if let Some(p) = piece {
                let uci = format!("{from}{to}");
                let rook = castling_rook_move(&from, &to).and_then(|(rf, rt)| {
                    let rf_idx = algebraic_to_index(&rf)?;
                    let rook_piece = squares[rf_idx]?;
                    Some((rf, rt, rook_piece))
                });
                set_animation.set(Some(ActiveAnimation {
                    uci,
                    piece: p,
                    rook,
                }));
                let set_anim = set_animation;
                leptos::task::spawn_local(async move {
                    gloo_timers::future::TimeoutFuture::new(200).await;
                    set_anim.set(None);
                });
            }
        }
    });

    let update_legal_targets = move |sq: &str, fen_val: &str| {
        if let Ok(pos) = WasmPosition::from_fen(fen_val) {
            let side = pos.side_to_move();
            let squares = fen_to_squares(fen_val);
            if let Some(idx) = algebraic_to_index(sq) {
                if let Some(p) = squares[idx] {
                    let is_white = is_white_piece(p);
                    if (side == "white" && is_white) || (side == "black" && !is_white) {
                        let moves = pos.legal_moves();
                        let targets: Vec<String> = moves
                            .iter()
                            .filter(|m| m.starts_with(sq))
                            .map(|m| m[2..4].to_string())
                            .collect();
                        set_legal_targets.set(targets);
                        set_selected.set(Some(sq.to_string()));
                        return;
                    }
                }
            }
        }
        set_selected.set(None);
        set_legal_targets.set(vec![]);
    };

    let try_move = move |from: &str, to: &str, promo: Option<char>| {
        let fen_val = fen.get_untracked();
        let Ok(pos) = WasmPosition::from_fen(&fen_val) else {
            return;
        };
        let legal = pos.legal_moves();
        let promos = promotion_options(&legal, from, to);
        let promo = if promos.len() > 1 && promo.is_none() {
            set_promotion_pick.set(Some((from.to_string(), to.to_string(), promos)));
            set_selected.set(None);
            set_legal_targets.set(vec![]);
            return;
        } else if promos.len() == 1 && promo.is_none() {
            Some(promos[0])
        } else {
            promo
        };
        if let Some(uci) = matching_uci(&legal, from, to, promo) {
            set_promotion_pick.set(None);
            set_selected.set(None);
            set_legal_targets.set(vec![]);
            on_move.run(uci);
        } else {
            set_shake.set(true);
            let set_sh = set_shake;
            leptos::task::spawn_local(async move {
                gloo_timers::future::TimeoutFuture::new(300).await;
                set_sh.set(false);
            });
        }
    };

    let on_square_click = move |sq: String| {
        if !interactive {
            return;
        }
        if did_drag.get_untracked() {
            set_did_drag.set(false);
            return;
        }
        if promotion_pick.get_untracked().is_some() {
            return;
        }
        let fen_val = fen.get_untracked();
        if let Some(sel) = selected.get_untracked() {
            if sel == sq {
                set_selected.set(None);
                set_legal_targets.set(vec![]);
                return;
            }
            try_move(&sel, &sq, None);
        } else {
            update_legal_targets(&sq, &fen_val);
        }
    };

    let on_promo_click = move |piece: char| {
        if let Some((from, to, _)) = promotion_pick.get_untracked() {
            try_move(&from, &to, Some(piece));
        }
    };

    let square_from_event = |ev: &leptos::ev::MouseEvent| -> Option<String> {
        let mut el = ev
            .target()
            .and_then(|t| t.dyn_into::<web_sys::Element>().ok());
        while let Some(node) = el {
            if let Some(sq) = node.get_attribute("data-square") {
                return Some(sq);
            }
            el = node.parent_element();
        }
        None
    };

    let on_mouse_down = move |ev: leptos::ev::MouseEvent| {
        if !interactive {
            return;
        }
        let fen_val = fen.get_untracked();
        if let Some(sq) = square_from_event(&ev) {
            let squares = fen_to_squares(&fen_val);
            if let Some(idx) = algebraic_to_index(&sq) {
                if let Some(p) = squares[idx] {
                    if let Ok(pos) = WasmPosition::from_fen(&fen_val) {
                        let side = pos.side_to_move();
                        let is_white = is_white_piece(p);
                        if (side == "white" && is_white) || (side == "black" && !is_white) {
                            set_dragging.set(Some((sq.clone(), p)));
                            update_legal_targets(&sq, &fen_val);
                            ev.prevent_default();
                        }
                    }
                }
            }
        }
    };

    let on_mouse_up = move |ev: leptos::ev::MouseEvent| {
        if let Some((from, _)) = dragging.get_untracked() {
            set_dragging.set(None);
            if let Some(to) = square_from_event(&ev) {
                if from != to {
                    set_did_drag.set(true);
                    try_move(&from, &to, None);
                }
            }
        }
    };

    view! {
        <div
            class="board-wrap"
            class:board-shake=move || shake.get()
            on:mousedown=on_mouse_down
            on:mouseup=on_mouse_up
        >
            <div class="board-grid">
                {move || {
                    let fen_val = fen.get();
                    let orient = orientation.get();
                    let squares = fen_to_squares(&fen_val);
                    let sel = selected.get();
                    let targets = legal_targets.get();
                    let lm = last_move.get();
                    let anim = animation.get();
                    let side = WasmPosition::from_fen(&fen_val)
                        .map(|p| p.side_to_move())
                        .unwrap_or_else(|_| "white".to_string());
                    let king_sq = king_square(&fen_val, &side);
                    let check = in_check.get();
                    let white_bottom = orient.white_bottom();

                    let mut cells = Vec::new();
                    for visual_row in 0..8u8 {
                        for visual_col in 0..8u8 {
                            let (rank, file) = if white_bottom {
                                (7 - visual_row, visual_col)
                            } else {
                                (visual_row, 7 - visual_col)
                            };
                            let idx = rank as usize * 8 + file as usize;
                            let sq = index_to_algebraic(idx);
                            let is_light = (rank + file) % 2 == 1;
                            let piece = squares[idx];

                            let is_selected = sel.as_deref() == Some(sq.as_str());
                            let is_target = targets.iter().any(|t| t == &sq);
                            let is_last_from = lm.as_ref().map(|(f, _)| f == &sq).unwrap_or(false);
                            let is_last_to = lm.as_ref().map(|(_, t)| t == &sq).unwrap_or(false);
                            let is_king_check =
                                check && king_sq.as_deref() == Some(sq.as_str());

                            let animating_from = anim
                                .as_ref()
                                .and_then(|a| parse_uci(&a.uci))
                                .map(|m| m.from == sq)
                                .unwrap_or(false);
                            let hide_piece = animating_from;

                            let sq_click = sq.clone();
                            cells.push(view! {
                                <div
                                    class="board-square"
                                    class:sq-light=is_light
                                    class:sq-dark=!is_light
                                    class:sq-selected=is_selected
                                    class:sq-target=is_target
                                    class:sq-last-from=is_last_from
                                    class:sq-last-to=is_last_to
                                    class:sq-check=is_king_check
                                    data-square=sq.clone()
                                    on:click=move |_| on_square_click(sq_click.clone())
                                >
                                    {if !hide_piece {
                                        piece.map(|p| view! { <PieceSvg piece=p/> })
                                    } else {
                                        None
                                    }}
                                    {if is_target {
                                        Some(view! { <span class="target-dot"/> })
                                    } else {
                                        None
                                    }}
                                </div>
                            });
                        }
                    }
                    cells
                }}
                {move || {
                    let anim = animation.get();
                    let orient = orientation.get();
                    let white_bottom = orient.white_bottom();
                    anim.and_then(|a| {
                        let uci = parse_uci(&a.uci)?;
                        let from_idx = algebraic_to_index(&uci.from)?;
                        let to_idx = algebraic_to_index(&uci.to)?;
                        let (from_row, from_col) = idx_to_visual(from_idx, white_bottom);
                        let (to_row, to_col) = idx_to_visual(to_idx, white_bottom);
                        let dx = (to_col as f64 - from_col as f64) * 100.0;
                        let dy = (to_row as f64 - from_row as f64) * 100.0;
                        let piece = a.piece;
                        let style = format!(
                            "--from-row:{from_row};--from-col:{from_col};--dx:{dx}%;--dy:{dy}%"
                        );
                        let rook_overlay = a.rook.and_then(|(rf, rt, rp)| {
                            let rf_idx = algebraic_to_index(&rf)?;
                            let rt_idx = algebraic_to_index(&rt)?;
                            let (rfr, rfc) = idx_to_visual(rf_idx, white_bottom);
                            let (rtr, rtc) = idx_to_visual(rt_idx, white_bottom);
                            let rdx = (rtc as f64 - rfc as f64) * 100.0;
                            let rdy = (rtr as f64 - rfr as f64) * 100.0;
                            let rstyle = format!(
                                "--from-row:{rfr};--from-col:{rfc};--dx:{rdx}%;--dy:{rdy}%"
                            );
                            Some(view! {
                                <div class="anim-piece anim-rook" style=rstyle>
                                    <PieceSvg piece=rp/>
                                </div>
                            })
                        });
                        Some(view! {
                            <>
                                <div class="anim-piece" style=style>
                                    <PieceSvg piece=piece/>
                                </div>
                                {rook_overlay}
                            </>
                        })
                    })
                }}
            </div>
            {move || promotion_pick.get().map(|(_, _, promos)| {
                let fen_val = fen.get();
                let white_promo = WasmPosition::from_fen(&fen_val)
                    .map(|p| p.side_to_move() == "white")
                    .unwrap_or(true);
                view! {
                    <div class="promo-picker">
                        <span class="promo-label">"Promote to"</span>
                        {promos.into_iter().map(|p| {
                            let display = if white_promo {
                                p.to_ascii_uppercase()
                            } else {
                                p.to_ascii_lowercase()
                            };
                            view! {
                                <button class="promo-btn" on:click=move |_| on_promo_click(p)>
                                    <PieceSvg piece=display/>
                                </button>
                            }
                        }).collect_view()}
                    </div>
                }
            })}
        </div>
    }
}

fn idx_to_visual(idx: usize, white_bottom: bool) -> (u8, u8) {
    let rank = (idx / 8) as u8;
    let file = (idx % 8) as u8;
    if white_bottom {
        (7 - rank, file)
    } else {
        (rank, 7 - file)
    }
}
