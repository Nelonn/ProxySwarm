use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, Timelike};
use js_sys::Date as JsDate;
use yew::prelude::*;

use super::{Button, ButtonType, Popup, PopupSize};

const DATEPICKER_STYLE_ID: &str = "md3-datepicker-component-styles";
const DATEPICKER_CSS: &str = r#"
.md3-datepicker-input {
    display: flex;
    gap: 0.5rem;
    align-items: center;
}
.md3-datepicker-open {
    appearance: none;
    border: 1px solid var(--md-sys-color-outline);
    background: transparent;
    color: var(--md-sys-color-on-surface);
    border-radius: 0.75rem;
    height: 2.75rem;
    padding: 0 0.75rem;
    cursor: pointer;
    transition: background-color 0.15s, border-color 0.15s;
}
.md3-datepicker-open:hover:not(:disabled) { background-color: rgba(208, 188, 255, 0.08); }
.md3-datepicker-open:disabled { opacity: 0.38; cursor: not-allowed; }
.md3-datepicker {
    display: flex;
    flex-direction: column;
    gap: 0.75rem;
}
.md3-datepicker-nav {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
}
.md3-datepicker-nav-label {
    font-weight: 600;
    letter-spacing: 0.02em;
}
.md3-datepicker-nav-label-btn {
    appearance: none;
    border: none;
    background: transparent;
    color: var(--md-sys-color-on-surface);
    font-weight: 600;
    letter-spacing: 0.02em;
    cursor: pointer;
    padding: 0.25rem 0.5rem;
    border-radius: 9999px;
    transition: background-color 0.15s;
}
.md3-datepicker-nav-label-btn:hover { background-color: rgba(208, 188, 255, 0.08); }
.md3-datepicker-nav-btn {
    width: 2.25rem;
    height: 2.25rem;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: 9999px;
    border: none;
    background: transparent;
    color: var(--md-sys-color-on-surface);
    cursor: pointer;
    transition: background-color 0.15s;
}
.md3-datepicker-nav-icon {
    width: 1.5rem;
    height: 1.5rem;
    display: block;
    flex: 0 0 auto;
}
.md3-datepicker-nav-btn:hover { background-color: rgba(208, 188, 255, 0.08); }
.md3-datepicker-weekdays {
    display: grid;
    grid-template-columns: repeat(7, 2.25rem);
    gap: 0.25rem;
    justify-content: center;
    color: var(--md-sys-color-on-surface-variant);
    font-size: 0.75rem;
    text-transform: uppercase;
    letter-spacing: 0.06em;
}
.md3-datepicker-weekdays > div { text-align: center; padding: 0.25rem 0; }
.md3-datepicker-grid {
    display: grid;
    grid-template-columns: repeat(7, 2.25rem);
    gap: 0.25rem;
    justify-content: center;
}
.md3-datepicker-cell { width: 2.25rem; height: 2.25rem; }
.md3-datepicker-day {
    height: 2.25rem;
    width: 2.25rem;
    border-radius: 9999px;
    border: none;
    background: transparent;
    color: var(--md-sys-color-on-surface);
    cursor: pointer;
    transition: background-color 0.15s;
}
.md3-datepicker-day:hover:not(:disabled):not(.md3-datepicker-day-selected) { background-color: rgba(208, 188, 255, 0.10); }
.md3-datepicker-day-selected {
    background-color: var(--md-sys-color-primary-container);
    color: var(--md-sys-color-on-primary-container);
}
.md3-datepicker-day-selected:hover:not(:disabled) {
    background-color: var(--md-sys-color-primary-hover);
    color: var(--md-sys-color-on-primary-container);
}
.md3-datepicker-day.md3-datepicker-day-outside:not(.md3-datepicker-day-selected) {
    color: var(--md-sys-color-on-surface-muted);
}
.md3-datepicker-day-inrange {
    background-color: rgba(208, 188, 255, 0.14);
}
.md3-datepicker-day-disabled { opacity: 0.38; cursor: not-allowed; }
.md3-datepicker-time {
    display: flex;
    gap: 0.75rem;
}
.md3-datepicker-time-field { flex: 1 1 auto; min-width: 0; }
.md3-datepicker-time-row {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
}
.md3-datepicker-time-input {
    width: 4.5rem;
    background-color: transparent;
    border: 2px solid transparent;
    box-shadow: none;
    padding: 0.625rem 0.75rem;
    text-align: center;
    appearance: textfield;
    -moz-appearance: textfield;
    transition: background-color 0.2s, border-color 0.2s;
}
.md3-datepicker-time-input:not(:focus) {
    background-color: var(--md-sys-color-input-idle-surface);
    border-color: transparent;
}
.md3-datepicker-time-input:focus {
    outline: none;
    background-color: var(--md-sys-color-primary-focus-surface);
    border-color: var(--md-sys-color-primary);
}
.md3-datepicker-time-input::-webkit-outer-spin-button,
.md3-datepicker-time-input::-webkit-inner-spin-button {
    -webkit-appearance: none;
    margin: 0;
}
.md3-datepicker-time-separator {
    color: var(--md-sys-color-on-surface-variant);
    font-size: 1.125rem;
    line-height: 1;
}
.md3-datepicker-actions {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 0.75rem;
    padding-top: 0.25rem;
}
.md3-datepicker-actions-right { display: flex; gap: 0.5rem; }
.md3-datepicker-monthyear { display: flex; flex-direction: column; gap: 0.75rem; }
.md3-datepicker-year {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 0.5rem;
}
.md3-datepicker-year-input {
    max-width: 8rem;
    text-align: center;
    appearance: textfield;
    -moz-appearance: textfield;
}
.md3-datepicker-year-input::-webkit-outer-spin-button,
.md3-datepicker-year-input::-webkit-inner-spin-button {
    -webkit-appearance: none;
    margin: 0;
}
.md3-datepicker-months {
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 0.5rem;
}
.md3-datepicker-month {
    height: 2.5rem;
    border-radius: 9999px;
    background: transparent;
    color: var(--md-sys-color-on-surface);
    cursor: pointer;
    transition: background-color 0.15s, border-color 0.15s;
    border: none;
    padding: 0 0.5rem;
    text-align: center;
    font-size: 0.85rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
}
.md3-datepicker-month:hover { background-color: rgba(208, 188, 255, 0.08); }
.md3-datepicker-month-selected {
    background-color: var(--md-sys-color-primary-container);
    color: var(--md-sys-color-on-primary-container);
}
.md3-daterange-time {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 0.75rem;
}
.md3-daterange-time-row {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-top: 0.5rem;
}
"#;

