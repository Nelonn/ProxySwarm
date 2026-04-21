use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct SwitchProps {
    pub checked: bool,
    pub onchange: Callback<Event>,
    #[prop_or(false)]
    pub disabled: bool,
}

#[derive(Properties, PartialEq)]
pub struct SwitchFieldProps {
    pub label: AttrValue,
    pub checked: bool,
    pub onchange: Callback<Event>,
    #[prop_or(false)]
    pub disabled: bool,
}

#[function_component(Switch)]
pub fn switch(props: &SwitchProps) -> Html {
    let checked = props.checked;

    html! {
        <>
            <style>
                {"
                .md3-switch-control {
                    position: relative;
                    display: inline-flex;
                    flex: 0 0 auto;
                    cursor: pointer;
                }

                .md3-switch-control-disabled {
                    opacity: 0.38;
                    cursor: not-allowed;
                    pointer-events: none;
                }

                .md3-switch-input {
                    position: absolute;
                    inset: 0;
                    width: 100%;
                    height: 100%;
                    margin: 0;
                    opacity: 0;
                    cursor: inherit;
                    z-index: 2;
                }

                .md3-switch-track {
                    position: relative;
                    width: 52px;
                    height: 32px;
                    padding: 2px;
                    box-sizing: border-box;
                    border-radius: 9999px;
                    border: none;
                    background-color: var(--md-sys-color-surface-container-highest);
                    transition: background-color 0.2s ease, box-shadow 0.2s ease;
                }

                .md3-switch-track[data-checked='false'] {
                    box-shadow: inset 0 0 0 2px var(--md-sys-color-outline);
                }

                .md3-switch-track[data-checked='true'] {
                    background-color: var(--md-sys-color-primary);
                    box-shadow: inset 0 0 0 2px var(--md-sys-color-primary);
                }

                .md3-switch-thumb-container {
                    position: absolute;
                    top: 8px;
                    left: 8px;
                    width: 16px;
                    height: 16px;
                    border-radius: 50%;
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    transition:
                        left 0.2s ease,
                        width 0.2s ease,
                        height 0.2s ease,
                        top 0.2s ease;
                }

                .md3-switch-track[data-checked='true'] .md3-switch-thumb-container {
                    top: 4px;
                    left: 24px;
                    width: 24px;
                    height: 24px;
                }

                .md3-switch-thumb-container::before {
                    content: '';
                    position: absolute;
                    inset: -12px;
                    border-radius: 50%;
                    background-color: var(--md-sys-color-on-surface);
                    opacity: 0;
                    transition: opacity 0.2s ease, background-color 0.2s ease, inset 0.2s ease;
                    pointer-events: none;
                }

                .md3-switch-track[data-checked='true'] .md3-switch-thumb-container::before {
                    inset: -8px;
                    background-color: var(--md-sys-color-primary);
                }

                .md3-switch-input:hover ~ .md3-switch-track .md3-switch-thumb-container::before,
                .md3-switch-input:focus-visible ~ .md3-switch-track .md3-switch-thumb-container::before {
                    opacity: 0.12;
                }

                .md3-switch-thumb {
                    width: 100%;
                    height: 100%;
                    border-radius: 50%;
                    background-color: var(--md-sys-color-outline);
                    display: flex;
                    align-items: center;
                    justify-content: center;
                    transition: background-color 0.2s ease;
                }

                .md3-switch-track[data-checked='true'] .md3-switch-thumb {
                    background-color: var(--md-sys-color-on-primary);
                }

                .md3-switch-icon {
                    width: 16px;
                    height: 16px;
                    color: var(--md-sys-color-on-primary-container);
                    pointer-events: none;
                    opacity: 0;
                    transition: opacity 0.15s ease;
                }

                .md3-switch-track[data-checked='true'] .md3-switch-icon {
                    opacity: 1;
                }

                .md3-switch-input:focus-visible ~ .md3-switch-track {
                    outline: 3px solid var(--md-sys-color-primary);
                    outline-offset: 2px;
                }
                "}
            </style>

            <label class={classes!(
                "md3-switch-control",
                props.disabled.then_some("md3-switch-control-disabled")
            )}>
                <input
                    type="checkbox"
                    class="md3-switch-input"
                    checked={checked}
                    onchange={props.onchange.clone()}
                    disabled={props.disabled}
                />
                <span
                    class="md3-switch-track"
                    data-checked={if checked { "true" } else { "false" }}
                >
                    <span class="md3-switch-thumb-container">
                        <span class="md3-switch-thumb">
                            // Check icon (MD3 uses a filled checkmark on the thumb when on)
                            <svg
                                class="md3-switch-icon"
                                viewBox="0 0 24 24"
                                fill="none"
                                xmlns="http://www.w3.org/2000/svg"
                                aria-hidden="true"
                            >
                                <path
                                    d="M5 12l4.5 4.5L19 7"
                                    stroke="currentColor"
                                    stroke-width="2.5"
                                    stroke-linecap="round"
                                    stroke-linejoin="round"
                                />
                            </svg>
                        </span>
                    </span>
                </span>
            </label>
        </>
    }
}

#[function_component(SwitchField)]
pub fn switch_field(props: &SwitchFieldProps) -> Html {
    html! {
        <>
            <style>
                {"
                .md3-switch-field {
                    display: flex;
                    align-items: center;
                    justify-content: space-between;
                    gap: 1rem;
                    min-height: 2rem;
                }

                .md3-switch-field-disabled {
                    opacity: 0.38;
                    cursor: not-allowed;
                }

                .md3-switch-label {
                    color: var(--md-sys-color-on-surface);
                    font-size: 1rem;
                    line-height: 1.5rem;
                    font-weight: 500;
                }
                "}
            </style>

            <label class={classes!(
                "md3-switch-field",
                props.disabled.then_some("md3-switch-field-disabled")
            )}>
                <span class="md3-switch-label">{ props.label.clone() }</span>
                <Switch
                    checked={props.checked}
                    onchange={props.onchange.clone()}
                    disabled={props.disabled}
                />
            </label>
        </>
    }
}
