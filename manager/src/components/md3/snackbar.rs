use gloo_timers::callback::Timeout;
use std::rc::Rc;
use yew::prelude::*;

#[derive(Clone)]
pub struct SnackbarBus {
    push_text_fn: Rc<dyn Fn(String) -> u64>,
    push_message_fn: Rc<dyn Fn(SnackbarMessage) -> u64>,
    hide_fn: Rc<dyn Fn(u64)>,
}

impl PartialEq for SnackbarBus {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.push_text_fn, &other.push_text_fn)
            && Rc::ptr_eq(&self.push_message_fn, &other.push_message_fn)
            && Rc::ptr_eq(&self.hide_fn, &other.hide_fn)
    }
}

impl SnackbarBus {
    pub fn push(&self, text: impl Into<String>) -> u64 {
        (self.push_text_fn)(text.into())
    }

    pub fn push_message(&self, message: SnackbarMessage) -> u64 {
        (self.push_message_fn)(message)
    }

    pub fn hide(&self, id: u64) {
        (self.hide_fn)(id)
    }
}

#[derive(Clone, PartialEq)]
struct SnackbarItem {
    id: u64,
    payload: SnackbarMessage,
    closing: bool,
}

#[derive(Clone, PartialEq)]
pub struct SnackbarMessage {
    pub text: String,
    pub action_label: Option<String>,
    pub on_action: Option<Callback<()>>,
    pub show_close: bool,
}

impl SnackbarMessage {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            action_label: None,
            on_action: None,
            show_close: true,
        }
    }
}

#[derive(Properties, PartialEq)]
pub struct SnackbarProviderProps {
    pub children: Children,
}