pub(crate) fn ensure_datepicker_styles() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    if document.get_element_by_id(DATEPICKER_STYLE_ID).is_some() {
        return;
    }
    let Ok(style_element) = document.create_element("style") else {
        return;
    };
    let _ = style_element.set_attribute("id", DATEPICKER_STYLE_ID);
    style_element.set_text_content(Some(DATEPICKER_CSS));
    if let Some(body) = document.body() {
        let _ = body.append_child(&style_element);
    }
}

#[derive(Clone, Copy, PartialEq)]
pub enum DatePickerType {
    Date,
    DateTimeLocal,
}

impl DatePickerType {
    fn is_datetime(self) -> bool {
        matches!(self, DatePickerType::DateTimeLocal)
    }
}

#[derive(Properties, PartialEq)]
pub struct DatePickerProps {
    pub label: String,
    pub value: String,
    pub onchange: Callback<String>,
    #[prop_or(DatePickerType::Date)]
    pub picker_type: DatePickerType,
    #[prop_or(None)]
    pub min: Option<String>,
    #[prop_or(None)]
    pub max: Option<String>,
    #[prop_or(false)]
    pub disabled: bool,
    #[prop_or(true)]
    pub show_trigger_button: bool,
    #[prop_or(None)]
    pub error: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum PickerView {
    Calendar,
    MonthYear,
}

#[derive(Clone, PartialEq)]
struct Draft {
    date: Option<NaiveDate>,
    hour: u32,
    minute: u32,
}

impl Draft {
    fn empty() -> Self {
        Self {
            date: None,
            hour: 0,
            minute: 0,
        }
    }
}

fn today_local() -> NaiveDate {
    let now = JsDate::new_0();
    let year = now.get_full_year() as i32;
    let month = (now.get_month() as u32) + 1; // 0-based in JS
    let day = now.get_date() as u32;
    NaiveDate::from_ymd_opt(year, month, day)
        .unwrap_or_else(|| NaiveDate::from_ymd_opt(1970, 1, 1).unwrap())
}

fn parse_value(picker_type: DatePickerType, value: &str) -> Option<Draft> {
    if value.trim().is_empty() {
        return None;
    }

    match picker_type {
        DatePickerType::Date => {
            let date = NaiveDate::parse_from_str(value.trim(), "%Y-%m-%d").ok()?;
            Some(Draft {
                date: Some(date),
                hour: 0,
                minute: 0,
            })
        }
        DatePickerType::DateTimeLocal => {
            let dt = NaiveDateTime::parse_from_str(value.trim(), "%Y-%m-%dT%H:%M").ok()?;
            Some(Draft {
                date: Some(dt.date()),
                hour: dt.hour(),
                minute: dt.minute(),
            })
        }
    }
}

fn format_value(picker_type: DatePickerType, draft: &Draft) -> String {
    let Some(date) = draft.date else {
        return String::new();
    };

    match picker_type {
        DatePickerType::Date => date.format("%Y-%m-%d").to_string(),
        DatePickerType::DateTimeLocal => {
            let dt = date
                .and_hms_opt(draft.hour.min(23), draft.minute.min(59), 0)
                .unwrap_or_else(|| date.and_hms_opt(0, 0, 0).unwrap());
            dt.format("%Y-%m-%dT%H:%M").to_string()
        }
    }
}

fn display_value(picker_type: DatePickerType, value: &str) -> String {
    if value.trim().is_empty() {
        return String::new();
    }
    match picker_type {
        DatePickerType::Date => value.to_string(),
        DatePickerType::DateTimeLocal => value.replace('T', " "),
    }
}

fn days_in_month(year: i32, month: u32) -> u32 {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let next_first = NaiveDate::from_ymd_opt(next_year, next_month, 1).unwrap();
    let last = next_first - Duration::days(1);
    last.day()
}

fn clamp_month(year: i32, month: i32) -> (i32, u32) {
    if month < 1 {
        (year - 1, 12)
    } else if month > 12 {
        (year + 1, 1)
    } else {
        (year, month as u32)
    }
}

fn parse_limit_date(picker_type: DatePickerType, value: &Option<String>) -> Option<NaiveDate> {
    let s = value.as_ref()?.trim();
    if s.is_empty() {
        return None;
    }
    match picker_type {
        DatePickerType::Date => NaiveDate::parse_from_str(s, "%Y-%m-%d").ok(),
        DatePickerType::DateTimeLocal => NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M")
            .ok()
            .map(|dt| dt.date()),
    }
}

fn is_outside_limits(d: NaiveDate, min: Option<NaiveDate>, max: Option<NaiveDate>) -> bool {
    if let Some(min) = min {
        if d < min {
            return true;
        }
    }
    if let Some(max) = max {
        if d > max {
            return true;
        }
    }
    false
}

fn chevron_icon(direction: &str) -> Html {
    let path = if direction == "left" {
        "M15.41 7.41 14 6l-6 6 6 6 1.41-1.41L10.83 12z"
    } else {
        "M8.59 16.59 10 18l6-6-6-6-1.41 1.41L13.17 12z"
    };

    html! {
        <svg class="md3-datepicker-nav-icon" viewBox="0 0 24 24" aria-hidden="true" focusable="false">
            <path d={path} fill="currentColor" />
        </svg>
    }
}

#[function_component(DatePicker)]
pub fn date_picker(props: &DatePickerProps) -> Html {
    use_effect_with((), move |_| {
        ensure_datepicker_styles();
        || ()
    });

    let open = use_state(|| false);
    let view = use_state(|| PickerView::Calendar);

    let draft =
        use_state(|| parse_value(props.picker_type, &props.value).unwrap_or_else(Draft::empty));

    {
        let draft = draft.clone();
        let value = props.value.clone();
        let picker_type = props.picker_type;
        let open = open.clone();
        use_effect_with(
            (value, picker_type, *open),
            move |(value, picker_type, open)| {
                // Keep internal state in sync with external value when the popup is closed.
                if !*open {
                    if let Some(next) = parse_value(*picker_type, value) {
                        draft.set(next);
                    } else {
                        draft.set(Draft::empty());
                    }
                }
                || ()
            },
        );
    }

    let view_month = use_state(|| {
        let base = today_local();
        (base.year(), base.month())
    });

    let open_popup = {
        let open = open.clone();
        let view = view.clone();
        let view_month = view_month.clone();
        let draft = draft.clone();
        let disabled = props.disabled;
        Callback::from(move |_| {
            if disabled {
                return;
            }
            let base = (*draft).date.unwrap_or_else(today_local);
            view_month.set((base.year(), base.month()));
            view.set(PickerView::Calendar);
            open.set(true);
        })
    };

    let close_popup = {
        let open = open.clone();
        Callback::from(move |_: ()| open.set(false))
    };

    let close_popup_btn = {
        let open = open.clone();
        Callback::from(move |_: MouseEvent| open.set(false))
    };

    let on_clear = {
        let onchange = props.onchange.clone();
        let open = open.clone();
        Callback::from(move |_| {
            onchange.emit(String::new());
            open.set(false);
        })
    };

    let min_date = parse_limit_date(props.picker_type, &props.min);
    let max_date = parse_limit_date(props.picker_type, &props.max);

    let label_id = format!("label-{}", props.label.replace(' ', "-").to_lowercase());

    let error_html = if let Some(error_msg) = &props.error {
        html! { <div class="text-sm mt-2" style="color: #F2B8B5;">{ error_msg }</div> }
    } else {
        html! {}
    };

    let (year, month) = *view_month;
    let first = NaiveDate::from_ymd_opt(year, month, 1).unwrap();
    let offset = first.weekday().num_days_from_monday() as i32;
    let dim = days_in_month(year, month) as i32;

    let month_names = [
        "January",
        "February",
        "March",
        "April",
        "May",
        "June",
        "July",
        "August",
        "September",
        "October",
        "November",
        "December",
    ];
    let month_label = format!("{} {}", month_names[(month - 1) as usize], year);

    let prev_month = {
        let view_month = view_month.clone();
        Callback::from(move |_| {
            let (y, m) = *view_month;
            let (ny, nm) = clamp_month(y, (m as i32) - 1);
            view_month.set((ny, nm));
        })
    };

    let next_month = {
        let view_month = view_month.clone();
        Callback::from(move |_| {
            let (y, m) = *view_month;
            let (ny, nm) = clamp_month(y, (m as i32) + 1);
            view_month.set((ny, nm));
        })
    };

    let open_month_year = {
        let view = view.clone();
        Callback::from(move |_| view.set(PickerView::MonthYear))
    };

    let back_to_calendar = {
        let view = view.clone();
        Callback::from(move |_| view.set(PickerView::Calendar))
    };

    let year_dec = {
        let view_month = view_month.clone();
        Callback::from(move |_| {
            let (y, m) = *view_month;
            view_month.set((y - 1, m));
        })
    };

    let year_inc = {
        let view_month = view_month.clone();
        Callback::from(move |_| {
            let (y, m) = *view_month;
            view_month.set((y + 1, m));
        })
    };

    let on_year_input = {
        let view_month = view_month.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target().unwrap();
            let value = js_sys::Reflect::get(&target, &"value".into())
                .ok()
                .and_then(|v| v.as_string())
                .unwrap_or_default();
            if let Ok(y) = value.parse::<i32>() {
                let (_, m) = *view_month;
                view_month.set((y, m));
            }
        })
    };

    let on_pick_month = {
        let view_month = view_month.clone();
        let view = view.clone();
        Callback::from(move |month: u32| {
            let (y, _) = *view_month;
            view_month.set((y, month));
            view.set(PickerView::Calendar);
        })
    };

    let is_selected = |d: NaiveDate, draft: &Draft| draft.date == Some(d);
    let on_pick_day = {
        let draft = draft.clone();
        let onchange = props.onchange.clone();
        let picker_type = props.picker_type;
        let open = open.clone();
        let view_month = view_month.clone();
        let min_date = min_date;
        let max_date = max_date;
        Callback::from(move |date: NaiveDate| {
            if is_outside_limits(date, min_date, max_date) {
                return;
            }

            view_month.set((date.year(), date.month()));

            let mut next = (*draft).clone();
            next.date = Some(date);
            draft.set(next.clone());

            if !picker_type.is_datetime() {
                onchange.emit(format_value(picker_type, &next));
                open.set(false);
            }
        })
    };

    let on_hour = {
        let draft = draft.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target().unwrap();
            let value = js_sys::Reflect::get(&target, &"value".into())
                .ok()
                .and_then(|v| v.as_string())
                .unwrap_or_default();
            let parsed = value.parse::<u32>().unwrap_or(0).min(23);
            let mut next = (*draft).clone();
            next.hour = parsed;
            draft.set(next);
        })
    };

    let on_minute = {
        let draft = draft.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target().unwrap();
            let value = js_sys::Reflect::get(&target, &"value".into())
                .ok()
                .and_then(|v| v.as_string())
                .unwrap_or_default();
            let parsed = value.parse::<u32>().unwrap_or(0).min(59);
            let mut next = (*draft).clone();
            next.minute = parsed;
            draft.set(next);
        })
    };

    let on_apply_datetime = {
        let draft = draft.clone();
        let onchange = props.onchange.clone();
        let picker_type = props.picker_type;
        let open = open.clone();
        Callback::from(move |_| {
            onchange.emit(format_value(picker_type, &*draft));
            open.set(false);
        })
    };

    let visible_start = first - Duration::days(offset as i64);
    let total_cells = ((offset + dim + 6) / 7) * 7;
    let calendar = (0..total_cells)
        .map(|i| {
            let date = visible_start + Duration::days(i as i64);
            let in_current_month = date.month() == month;
            let selected = is_selected(date, &*draft);
            let disabled = is_outside_limits(date, min_date, max_date) || props.disabled;
            let class = classes!(
                "md3-datepicker-day",
                if selected {
                    "md3-datepicker-day-selected"
                } else {
                    ""
                },
                if !in_current_month {
                    "md3-datepicker-day-outside"
                } else {
                    ""
                },
                if disabled {
                    "md3-datepicker-day-disabled"
                } else {
                    ""
                },
            );

            let on_pick_day = on_pick_day.clone();
            html! {
                <button
                    type="button"
                    class={class}
                    disabled={disabled}
                    onclick={Callback::from(move |_| on_pick_day.emit(date))}
                >
                    { date.day().to_string() }
                </button>
            }
        })
        .collect::<Html>();

    let datetime_controls = if props.picker_type.is_datetime() {
        html! {
            <div class="md3-datepicker-time">
                <div class="md3-datepicker-time-field">
                    <div class="md3-datepicker-time-row">
                        <input class="md3-input md3-datepicker-time-input" type="number" min="0" max="23" value={draft.hour.to_string()} oninput={on_hour} />
                        <span class="md3-datepicker-time-separator" aria-hidden="true">{ ":" }</span>
                        <input class="md3-input md3-datepicker-time-input" type="number" min="0" max="59" value={draft.minute.to_string()} oninput={on_minute} />
                    </div>
                </div>
            </div>
        }
    } else {
        html! {}
    };

    let footer = if props.picker_type.is_datetime() {
        html! {
            <div class="md3-datepicker-actions">
                <Button label="Clear" button_type={ButtonType::Text} onclick={on_clear.clone()} />
                <div class="md3-datepicker-actions-right">
                    <Button label="Cancel" button_type={ButtonType::Text} onclick={close_popup_btn.clone()} />
                    <Button label="Apply" button_type={ButtonType::Filled} onclick={on_apply_datetime} />
                </div>
            </div>
        }
    } else {
        html! {
            <div class="md3-datepicker-actions">
                <Button label="Clear" button_type={ButtonType::Text} onclick={on_clear.clone()} />
                <Button label="Close" button_type={ButtonType::Text} onclick={close_popup_btn.clone()} />
            </div>
        }
    };

    let month_year_view = {
        let months = [
            "January",
            "February",
            "March",
            "April",
            "May",
            "June",
            "July",
            "August",
            "September",
            "October",
            "November",
            "December",
        ];
        let (y, m) = *view_month;
        let on_pick_month = on_pick_month.clone();

        html! {
            <div class="md3-datepicker-monthyear">
                <div class="md3-datepicker-year">
                    <button type="button" class="md3-datepicker-nav-btn" onclick={year_dec.clone()} aria-label="Previous year">{ chevron_icon("left") }</button>
                    <input
                        class="md3-input md3-datepicker-year-input"
                        type="number"
                        value={y.to_string()}
                        oninput={on_year_input.clone()}
                    />
                    <button type="button" class="md3-datepicker-nav-btn" onclick={year_inc.clone()} aria-label="Next year">{ chevron_icon("right") }</button>
                </div>

                <div class="md3-datepicker-months">
                    { for months.iter().enumerate().map(|(idx, label)| {
                        let month = (idx as u32) + 1;
                        let class = classes!(
                            "md3-datepicker-month",
                            if month == m { "md3-datepicker-month-selected" } else { "" },
                        );
                        let on_pick_month = on_pick_month.clone();
                        html! {
                            <button
                                type="button"
                                class={class}
                                onclick={Callback::from(move |_| on_pick_month.emit(month))}
                            >
                                { *label }
                            </button>
                        }
                    }) }
                </div>

                <div class="md3-datepicker-actions">
                    <Button label="Back" button_type={ButtonType::Text} onclick={back_to_calendar.clone()} />
                    <Button label="Close" button_type={ButtonType::Text} onclick={close_popup_btn.clone()} />
                </div>
            </div>
        }
    };

    html! {
        <div class="w-full">
            <label id={label_id} class="block text-sm font-medium mb-1 text-on-surface">
                { &props.label }
            </label>
            <div class="md3-datepicker-input">
                <input
                    class="md3-input"
                    readonly={true}
                    value={display_value(props.picker_type, &props.value)}
                    disabled={props.disabled}
                    onclick={open_popup.clone()}
                />
                {
                    if props.show_trigger_button {
                        html! {
                            <button
                                type="button"
                                class="md3-datepicker-open"
                                onclick={open_popup}
                                disabled={props.disabled}
                                aria-label="Open date picker"
                            >
                                { "CAL" }
                            </button>
                        }
                    } else {
                        html! {}
                    }
                }
            </div>

            { if *open {
                html! {
                    <Popup
                        title={"Select date"}
                        size={PopupSize::Sm}
                        on_close={close_popup.clone()}
                    >
                        <div class="md3-datepicker">
                            <div class="md3-datepicker-nav">
                                <button type="button" class="md3-datepicker-nav-btn" onclick={prev_month.clone()} aria-label="Previous month">{ chevron_icon("left") }</button>
                                <button type="button" class="md3-datepicker-nav-label-btn" onclick={open_month_year.clone()} aria-label="Select month and year">
                                    { month_label.clone() }
                                </button>
                                <button type="button" class="md3-datepicker-nav-btn" onclick={next_month.clone()} aria-label="Next month">{ chevron_icon("right") }</button>
                            </div>

                            {
                                if *view == PickerView::MonthYear {
                                    month_year_view
                                } else {
                                    html! {
                                        <>
                                            <div class="md3-datepicker-weekdays">
                                                <div>{ "Mon" }</div>
                                                <div>{ "Tue" }</div>
                                                <div>{ "Wed" }</div>
                                                <div>{ "Thu" }</div>
                                                <div>{ "Fri" }</div>
                                                <div>{ "Sat" }</div>
                                                <div>{ "Sun" }</div>
                                            </div>
                                            <div class="md3-datepicker-grid">
                                                { calendar }
                                            </div>
                                            { datetime_controls }
                                            { footer }
                                        </>
                                    }
                                }
                            }
                        </div>
                    </Popup>
                }
            } else {
                html! {}
            }}
            { error_html }
        </div>
    }
}
