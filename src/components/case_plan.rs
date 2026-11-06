use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::components::case_overview::month_day;
use crate::server_fns::case_plan::{
    add_plan_action, list_case_plan, list_plan_assignees, record_task_outcome, CasePlanAction,
    CaseTask, PlanAssignee, TaskOutcome,
};
use crate::state::{now_stamp, AppState};

const CARD: &str =
    "rounded-card border border-slate-800 bg-slate-900 px-6 pt-[22px] pb-5 shadow-card";
const PILL: &str =
    "inline-flex items-center gap-1.5 rounded-full px-[11px] py-[3px] text-[13px] font-semibold leading-5 whitespace-nowrap";
const FORM: &str = "grid gap-x-4 gap-y-3 rounded-[14px] bg-[#fdfaf5] p-4 sm:grid-cols-2 items-end";
const FIELD: &str = "flex flex-col gap-1.5";
const FIELD_LABEL: &str = "text-[13.5px] font-semibold text-slate-400";
const FIELD_INPUT: &str = "min-h-[42px] rounded-xl border border-slate-700 bg-slate-900 px-3.5 py-2 text-[14.5px] text-slate-100 focus:border-primary-500 focus:outline-none";
const PRIMARY: &str = "inline-flex min-h-[34px] items-center rounded-full bg-primary-600 px-3.5 text-[13.5px] font-semibold text-white hover:bg-primary-700";
const SECONDARY: &str = "inline-flex min-h-[34px] items-center gap-1.5 rounded-full border border-slate-700 bg-slate-900 px-3.5 text-[13.5px] font-semibold text-slate-200 hover:bg-slate-950";

#[component]
pub fn CasePlanCard(case_id: String, can_edit: bool) -> impl IntoView {
    let state = expect_context::<AppState>();
    let viewer_id = StoredValue::new(
        state
            .current_user_summary
            .with_untracked(|user| user.as_ref().map(|user| user.id.clone()))
            .unwrap_or_default(),
    );
    let case_sv = StoredValue::new(case_id);
    let plan = RwSignal::new(Vec::<CasePlanAction>::new());
    let assignees = RwSignal::new(Vec::<PlanAssignee>::new());
    let adding = RwSignal::new(false);
    let error = RwSignal::new(String::new());

    let reload = move || {
        spawn_local(async move {
            if let Ok(found) = list_case_plan(case_sv.get_value()).await {
                plan.set(found);
            }
        })
    };
    reload();
    if can_edit {
        spawn_local(async move {
            if let Ok(found) = list_plan_assignees(case_sv.get_value()).await {
                assignees.set(found);
            }
        });
    }

    let title = RwSignal::new(String::new());
    let assignee = RwSignal::new(String::new());
    let follow_up = RwSignal::new(String::new());
    let save_action = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        spawn_local(async move {
            match add_plan_action(
                case_sv.get_value(),
                title.get(),
                assignee.get(),
                follow_up.get(),
            )
            .await
            {
                Ok(()) => {
                    title.set(String::new());
                    follow_up.set(String::new());
                    error.set(String::new());
                    adding.set(false);
                    reload();
                }
                Err(e) => error.set(e.to_string()),
            }
        });
    };

    let add_form = move || {
        adding.get().then(|| {
            view! {
                <form on:submit=save_action aria-label="Add an action" class=format!("mt-4 {FORM}")>
                    <div class=format!("{FIELD} sm:col-span-2")>
                        <label for="plan-title" class=FIELD_LABEL>"Action"</label>
                        <input id="plan-title" class=FIELD_INPUT required bind:value=title placeholder="What needs to happen" />
                    </div>
                    <div class=FIELD>
                        <label for="plan-assignee" class=FIELD_LABEL>"Assigned to"</label>
                        <select id="plan-assignee" class=FIELD_INPUT required bind:value=assignee>
                            <option value="">"Choose someone on this case"</option>
                            {move || {
                                assignees
                                    .get()
                                    .into_iter()
                                    .map(|a| view! { <option value=a.id>{a.name}</option> })
                                    .collect::<Vec<_>>()
                            }}
                        </select>
                    </div>
                    <div class=FIELD>
                        <label for="plan-follow-up" class=FIELD_LABEL>"Follow-up date"</label>
                        <input id="plan-follow-up" type="date" class=FIELD_INPUT bind:value=follow_up />
                    </div>
                    <div class="flex gap-2 sm:col-span-2">
                        <button type="submit" class=PRIMARY>"Save action"</button>
                        <button type="button" class=SECONDARY on:click=move |_| adding.set(false)>"Cancel"</button>
                    </div>
                </form>
            }
        })
    };

    let actions = move || {
        let items = plan.get();
        if items.is_empty() {
            return view! { <p class="mt-4 text-sm text-slate-500">"No approved actions yet."</p> }
                .into_any();
        }
        view! {
            <ul class="mt-4 flex flex-col gap-3">
                {items
                    .into_iter()
                    .flat_map(|action| {
                        let added = format!(
                            "Added by {}, {}",
                            action.added_by_name,
                            month_day(&action.added_on),
                        );
                        let title = action.title;
                        action.tasks.into_iter().map(move |task| {
                            let can_record = can_edit
                                || viewer_id.with_value(|v| *v == task.assignee_id);
                            view! {
                                <TaskItem
                                    title=title.clone()
                                    added=added.clone()
                                    task=task
                                    can_record=can_record
                                    on_saved=Callback::new(move |_| reload())
                                />
                            }
                        })
                    })
                    .collect::<Vec<_>>()}
            </ul>
        }
        .into_any()
    };

    view! {
        <section class=CARD aria-labelledby="case-plan-heading">
            <div class="-mx-6 flex items-start justify-between gap-3 border-b border-slate-800 px-6 pb-3.5">
                <div class="min-w-0 flex-1">
                    <p class="flex flex-wrap items-center gap-2">
                        <span class=format!("{PILL} bg-[#e3f5ec] text-[#11633f]")>
                            <svg class="h-[13px] w-[13px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><circle cx="12" cy="8" r="4"></circle><path d="M4 21c1.5-4 4.5-6 8-6s6.5 2 8 6"></path></svg>
                            "Approved by staff"
                        </span>
                        <span class="inline-flex items-center gap-[5px] rounded-full border border-[#fcd34d] bg-[#fffdf5] px-2.5 py-0.5 text-[12.5px] font-bold text-[#7a4a06]">
                            <svg class="h-[13px] w-[13px]" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" aria-hidden="true"><rect x="5" y="11" width="14" height="10" rx="2"></rect><path d="M8 11V7a4 4 0 0 1 8 0v4"></path></svg>
                            "Volunteers only"
                        </span>
                    </p>
                    <h2 id="case-plan-heading" class="mt-2.5 text-2xl font-semibold">"Approved case plan"</h2>
                    <p class="mt-0.5 text-[13.5px] text-slate-500">
                        "Actions the team has approved and the tasks assigned for them. AI proposals appear here only after a person approves them."
                    </p>
                </div>
                {can_edit
                    .then(|| {
                        view! {
                            <button type="button" class=format!("shrink-0 {SECONDARY}") on:click=move |_| adding.set(true)>
                                <svg class="h-4 w-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" stroke-linecap="round" aria-hidden="true"><path d="M12 5v14M5 12h14"></path></svg>
                                "Add action"
                            </button>
                        }
                    })}
            </div>
            {add_form}
            <Show when=move || !error.get().is_empty()>
                <p role="alert" class="mt-3 text-sm text-rose-300">{move || error.get()}</p>
            </Show>
            {actions}
        </section>
    }
}