#[function_component(SnackbarProvider)]
pub fn snackbar_provider(props: &SnackbarProviderProps) -> Html {
    let items = use_state(Vec::<SnackbarItem>::new);
    let next_id = use_mut_ref(|| 1u64);

    let hide = {
        let items = items.clone();
        Rc::new(move |id: u64| {
            let mut next = (*items).clone();
            if let Some(item) = next.iter_mut().find(|x| x.id == id) {
                item.closing = true;
            }
            items.set(next);

            let items_for_remove = items.clone();
            Timeout::new(220, move || {
                let mut next = (*items_for_remove).clone();
                next.retain(|x| x.id != id);
                items_for_remove.set(next);
            })
            .forget();
        })
    };

    let push_with_action = {
        let items = items.clone();
        let next_id = next_id.clone();
        let hide = hide.clone();
        Rc::new(move |payload: SnackbarMessage| -> u64 {
            let id = {
                let mut current = next_id.borrow_mut();
                let id = *current;
                *current += 1;
                id
            };
            let mut next = (*items).clone();
            next.push(SnackbarItem {
                id,
                payload,
                closing: false,
            });
            items.set(next);

            let hide_for_timeout = hide.clone();
            Timeout::new(5000, move || {
                hide_for_timeout(id);
            })
            .forget();
            id
        })
    };

    let push_text = {
        let push_with_action = push_with_action.clone();
        Rc::new(move |message: String| -> u64 { push_with_action(SnackbarMessage::plain(message)) })
    };

    let bus = SnackbarBus {
        push_text_fn: push_text,
        push_message_fn: push_with_action,
        hide_fn: hide,
    };

    html! {
        <ContextProvider<SnackbarBus> context={bus.clone()}>
            { for props.children.iter() }
            <style>
                {"
                .md3-snackbar-stack {
                    position: fixed;
                    right: 1rem;
                    bottom: 1rem;
                    z-index: 2000;
                    display: flex;
                    flex-direction: column;
                    gap: 0.5rem;
                    max-width: min(26rem, calc(100vw - 2rem));
                    pointer-events: none;
                }
                .md3-snackbar {
                    pointer-events: auto;
                    display: flex;
                    align-items: center;
                    justify-content: space-between;
                    gap: 0.75rem;
                    padding: 0.75rem 0.875rem 0.75rem 1rem;
                    border-radius: 0.75rem;
                    background: color-mix(in srgb, var(--md-sys-color-surface-container) 86%, black 14%);
                    color: var(--md-sys-color-on-surface);
                    box-shadow: 0 10px 28px rgba(0, 0, 0, 0.42);
                    border: 1px solid color-mix(in srgb, var(--md-sys-color-primary) 20%, transparent);
                    animation: md3-snackbar-enter 220ms cubic-bezier(0.2, 0.0, 0.2, 1);
                    transform-origin: bottom right;
                }
                .md3-snackbar-exit {
                    animation: md3-snackbar-exit 220ms cubic-bezier(0.4, 0.0, 1, 1) forwards;
                }
                @keyframes md3-snackbar-enter {
                    from {
                        opacity: 0;
                        transform: translateY(8px) scale(0.98);
                    }
                    to {
                        opacity: 1;
                        transform: translateY(0) scale(1);
                    }
                }
                @keyframes md3-snackbar-exit {
                    from {
                        opacity: 1;
                        transform: translateY(0) scale(1);
                    }
                    to {
                        opacity: 0;
                        transform: translateY(10px) scale(0.98);
                    }
                }
                .md3-snackbar-message {
                    font-size: 0.875rem;
                    line-height: 1.35;
                    word-break: break-word;
                }
                .md3-snackbar-actions {
                    display: inline-flex;
                    align-items: center;
                    gap: 0.25rem;
                    flex: 0 0 auto;
                }
                .md3-snackbar-action {
                    border: none;
                    background: transparent;
                    color: var(--md-sys-color-primary);
                    font-size: 0.8125rem;
                    font-weight: 600;
                    padding: 0.3rem 0.65rem;
                    border-radius: 9999px;
                    cursor: pointer;
                }
                .md3-snackbar-action:hover {
                    background: color-mix(in srgb, var(--md-sys-color-primary) 18%, transparent);
                }
                .md3-snackbar-close-icon {
                    border: none;
                    background: transparent;
                    color: var(--md-sys-color-on-surface);
                    width: 1.75rem;
                    height: 1.75rem;
                    border-radius: 9999px;
                    display: inline-flex;
                    align-items: center;
                    justify-content: center;
                    cursor: pointer;
                    padding: 0;
                }
                .md3-snackbar-close-icon svg {
                    width: 1rem;
                    height: 1rem;
                    display: block;
                }
                .md3-snackbar-close-icon:hover {
                    background: color-mix(in srgb, var(--md-sys-color-on-surface) 14%, transparent);
                }
                @media (max-width: 780px) {
                    .md3-snackbar-stack {
                        left: 0.75rem;
                        right: 0.75rem;
                        bottom: 0.75rem;
                        max-width: none;
                    }
                }
                "}
            </style>
            <div class="md3-snackbar-stack">
                {
                    for items.iter().map(|item| {
                        let id = item.id;
                        let hide = bus.hide_fn.clone();
                        let action_label = item.payload.action_label.clone();
                        let action_cb = item.payload.on_action.clone();
                        let show_close = item.payload.show_close;
                        let on_close = Callback::from(move |_| {
                            hide(id);
                        });
                        html! {
                            <div class={classes!("md3-snackbar", item.closing.then_some("md3-snackbar-exit"))} key={id.to_string()}>
                                <div class="md3-snackbar-message">{ item.payload.text.clone() }</div>
                                <div class="md3-snackbar-actions">
                                    {
                                        if let (Some(label), Some(cb)) = (action_label, action_cb) {
                                            html! {
                                                <button type="button" class="md3-snackbar-action" onclick={Callback::from(move |_| cb.emit(()))}>{ label }</button>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                    {
                                        if show_close {
                                            html! {
                                                <button type="button" class="md3-snackbar-close-icon" aria-label="Close" onclick={on_close}>
                                                    <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
                                                        <path d="M18.3 5.71a1 1 0 0 0-1.41 0L12 10.59 7.11 5.7A1 1 0 0 0 5.7 7.12L10.58 12l-4.9 4.89a1 1 0 0 0 1.42 1.41L12 13.41l4.89 4.9a1 1 0 0 0 1.41-1.42L13.41 12l4.9-4.89a1 1 0 0 0 0-1.4Z" fill="currentColor"/>
                                                    </svg>
                                                </button>
                                            }
                                        } else {
                                            html! {}
                                        }
                                    }
                                </div>
                            </div>
                        }
                    })
                }
            </div>
        </ContextProvider<SnackbarBus>>
    }
}
