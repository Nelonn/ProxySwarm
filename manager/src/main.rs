use yew::prelude::*;
use yew_router::prelude::*;

mod components;
mod pages;
pub mod pb;
mod services;
mod state;
mod storage;

#[derive(Clone, Routable, PartialEq)]
enum Route {
    #[at("/")]
    Dashboard,
    #[at("/nodes")]
    Nodes,
    #[at("/nodes/:id/config")]
    NodeConfig { id: String },
    #[at("/accounts")]
    Accounts,
    #[at("/save-data")]
    SaveData,
    #[not_found]
    #[at("/404")]
    NotFound,
}

use components::{SnackbarProvider, SvgIcon, SvgSpriteDefs};
use pages::accounts::Accounts;
use pages::dashboard::Dashboard;
use pages::node_config::NodeConfigPage;
use pages::nodes::Nodes;
use pages::save_data::SaveData;
use state::State;

fn switch(routes: Route) -> Html {
    match routes {
        Route::Dashboard => html! { <Dashboard /> },
        Route::Nodes => html! { <Nodes /> },
        Route::NodeConfig { id } => html! { <NodeConfigPage id={id} /> },
        Route::Accounts => html! { <Accounts /> },
        Route::SaveData => html! { <SaveData /> },
        Route::NotFound => html! { <div class="p-6"><h1>{ "404 Not Found" }</h1></div> },
    }
}

