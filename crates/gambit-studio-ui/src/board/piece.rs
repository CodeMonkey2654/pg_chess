//! Inline SVG chess pieces — geometric blade style.

use leptos::prelude::*;

/// Render a chess piece as inline SVG from its FEN character.
#[component]
pub fn PieceSvg(piece: char) -> impl IntoView {
    let is_white = piece.is_ascii_uppercase();
    let kind = piece.to_ascii_lowercase();
    let grad_id = format!("{}-{}", if is_white { "w" } else { "b" }, kind);
    let stroke = if is_white { "#8899aa" } else { "#0a0c10" };
    let fill = format!("url(#{grad_id})");

    view! {
        <svg class="piece-svg" viewBox="0 0 45 45" xmlns="http://www.w3.org/2000/svg">
            <defs>
                <linearGradient id=grad_id.clone() x1="0%" y1="0%" x2="100%" y2="100%">
                    {if is_white {
                        view! {
                            <stop offset="0%" stop-color="#e8ecf4"/>
                            <stop offset="100%" stop-color="#b8c4d8"/>
                        }.into_any()
                    } else {
                        view! {
                            <stop offset="0%" stop-color="#4a5568"/>
                            <stop offset="100%" stop-color="#1a1f2e"/>
                        }.into_any()
                    }}
                </linearGradient>
            </defs>
            <g fill=fill stroke=stroke stroke-width="1.2">
                {match kind {
                    'k' => view! {
                        <path d="M22.5 4 L24 10 L22.5 12 L21 10 Z"/>
                        <path d="M22.5 12 L26 16 L26 20 L22.5 22 L19 20 L19 16 Z"/>
                        <rect x="14" y="22" width="17" height="6" rx="1"/>
                        <rect x="12" y="28" width="21" height="5" rx="1.5"/>
                        <rect x="10" y="33" width="25" height="6" rx="2"/>
                    }.into_any(),
                    'q' => view! {
                        <circle cx="22.5" cy="8" r="3"/>
                        <circle cx="15" cy="12" r="2.5"/>
                        <circle cx="30" cy="12" r="2.5"/>
                        <circle cx="22.5" cy="14" r="2.5"/>
                        <path d="M22.5 16 L28 22 L28 26 L22.5 28 L17 26 L17 22 Z"/>
                        <rect x="14" y="28" width="17" height="5" rx="1"/>
                        <rect x="10" y="33" width="25" height="6" rx="2"/>
                    }.into_any(),
                    'r' => view! {
                        <rect x="14" y="6" width="5" height="4" rx="0.5"/>
                        <rect x="20" y="6" width="5" height="4" rx="0.5"/>
                        <rect x="26" y="6" width="5" height="4" rx="0.5"/>
                        <rect x="13" y="10" width="19" height="18" rx="1.5"/>
                        <rect x="10" y="33" width="25" height="6" rx="2"/>
                    }.into_any(),
                    'b' => view! {
                        <ellipse cx="22.5" cy="10" rx="4" ry="5"/>
                        <path d="M22.5 15 L27 22 L27 27 L22.5 29 L18 27 L18 22 Z"/>
                        <rect x="14" y="29" width="17" height="4" rx="1"/>
                        <rect x="10" y="33" width="25" height="6" rx="2"/>
                        <circle cx="22.5" cy="20" r="1.5" fill=if is_white { "#7c5cff" } else { "#22d3ee" }/>
                    }.into_any(),
                    'n' => view! {
                        <path d="M30 8 C28 6 22 6 18 10 C14 14 14 20 16 24 L14 28 L18 28 L20 24 C18 20 18 16 20 13 C22 10 26 9 28 11 Z"/>
                        <rect x="12" y="28" width="18" height="5" rx="1"/>
                        <rect x="10" y="33" width="25" height="6" rx="2"/>
                    }.into_any(),
                    _ => view! {
                        <circle cx="22.5" cy="12" r="5"/>
                        <path d="M22.5 17 L26 24 L26 28 L22.5 30 L19 28 L19 24 Z"/>
                        <rect x="10" y="33" width="25" height="6" rx="2"/>
                    }.into_any(),
                }}
            </g>
        </svg>
    }
}
