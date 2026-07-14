//! gambit brand components.

use leptos::prelude::*;

/// Lowercase wordmark with diagonal slash accent.
#[component]
pub fn Logo() -> impl IntoView {
    view! {
        <div class="logo">
            <div class="logo-mark" aria-hidden="true">
                <svg viewBox="0 0 32 32" fill="none" xmlns="http://www.w3.org/2000/svg">
                    <rect width="32" height="32" rx="8" fill="url(#logo-grad)"/>
                    <path d="M8 24L16 8L24 24H19L16 17L13 24H8Z" fill="white" fill-opacity="0.95"/>
                    <defs>
                        <linearGradient id="logo-grad" x1="4" y1="4" x2="28" y2="28" gradientUnits="userSpaceOnUse">
                            <stop stop-color="#7c5cff"/>
                            <stop offset="1" stop-color="#22d3ee"/>
                        </linearGradient>
                    </defs>
                </svg>
            </div>
            <div class="logo-text">
                <h1 class="logo-wordmark">"gambit"</h1>
                <p class="logo-tagline">"chess data terminal"</p>
            </div>
        </div>
    }
}

/// Live connection indicator for the API + database.
#[component]
pub fn HealthBadge(healthy: ReadSignal<bool>) -> impl IntoView {
    view! {
        <div class=move || if healthy.get() { "health-badge ok" } else { "health-badge err" }>
            <span class="health-dot"/>
            <span>{move || if healthy.get() { "Connected" } else { "Offline" }}</span>
        </div>
    }
}