#[function_component(App)]
fn app() -> Html {
    let state = use_state(storage::load_state);

    {
        let state = state.clone();
        use_effect_with((), move |_| {
            storage::hydrate_desktop_state(state);
            || ()
        });
    }

    html! {
        <BrowserRouter>
            <ContextProvider<UseStateHandle<State>> context={state}>
                <SnackbarProvider>
                    <SvgSpriteDefs />
                    <div class="flex h-screen bg-surface">
                        // Sidebar (MD3 Navigation Rail/Drawer)
                        <nav class="w-64 bg-surface-container p-4">
                            <div class="text-2xl font-bold p-4">{ "ProxySwarm" }</div>
                            <NavItem
                                to={Route::Dashboard}
                                icon_name={"icon-dashboard"}
                                label={"Dashboard"}
                            />
                            <NavItem
                                to={Route::Nodes}
                                icon_name={"icon-network-node"}
                                label={"Nodes"}
                            />
                            <NavItem
                                to={Route::Accounts}
                                icon_name={"icon-groups"}
                                label={"Accounts"}
                            />
                            <NavItem
                                to={Route::SaveData}
                                icon_name={"icon-exit-to-app"}
                                label={"Save Data"}
                            />
                        </nav>

                        // Main Content
                        <main class="flex-1 overflow-auto bg-surface text-on-surface">
                            <Switch<Route> render={switch} />
                        </main>
                    </div>
                </SnackbarProvider>
            </ContextProvider<UseStateHandle<State>>>

            <style>
                { "
                :root {
                    --md-sys-color-primary: #D0BCFF;
                    --md-sys-color-primary-hover: #5B419C;
                    --md-sys-color-on-primary: #381E72;
                    --md-sys-color-surface: #141218;
                    --md-sys-color-on-surface: #E6E0E9;
                    --md-sys-color-outline: #938F99;
                    --md-sys-color-surface-container: #211F26;
                    --md-sys-color-primary-container: #4F378B;
                    --md-sys-color-on-primary-container: #EADDFF;
                    --md-sys-color-primary-focus-surface: rgba(208, 188, 255, 0.14);
                    --md-sys-color-primary-outline: color-mix(in srgb, var(--md-sys-color-primary) 28%, var(--md-sys-color-surface) 72%);
                    --md-sys-color-input-idle-surface: rgba(255, 255, 255, 0.04);
                    --md-sys-color-surface-variant: #49454F;
                    --md-sys-color-on-surface-variant: #CAC4D0;
                    --md-sys-color-on-surface-muted: rgba(202, 196, 208, 0.58);
                    --md-sys-color-inverse-surface: #E6E0E9;
                    --md-sys-color-inverse-on-surface: #322F35;
                    --md-sys-color-inverse-primary: #4F378B;
                    --md-sys-color-shadow: #000000;
                    --md-sys-color-surface-tint: #D0BCFF;
                    --md-sys-color-outline-variant: #49454F;
                    --md-sys-color-error-soft: #F1A7B3;
                    --md-sys-color-error-soft-surface: rgba(241, 167, 179, 0.10);
                }
                body { margin: 0; font-family: AppFont, system-ui, -apple-system, 'Segoe UI', Arial, sans-serif; }
                button, input, textarea, select { font-family: inherit; }
                * {
                    scrollbar-width: thin;
                    scrollbar-color: color-mix(in srgb, var(--md-sys-color-primary) 42%, var(--md-sys-color-surface) 58%) transparent;
                }
                *::-webkit-scrollbar {
                    width: 12px;
                    height: 12px;
                }
                *::-webkit-scrollbar-track {
                    background: transparent;
                }
                *::-webkit-scrollbar-thumb {
                    background-color: color-mix(in srgb, var(--md-sys-color-primary) 42%, var(--md-sys-color-surface) 58%);
                    border-radius: 9999px;
                    border: 3px solid transparent;
                    background-clip: padding-box;
                }
                *::-webkit-scrollbar-thumb:hover {
                    background-color: color-mix(in srgb, var(--md-sys-color-primary) 56%, var(--md-sys-color-surface) 44%);
                }
                *::-webkit-scrollbar-corner {
                    background: transparent;
                }
                
                /* Layout utilities */
                .flex { display: flex; }
                .h-screen { height: 100vh; }
                .w-64 { width: 16rem; }
                .flex-1 { flex: 1; }
                .flex-col { flex-direction: column; }
                .justify-between { justify-content: space-between; }
                
                /* Spacing utilities */
                .p-4 { padding: 1rem; }
                .p-6 { padding: 1.5rem; }
                .p-12 { padding: 3rem; }
                .space-y-2 > * + * { margin-top: 0.5rem; }
                .space-y-1 > * + * { margin-top: 0.25rem; }
                .space-y-4 > * + * { margin-top: 1rem; }
                .space-y-6 > * + * { margin-top: 1.5rem; }
                .space-x-2 > * + * { margin-left: 0.5rem; }
                .mb-1 { margin-bottom: 0.25rem; }
                .mb-4 { margin-bottom: 1rem; }
                .mt-4 { margin-top: 1rem; }
                .mt-2 { margin-top: 0.5rem; }
                .mr-2 { margin-right: 0.5rem; }
                .block { display: block; }
                
                /* MD3 Borders - Cards use outline-variant, not 1px borders */
                .border-r { border-right-width: 1px; border-right-style: solid; border-color: var(--md-sys-color-outline-variant); }
                .border-outline { border-color: var(--md-sys-color-outline-variant); }
                .border-primary { border-color: var(--md-sys-color-primary); }
                .border { border-width: 1px; border-style: solid; border-color: var(--md-sys-color-outline-variant); }
                
                /* MD3 Cards should use elevation instead of borders */
                .md3-card {
                    background-color: var(--md-sys-color-surface);
                    border-radius: 1rem;
                    padding: 1.5rem;
                }
                .md3-card:hover {
                }
                
                /* MD3 Input Fields */
                .md3-input {
                    width: 100%;
                    background-color: var(--md-sys-color-surface-variant);
                    border: 2px solid var(--md-sys-color-primary-outline);
                    border-radius: 0.5rem;
                    padding: 0.75rem 1rem;
                    color: var(--md-sys-color-on-surface);
                    font-size: 1rem;
                    transition: border-color 0.2s, box-shadow 0.2s;
                }
                .md3-input:focus {
                    outline: none;
                    border-color: var(--md-sys-color-primary);
                }
                .md3-input-shell {
                    position: relative;
                }
                .md3-input-with-action {
                    padding-right: 3.5rem;
                }
                .md3-input-action {
                    position: absolute;
                    top: 50%;
                    right: 0.5rem;
                    transform: translateY(-50%);
                    width: 2.25rem;
                    height: 2.25rem;
                    border: none;
                    border-radius: 9999px;
                    padding: 0;
                    display: inline-flex;
                    align-items: center;
                    justify-content: center;
                    background: transparent;
                    color: var(--md-sys-color-on-surface);
                    cursor: pointer;
                    transition: background-color 0.2s ease;
                }
                .md3-input-action:hover {
                    background: color-mix(in srgb, var(--md-sys-color-primary) 12%, transparent);
                }
                .md3-input-action:active {
                    background: color-mix(in srgb, var(--md-sys-color-primary) 18%, transparent);
                }
                .md3-input-action:focus-visible {
                    outline: none;
                    box-shadow: 0 0 0 2px color-mix(in srgb, var(--md-sys-color-primary) 24%, transparent);
                }
                .md3-input-action span,
                .md3-input-action svg {
                    width: 1.25rem;
                    height: 1.25rem;
                    display: block;
                }
                .md3-input-action svg {
                    fill: currentColor;
                    stroke: currentColor;
                }
                .md3-input-error {
                    border-color: var(--md-sys-color-error-soft);
                    background-color: var(--md-sys-color-error-soft-surface);
                }
                .md3-input-error:focus {
                    border-color: var(--md-sys-color-error-soft);
                    box-shadow: 0 0 0 1px color-mix(in srgb, var(--md-sys-color-error-soft) 36%, transparent 64%);
                }
                .md3-input::placeholder {
                    color: var(--md-sys-color-on-surface-variant);
                    opacity: 0.7;
                }
                .md3-input[type=number] {
                    appearance: textfield;
                    -moz-appearance: textfield;
                }
                .md3-input[type=number]::-webkit-outer-spin-button,
                .md3-input[type=number]::-webkit-inner-spin-button {
                    -webkit-appearance: none;
                    margin: 0;
                }
                
                .md3-country-picker-menu {
                    position: absolute;
                    left: 0;
                    right: 0;
                    top: calc(100% + 0.5rem);
                    z-index: 40;
                    background-color: var(--md-sys-color-surface);
                    border: 2px solid var(--md-sys-color-primary-outline);
                    border-radius: 1rem;
                    box-shadow: 0 18px 40px rgba(0, 0, 0, 0.35);
                    padding: 0.4rem;
                    max-height: 16rem;
                    overflow-y: auto;
                }
                .md3-country-picker-option {
                    width: 100%;
                    display: flex;
                    align-items: center;
                    gap: 0.75rem;
                    border: none;
                    background: transparent;
                    color: var(--md-sys-color-on-surface);
                    border-radius: 0.875rem;
                    padding: 0.7rem 0.8rem;
                    text-align: left;
                    cursor: pointer;
                }
                .md3-country-picker-option:hover {
                    background-color: rgba(208, 188, 255, 0.08);
                }

                /* Border radius utilities */
                .rounded-2xl { border-radius: 1rem; }
                .rounded-xl { border-radius: 0.75rem; }
                .rounded-lg { border-radius: 0.5rem; }
                .rounded-3xl { border-radius: 1.5rem; }
                .rounded-full { border-radius: 9999px; }
                
                /* Size utilities */
                .max-w-md { max-width: 28rem; }
                .max-w-2xl { max-width: 42rem; }
                .w-full { width: 100%; }
                .h-full { height: 100%; }
                .text-center { text-align: center; }
                .overflow-auto { overflow: auto; }
                
                /* Color utilities */
                .bg-surface { background-color: var(--md-sys-color-surface); }
                .bg-surface-container { background-color: var(--md-sys-color-surface-container); }
                .bg-primary { background-color: var(--md-sys-color-primary); }
                .text-primary { color: var(--md-sys-color-primary); }
                .text-on-surface { color: var(--md-sys-color-on-surface); }
                .text-on-primary { color: var(--md-sys-color-on-primary); }
                .text-on-primary-container { color: var(--md-sys-color-on-primary-container); }
                .bg-primary-container { background-color: var(--md-sys-color-primary-container); }
                .opacity-70 { opacity: 0.7; }
                
                /* Typography */
                .text-2xl { font-size: 1.5rem; line-height: 2rem; }
                .text-3xl { font-size: 1.875rem; line-height: 2.25rem; }
                .text-xl { font-size: 1.25rem; line-height: 1.75rem; }
                .text-lg { font-size: 1.125rem; line-height: 1.75rem; }
                .text-sm { font-size: 0.875rem; line-height: 1.25rem; }
                .font-bold { font-weight: 700; }
                .font-semibold { font-weight: 600; }
                .font-medium { font-weight: 500; }
                
                /* Grid */
                .grid { display: grid; }
                .gap-4 { gap: 1rem; }
                .gap-6 { gap: 1.5rem; }
                .grid-cols-1 { grid-template-columns: repeat(1, minmax(0, 1fr)); }
                @media (min-width: 768px) { .md-grid-cols-2 { grid-template-columns: repeat(2, minmax(0, 1fr)); } }
                @media (min-width: 768px) { .md-grid-cols-3 { grid-template-columns: repeat(3, minmax(0, 1fr)); } }
                @media (min-width: 1024px) { .lg-grid-cols-3 { grid-template-columns: repeat(3, minmax(0, 1fr)); } }
                
                /* Navigation */
                .nav-item {
                    display: flex;
                    align-items: center;
                    gap: 12px;
                    min-height: 56px;
                    padding: 16px;
                    text-decoration: none;
                    color: var(--md-sys-color-on-surface-variant);
                    border-radius: 9999px;
                    font-weight: 600;
                    transition: background-color 0.2s, color 0.2s;
                }
                .nav-item:hover { background-color: rgba(208, 188, 255, 0.08); color: var(--md-sys-color-on-surface); }
                .nav-item.active { background-color: var(--md-sys-color-primary-focus-surface); color: var(--md-sys-color-primary); }
                .nav-item-icon {
                    width: 24px;
                    height: 24px;
                    display: inline-flex;
                    align-items: center;
                    justify-content: center;
                    flex: 0 0 auto;
                }
                .nav-item-icon svg {
                    width: 100%;
                    height: 100%;
                    display: block;
                }
                .nav-item-label {
                    display: block;
                    line-height: 24px;
                }
                
                /* Effects */
                .hover-opacity-90:hover { opacity: 0.9; }
                .transition-opacity { transition: opacity 0.2s; }
                .transition-colors { transition: background-color 0.2s, color 0.2s; }
                .outline-none { outline: none; }

                /* Lists */
                .md3-divider {
                    height: 1px;
                    background-color: var(--md-sys-color-outline-variant);
                    opacity: 0.7;
                }
                .md3-list { display: block; }
                .md3-list-header {
                    display: grid;
                    grid-template-columns: var(--rich-table-columns, repeat(1, minmax(0, 1fr)));
                    align-items: center;
                    gap: 1rem;
                    padding: 0.75rem 0.5rem;
                    color: var(--md-sys-color-on-surface-variant);
                    font-size: 0.875rem;
                    text-transform: uppercase;
                    letter-spacing: 0.06em;
                }
                .md3-list-row {
                    display: grid;
                    grid-template-columns: var(--rich-table-columns, repeat(1, minmax(0, 1fr)));
                    align-items: center;
                    gap: 1rem;
                    padding: 1rem 0.5rem;
                }
                .md3-list-col { min-width: 0; }
                .md3-list-col-main,
                .md3-list-col-access-id,
                .md3-list-col-actions { min-width: 0; }
                .md3-list-actions { display: flex; gap: 0.5rem; justify-content: flex-end; align-items: center; }
                @media (max-width: 900px) {
                    .md3-list-header { display: none; }
                    .md3-list-row { display: flex; flex-direction: column; gap: 0.75rem; }
                    .md3-list-col, .md3-list-col-actions { min-width: 0; }
                    .md3-list-actions { justify-content: flex-start; }
                }

                .md3-secret-btn {
                    display: inline-flex;
                    align-items: center;
                    border: none;
                    background: transparent;
                    padding: 0;
                    cursor: pointer;
                    text-align: left;
                }
                .md3-secret {
                    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', 'Courier New', monospace;
                    font-size: 0.875rem;
                    line-height: 1.25rem;
                    padding: 0.25rem 0.5rem;
                    border-radius: 0.5rem;
                    display: inline-block;
                }
                .md3-secret-hidden {
                    background-color: rgba(147, 143, 153, 0.18);
                    color: var(--md-sys-color-on-surface-variant);
                    letter-spacing: 0.12em;
                }
                .md3-secret-revealed {
                    background-color: var(--md-sys-color-surface-container);
                    color: var(--md-sys-color-on-surface);
                }
                .md3-code-block {
                    margin: 0;
                    max-height: 24rem;
                    overflow: auto;
                    padding: 1rem;
                    border-radius: 1rem;
                    background-color: rgba(255, 255, 255, 0.04);
                    border: 1px solid var(--md-sys-color-outline-variant);
                    color: var(--md-sys-color-on-surface);
                    font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, 'Liberation Mono', 'Courier New', monospace;
                    font-size: 0.875rem;
                    line-height: 1.5;
                    white-space: pre-wrap;
                    word-break: break-word;
                }

                .md3-config-nav {
                    position: fixed;
                    left: 50%;
                    bottom: 1.5rem;
                    transform: translateX(-50%);
                    display: flex;
                    align-items: center;
                    gap: 0.5rem;
                    padding: 0.5rem;
                    background-color: rgba(33, 31, 38, 0.92);
                    border-radius: 9999px;
                    box-shadow: 0 20px 48px rgba(0, 0, 0, 0.36);
                    backdrop-filter: blur(12px);
                    z-index: 30;
                }
                .md3-config-nav-item {
                    display: inline-flex;
                    position: relative;
                    align-items: center;
                    height: 40px;
                    gap: 4px;
                    border: none;
                    background: transparent;
                    color: var(--md-sys-color-on-surface-variant);
                    border-radius: 9999px;
                    padding: 10px 16px;
                    cursor: pointer;
                    font-weight: 600;
                    transition: color 0.18s ease;
                    overflow: hidden;
                }
                .md3-config-nav-item::before {
                    content: '';
                    position: absolute;
                    inset: 0;
                    border-radius: inherit;
                    background-color: rgba(208, 188, 255, 0);
                    transform: scale(0.86);
                    opacity: 0;
                    transition: transform 0.2s ease, opacity 0.2s ease, background-color 0.2s ease;
                    z-index: 0;
                }
                .md3-config-nav-item > * {
                    position: relative;
                    z-index: 1;
                }
                .md3-config-nav-item:hover {
                    color: var(--md-sys-color-on-surface);
                }
                .md3-config-nav-item:hover::before {
                    background-color: rgba(208, 188, 255, 0.08);
                    transform: scale(1);
                    opacity: 1;
                }
                .md3-config-nav-item-active {
                    color: var(--md-sys-color-primary);
                }
                .md3-config-nav-item-active::before {
                    background-color: rgba(208, 188, 255, 0.14);
                    transform: scale(1);
                    opacity: 1;
                }
                .md3-config-nav-item-active:hover {
                    color: var(--md-sys-color-on-surface);
                }
                .md3-config-nav-item-active:hover::before {
                    background-color: rgba(208, 188, 255, 0.18);
                    transform: scale(1);
                    opacity: 1;
                }
                .md3-config-nav-item-active:hover {
                    color: var(--md-sys-color-primary);
                }
                .md3-config-nav-icon {
                    width: 20px;
                    height: 20px;
                    display: inline-flex;
                    align-items: center;
                    justify-content: center;
                    flex: 0 0 auto;
                }
                .md3-config-nav-icon svg {
                    width: 100%;
                    height: 100%;
                    display: block;
                }
                .md3-config-nav-label {
                    display: block;
                    line-height: 20px;
                    text-box-trim: trim-both;
                    text-box-edge: cap alphabetic;
                }
                .md3-qr-card {
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    padding: 1rem;
                    border-radius: 1rem;
                    background-color: rgba(255, 255, 255, 0.04);
                }
                .md3-qr-card svg {
                    width: 16rem;
                    max-width: 100%;
                    height: auto;
                    border-radius: 0.75rem;
                    background: white;
                }
                .md3-access-link-row {
                    display: flex;
                    align-items: flex-start;
                    gap: 0.75rem;
                }
                .md3-access-link {
                    flex: 1 1 auto;
                    min-width: 0;
                    word-break: break-all;
                    padding: 0.875rem 1rem;
                    border-radius: 0.875rem;
                    background-color: var(--md-sys-color-surface-container);
                    border: 2px solid var(--md-sys-color-primary-outline);
                    font-size: 0.875rem;
                    line-height: 1.4;
                }
                .md3-routing-rule-card {
                    transition: transform 180ms ease, box-shadow 180ms ease;
                }
                .md3-routing-rule-card-move-up {
                    animation: md3-routing-rule-move-up 280ms cubic-bezier(0.2, 0.0, 0.2, 1);
                }
                .md3-routing-rule-card-move-down {
                    animation: md3-routing-rule-move-down 280ms cubic-bezier(0.2, 0.0, 0.2, 1);
                }
                @keyframes md3-routing-rule-move-up {
                    0% { transform: translateY(10px); box-shadow: 0 0 0 rgba(0, 0, 0, 0); }
                    55% { transform: translateY(-2px); box-shadow: 0 16px 30px rgba(0, 0, 0, 0.24); }
                    100% { transform: translateY(0); box-shadow: 0 0 0 rgba(0, 0, 0, 0); }
                }
                @keyframes md3-routing-rule-move-down {
                    0% { transform: translateY(-10px); box-shadow: 0 0 0 rgba(0, 0, 0, 0); }
                    55% { transform: translateY(2px); box-shadow: 0 16px 30px rgba(0, 0, 0, 0.24); }
                    100% { transform: translateY(0); box-shadow: 0 0 0 rgba(0, 0, 0, 0); }
                }
                @media (max-width: 780px) {
                    .md3-config-nav {
                        width: calc(100vw - 1.5rem);
                        justify-content: space-between;
                        gap: 0.25rem;
                    }
                    .md3-config-nav-item {
                        flex: 1 1 0;
                        justify-content: center;
                        padding: 10px 16px;
                    }
                    .md3-config-nav-item span:last-child {
                        display: none;
                    }
                    .md3-access-link-row {
                        flex-direction: column;
                    }
                }
                " }
            </style>
        </BrowserRouter>
    }
}

#[derive(Properties, PartialEq)]
struct NavItemProps {
    to: Route,
    icon_name: String,
    label: String,
}

#[function_component(NavItem)]
fn nav_item(props: &NavItemProps) -> Html {
    let navigator = use_navigator().unwrap();
    let current_route = use_route::<Route>().unwrap_or(Route::Dashboard);
    let active = current_route == props.to;

    let onclick = {
        let to = props.to.clone();
        Callback::from(move |_| navigator.push(&to))
    };

    html! {
        <a class={classes!("nav-item", if active { "active" } else { "" })}
           href="javascript:void(0)"
           {onclick}>
            <span class="nav-item-icon">
                <SvgIcon name={AttrValue::from(props.icon_name.clone())} />
            </span>
            <span class="nav-item-label">{ &props.label }</span>
        </a>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}
