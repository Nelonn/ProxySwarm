use chrono::{Datelike, Duration, NaiveDate, NaiveDateTime, Timelike};
use js_sys::Date as JsDate;
use yew::prelude::*;

use super::{
    date_picker::ensure_datepicker_styles, Button, ButtonType, DatePickerType, Popup, PopupSize,
};

#[derive(Properties, PartialEq)]
pub struct DateRangePickerProps {
    pub label: String,
    pub start: String,
    pub end: String,
    pub onchange: Callback<(String, String)>,
    #[prop_or(DatePickerType::Date)]
    pub picker_type: DatePickerType,
    #[prop_or(None)]
    pub min: Option<String>,
    #[prop_or(None)]
    pub max: Option<String>,
    #[prop_or(false)]
    pub disabled: bool,
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

#[derive(Clone, PartialEq)]
struct RangeDraft {
    start: Draft,
    end: Draft,
}

impl RangeDraft {
    fn empty() -> Self {
        Self {
            start: Draft::empty(),
            end: Draft::empty(),
        }
    }
}

fn today_local() -> NaiveDate {
    let now = JsDate::new_0();
    let year = now.get_full_year() as i32;
    let month = (now.get_month() as u32) + 1;
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

fn normalize_range(a: NaiveDate, b: NaiveDate) -> (NaiveDate, NaiveDate) {
    if a <= b {
        (a, b)
    } else {
        (b, a)
    }
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

#[function_component(DateRangePicker)]
pub fn date_range_picker(props: &DateRangePickerProps) -> Html {
    use_effect_with((), move |_| {
        ensure_datepicker_styles();
        || ()
    });

    let open = use_state(|| false);
    let view = use_state(|| PickerView::Calendar);

    let draft = use_state(|| {
        let mut d = RangeDraft::empty();
        if let Some(s) = parse_value(props.picker_type, &props.start) {
            d.start = s;
        }
        if let Some(e) = parse_value(props.picker_type, &props.end) {
            d.end = e;
        }
        d
    });

    {
        let draft = draft.clone();
        let start = props.start.clone();
        let end = props.end.clone();
        let picker_type = props.picker_type;
        let open = open.clone();
        use_effect_with(
            (start, end, picker_type, *open),
            move |(start, end, picker_type, open)| {
                if !*open {
                    let mut d = RangeDraft::empty();
                    if let Some(s) = parse_value(*picker_type, start) {
                        d.start = s;
                    }
                    if let Some(e) = parse_value(*picker_type, end) {
                        d.end = e;
                    }
                    draft.set(d);
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
            let base = (*draft)
                .start
                .date
                .or((*draft).end.date)
                .unwrap_or_else(today_local);
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
        Callback::from(move |_: MouseEvent| {
            onchange.emit((String::new(), String::new()));
            open.set(false);
        })
    };

    let min_date = parse_limit_date(props.picker_type, &props.min);
    let max_date = parse_limit_date(props.picker_type, &props.max);

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
        Callback::from(move |_: MouseEvent| {
            let (y, m) = *view_month;
            let (ny, nm) = clamp_month(y, (m as i32) - 1);
            view_month.set((ny, nm));
        })
    };

    let next_month = {
        let view_month = view_month.clone();
        Callback::from(move |_: MouseEvent| {
            let (y, m) = *view_month;
            let (ny, nm) = clamp_month(y, (m as i32) + 1);
            view_month.set((ny, nm));
        })
    };

    let open_month_year = {
        let view = view.clone();
        Callback::from(move |_: MouseEvent| view.set(PickerView::MonthYear))
    };

    let back_to_calendar = {
        let view = view.clone();
        Callback::from(move |_: MouseEvent| view.set(PickerView::Calendar))
    };

    let year_dec = {
        let view_month = view_month.clone();
        Callback::from(move |_: MouseEvent| {
            let (y, m) = *view_month;
            view_month.set((y - 1, m));
        })
    };

    let year_inc = {
        let view_month = view_month.clone();
        Callback::from(move |_: MouseEvent| {
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

    let on_pick_day = {
        let draft = draft.clone();
        let min_date = min_date;
        let max_date = max_date;
        let view_month = view_month.clone();
        Callback::from(move |date: NaiveDate| {
            if is_outside_limits(date, min_date, max_date) {
                return;
            }

            view_month.set((date.year(), date.month()));

            let mut next = (*draft).clone();
            let start_set = next.start.date.is_some();
            let end_set = next.end.date.is_some();

            if !start_set || (start_set && end_set) {
                next.start.date = Some(date);
                next.end.date = None;
                draft.set(next);
                return;
            }

            // start is set, end is empty
            next.end.date = Some(date);
            let (a, b) = normalize_range(next.start.date.unwrap(), date);
            next.start.date = Some(a);
            next.end.date = Some(b);
            draft.set(next);
        })
    };

    let on_start_hour = {
        let draft = draft.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target().unwrap();
            let value = js_sys::Reflect::get(&target, &"value".into())
                .ok()
                .and_then(|v| v.as_string())
                .unwrap_or_default();
            let parsed = value.parse::<u32>().unwrap_or(0).min(23);
            let mut next = (*draft).clone();
            next.start.hour = parsed;
            draft.set(next);
        })
    };

    let on_start_minute = {
        let draft = draft.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target().unwrap();
            let value = js_sys::Reflect::get(&target, &"value".into())
                .ok()
                .and_then(|v| v.as_string())
                .unwrap_or_default();
            let parsed = value.parse::<u32>().unwrap_or(0).min(59);
            let mut next = (*draft).clone();
            next.start.minute = parsed;
            draft.set(next);
        })
    };

    let on_end_hour = {
        let draft = draft.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target().unwrap();
            let value = js_sys::Reflect::get(&target, &"value".into())
                .ok()
                .and_then(|v| v.as_string())
                .unwrap_or_default();
            let parsed = value.parse::<u32>().unwrap_or(0).min(23);
            let mut next = (*draft).clone();
            next.end.hour = parsed;
            draft.set(next);
        })
    };

    let on_end_minute = {
        let draft = draft.clone();
        Callback::from(move |e: InputEvent| {
            let target = e.target().unwrap();
            let value = js_sys::Reflect::get(&target, &"value".into())
                .ok()
                .and_then(|v| v.as_string())
                .unwrap_or_default();
            let parsed = value.parse::<u32>().unwrap_or(0).min(59);
            let mut next = (*draft).clone();
            next.end.minute = parsed;
            draft.set(next);
        })
    };

    let on_apply = {
        let draft = draft.clone();
        let picker_type = props.picker_type;
        let onchange = props.onchange.clone();
        let open = open.clone();
        Callback::from(move |_: MouseEvent| {
            let start = format_value(picker_type, &(*draft).start);
            let end = format_value(picker_type, &(*draft).end);
            onchange.emit((start, end));
            open.set(false);
        })
    };

    let (range_start, range_end) = match ((*draft).start.date, (*draft).end.date) {
        (Some(a), Some(b)) => {
            let (s, e) = normalize_range(a, b);
            (Some(s), Some(e))
        }
        (Some(a), None) => (Some(a), None),
        _ => (None, None),
    };

    let visible_start = first - Duration::days(offset as i64);
    let total_cells = ((offset + dim + 6) / 7) * 7;
    let calendar = (0..total_cells)
        .map(|i| {
            let date = visible_start + Duration::days(i as i64);
            let in_current_month = date.month() == month;

            let disabled = is_outside_limits(date, min_date, max_date) || props.disabled;
            let in_range = match (range_start, range_end) {
                (Some(s), Some(e)) => date >= s && date <= e,
                _ => false,
            };
            let is_edge = match (range_start, range_end) {
                (Some(s), Some(e)) => date == s || date == e,
                (Some(s), None) => date == s,
                _ => false,
            };

            let class = classes!(
                "md3-datepicker-day",
                if in_range {
                    "md3-datepicker-day-inrange"
                } else {
                    ""
                },
                if is_edge {
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

    let datetime_controls = if matches!(props.picker_type, DatePickerType::DateTimeLocal) {
        html! {
            <div class="md3-daterange-time">
                <div class="md3-daterange-time-col">
                    <div class="text-sm font-medium">{ "Start time" }</div>
                    <div class="md3-daterange-time-row">
                        <input class="md3-input md3-datepicker-time-input" type="number" min="0" max="23" value={(*draft).start.hour.to_string()} oninput={on_start_hour} />
                        <span class="md3-datepicker-time-separator" aria-hidden="true">{ ":" }</span>
                        <input class="md3-input md3-datepicker-time-input" type="number" min="0" max="59" value={(*draft).start.minute.to_string()} oninput={on_start_minute} />
                    </div>
                </div>
                <div class="md3-daterange-time-col">
                    <div class="text-sm font-medium">{ "End time" }</div>
                    <div class="md3-daterange-time-row">
                        <input class="md3-input md3-datepicker-time-input" type="number" min="0" max="23" value={(*draft).end.hour.to_string()} oninput={on_end_hour} />
                        <span class="md3-datepicker-time-separator" aria-hidden="true">{ ":" }</span>
                        <input class="md3-input md3-datepicker-time-input" type="number" min="0" max="59" value={(*draft).end.minute.to_string()} oninput={on_end_minute} />
                    </div>
                </div>
            </div>
        }
    } else {
        html! {}
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
                    <button type="button" class="md3-datepicker-nav-btn" onclick={year_dec} aria-label="Previous year">{ chevron_icon("left") }</button>
                    <input
                        class="md3-input md3-datepicker-year-input"
                        type="number"
                        value={y.to_string()}
                        oninput={on_year_input}
                    />
                    <button type="button" class="md3-datepicker-nav-btn" onclick={year_inc} aria-label="Next year">{ chevron_icon("right") }</button>
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
                    <Button label="Back" button_type={ButtonType::Text} onclick={back_to_calendar} />
                    <Button label="Close" button_type={ButtonType::Text} onclick={close_popup_btn.clone()} />
                </div>
            </div>
        }
    };

    let error_html = if let Some(error_msg) = &props.error {
        html! { <div class="text-sm mt-2" style="color: #F2B8B5;">{ error_msg }</div> }
    } else {
        html! {}
    };

    let display = {
        let s = display_value(props.picker_type, &props.start);
        let e = display_value(props.picker_type, &props.end);
        if s.is_empty() && e.is_empty() {
            String::new()
        } else if !s.is_empty() && e.is_empty() {
            format!("{} -", s)
        } else if s.is_empty() && !e.is_empty() {
            format!("- {}", e)
        } else {
            format!("{} - {}", s, e)
        }
    };

    let label_id = format!("label-{}", props.label.replace(' ', "-").to_lowercase());

    html! {
        <div class="w-full">
            <label id={label_id} class="block text-sm font-medium mb-1 text-on-surface">
                { &props.label }
            </label>

            <div class="md3-datepicker-input">
                <input
                    class="md3-input"
                    readonly={true}
                    value={display}
                    disabled={props.disabled}
                    onclick={open_popup.clone()}
                />
                <button
                    type="button"
                    class="md3-datepicker-open"
                    onclick={open_popup}
                    disabled={props.disabled}
                    aria-label="Open date range picker"
                >
                    { "RNG" }
                </button>
            </div>

            { if *open {
                html! {
                    <Popup
                        title={"Select range"}
                        size={PopupSize::Sm}
                        on_close={close_popup}
                    >
                        <div class="md3-datepicker">
                            <div class="md3-datepicker-nav">
                                <button type="button" class="md3-datepicker-nav-btn" onclick={prev_month} aria-label="Previous month">{ chevron_icon("left") }</button>
                                <button type="button" class="md3-datepicker-nav-label-btn" onclick={open_month_year} aria-label="Select month and year">
                                    { month_label }
                                </button>
                                <button type="button" class="md3-datepicker-nav-btn" onclick={next_month} aria-label="Next month">{ chevron_icon("right") }</button>
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
                                            <div class="md3-datepicker-actions">
                                                <Button label="Clear" button_type={ButtonType::Text} onclick={on_clear} />
                                                <div class="md3-datepicker-actions-right">
                                                    <Button label="Cancel" button_type={ButtonType::Text} onclick={close_popup_btn.clone()} />
                                                    <Button label="Apply" button_type={ButtonType::Filled} onclick={on_apply} />
                                                </div>
                                            </div>
                                        </>
                                    }
                                }
                            }
                        </div>
                    </Popup>
                }
            } else { html!{} } }

            { error_html }
        </div>
    }
}