#[component]
fn TaskItem(
    title: String,
    added: String,
    task: CaseTask,
    can_record: bool,
    on_saved: Callback<()>,
) -> impl IntoView {
    let error = RwSignal::new(String::new());
    let editing = RwSignal::new(false);
    let initial_outcome = task
        .outcome
        .map(|o| o.slug().to_string())
        .unwrap_or_default();
    let initial_notes = task.outcome_notes.clone();
    let initial_follow_up = task.follow_up_on.clone();
    let outcome = RwSignal::new(initial_outcome.clone());
    let notes = RwSignal::new(initial_notes.clone());
    let follow_up = RwSignal::new(initial_follow_up.clone());
    let task_id = StoredValue::new(task.id.clone());
    let form_id = StoredValue::new(format!("outcome-{}", task.id));
    let form_label = StoredValue::new(format!("Record outcome for {title}"));
    let initial = StoredValue::new((
        initial_outcome.clone(),
        initial_notes.clone(),
        initial_follow_up.clone(),
    ));

    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        spawn_local(async move {
            let chosen = TaskOutcome::from_slug(&outcome.get());
            match record_task_outcome(task_id.get_value(), chosen, notes.get(), follow_up.get())
                .await
            {
                Ok(()) => {
                    editing.set(false);
                    on_saved.run(());
                }
                Err(e) => error.set(e.to_string()),
            }
        });
    };
    let reset = move |_| {
        initial.with_value(|(o, n, f)| {
            outcome.set(o.clone());
            notes.set(n.clone());
            follow_up.set(f.clone());
        });
        error.set(String::new());
        editing.set(false);
    };

    let status_pill = match task.outcome {
        None => view! {
            <span class=format!("{PILL} bg-[#fef3c7] text-[#7a4a06]")>
                <svg class="h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.6" aria-hidden="true"><circle cx="12" cy="12" r="9"></circle><path d="M12 7v5l3 2"></path></svg>
                "Active"
            </span>
        }
        .into_any(),
        Some(TaskOutcome::Completed) => view! {
            <span class=format!("{PILL} bg-[#e3f5ec] text-[#11633f]")>
                <svg class="h-3 w-3" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="3" aria-hidden="true"><path d="M5 12l5 5L20 7"></path></svg>
                {format!("Completed {}", month_day(&task.outcome_recorded_on))}
            </span>
        }
        .into_any(),
        Some(other) => view! {
            <span class=format!("{PILL} bg-[#f3ece1] text-[#5b4f43]")>{other.label()}</span>
        }
        .into_any(),
    };
    let active = task.outcome.is_none();
    let overdue =
        active && !task.follow_up_on.is_empty() && task.follow_up_on.as_str() < &now_stamp()[..10];

    view! {
        <li class="flex flex-col gap-3.5 rounded-2xl border border-slate-800 px-[18px] py-4">
            <div class="flex flex-wrap items-start justify-between gap-x-4 gap-y-2.5">
                <div class="min-w-0 flex-[1_1_320px]">
                    <p class="text-base font-bold">{title.clone()}</p>
                    <p class="text-[13.5px] text-slate-500">
                        {format!("Assigned to {} \u{b7} {added}", task.assignee_name)}
                    </p>
                    {(!active)
                        .then(|| {
                            view! {
                                <p class="mt-1.5 text-[13.5px]">
                                    <strong>"Outcome: "</strong>
                                    {(!task.outcome_notes.is_empty())
                                        .then(|| format!("{} ", task.outcome_notes))}
                                    <span class="text-slate-500">
                                        {format!("Recorded {}", month_day(&task.outcome_recorded_on))}
                                    </span>
                                </p>
                            }
                        })}
                </div>
                <p class="flex flex-wrap gap-1.5">
                    {status_pill}
                    {(active && !task.follow_up_on.is_empty())
                        .then(|| {
                            let (label, colors) = if overdue {
                                ("Overdue since", "bg-[#fbe4e1] text-[#9b2c22]")
                            } else {
                                ("Follow up", "bg-[#f3ece1] text-[#5b4f43]")
                            };
                            view! {
                                <span class=format!("{PILL} {colors}")>
                                    {format!("{label} {}", month_day(&task.follow_up_on))}
                                </span>
                            }
                        })}
                </p>
            </div>
            {(active && can_record)
                .then(|| {
                    view! {
                        <Show when=move || !editing.get()>
                            <button type="button" class=format!("self-start {SECONDARY}") on:click=move |_| editing.set(true)>
                                "Record outcome"
                            </button>
                        </Show>
                        <Show when=move || editing.get()>
                        <form on:submit=save aria-label=form_label.get_value() class=FORM>
                            <div class=FIELD>
                                <label for=format!("{}-date", form_id.get_value()) class=FIELD_LABEL>"Follow-up date"</label>
                                <input id=format!("{}-date", form_id.get_value()) type="date" class=FIELD_INPUT bind:value=follow_up />
                            </div>
                            <div class=FIELD>
                                <label for=format!("{}-outcome", form_id.get_value()) class=FIELD_LABEL>"Outcome"</label>
                                <select id=format!("{}-outcome", form_id.get_value()) class=FIELD_INPUT bind:value=outcome>
                                    <option value="">"Not recorded yet"</option>
                                    {TaskOutcome::ALL
                                        .iter()
                                        .map(|o| view! { <option value=o.slug()>{o.label()}</option> })
                                        .collect::<Vec<_>>()}
                                </select>
                            </div>
                            <div class=format!("{FIELD} sm:col-span-2")>
                                <label for=format!("{}-notes", form_id.get_value()) class=FIELD_LABEL>"Outcome notes"</label>
                                <textarea id=format!("{}-notes", form_id.get_value()) class=FIELD_INPUT rows="2" bind:value=notes></textarea>
                            </div>
                            <div class="flex gap-2 sm:col-span-2">
                                <button type="submit" class=PRIMARY>"Save outcome"</button>
                                <button type="button" class=SECONDARY on:click=reset>"Cancel"</button>
                            </div>
                            <Show when=move || !error.get().is_empty()>
                                <p role="alert" class="text-sm text-rose-300 sm:col-span-2">{move || error.get()}</p>
                            </Show>
                        </form>
                        </Show>
                    }
                })}
        </li>
    }
}
