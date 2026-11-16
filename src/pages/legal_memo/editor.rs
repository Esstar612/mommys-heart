use leptos::prelude::*;
use leptos::task::spawn_local;

use super::{
    back_href, back_label, clock, matter, read_tables, section_card, status_pill, when, SECTIONS,
};
use crate::components::autosave::Autosaver;
use crate::components::design::{
    icon, DarkBanner, Icon, PageHeader, Segmented, VolunteersOnlyBadge, BTN, BTN_SM, ERRMSG, FIELD,
    HINT, INPUT, INPUT_READONLY, KICKER, LBL, MUTED, SAVED,
};
use crate::components::legal_memos::ERRTEXT;
use crate::server_fns::err_text;
use crate::server_fns::legal_memos::{
    memo_attorney_choices, save_legal_memo_draft, validate_draft, LegalMemo, LegalMemoContent,
    MemoAttorney, MemoFact, MemoStatus, MemoVersionEntry, VersionEvent, ANALYSIS_HEADINGS,
    MAX_LONG, MAX_ROWS, MAX_SHORT,
};
use crate::state::now_stamp;

pub(super) fn add_row(label: &'static str, full: Signal<bool>, on_click: Callback<()>) -> AnyView {
    view! {
        <div class="mt-3 flex flex-wrap items-center gap-2">
            <button type="button" class=BTN_SM prop:disabled=move || full.get() on:click=move |_| on_click.run(())>
                {icon(Icon::Plus, "h-4 w-4 flex-none")}
                {label}
            </button>
            <Show when=move || full.get()>
                <span class=HINT>{format!("{MAX_ROWS} rows is the limit.")}</span>
            </Show>
        </div>
    }
    .into_any()
}

#[component]
pub(super) fn MemoEditor(
    memo: LegalMemo,
    versions: RwSignal<Vec<MemoVersionEntry>>,
    on_changed: Callback<()>,
) -> impl IntoView {
    let memo_id = StoredValue::new(memo.id.clone());
    let title = RwSignal::new(memo.title.clone());
    let attorney = RwSignal::new(memo.attorney_id.clone());
    let mut initial = memo.content.clone();
    if initial.memo_date.is_empty() {
        initial.memo_date = now_stamp()[..10].to_string();
    }
    if initial.questions.is_empty() {
        initial.questions.push(String::new());
    }
    if initial.facts.is_empty() {
        initial.facts.push(MemoFact::default());
    }
    let content = RwSignal::new(initial);
    let attorneys = RwSignal::new(Vec::<MemoAttorney>::new());
    let attorneys_error = RwSignal::new(String::new());
    let stale_attorney = RwSignal::new(String::new());

    let autosave = Autosaver::new(RwSignal::new(false), move || {
        let (Some(id), Some(t), Some(a), Some(c)) = (
            memo_id.try_get_value(),
            title.try_get_untracked(),
            attorney.try_get_untracked(),
            content.try_get_untracked(),
        ) else {
            return None;
        };
        Some(Box::pin(async move {
            validate_draft(&t, &c)?;
            save_legal_memo_draft(id, t, a, c).await.map_err(err_text)
        }))
    });
    let touch = Callback::new(move |_: ()| autosave.changed());
    let draft_error = Memo::new(move |_| {
        let t = title.get();
        content.with(|c| validate_draft(&t, c).err())
    });

    let choices_case = memo.case_id.clone();
    let saved_attorney_name = memo.attorney_name.clone();
    spawn_local(async move {
        match memo_attorney_choices(choices_case).await {
            Ok(list) => {
                let current = attorney.try_get_untracked().unwrap_or_default();
                if !current.is_empty() && !list.iter().any(|a| a.id == current) {
                    let _ = attorney.try_set(String::new());
                    let _ = stale_attorney.try_set(saved_attorney_name);
                    let _ = touch.try_run(());
                }
                let _ = attorneys.try_set(list);
            }
            Err(e) => {
                let _ = attorneys_error.try_set(err_text(e));
            }
        }
    });

    let save_now = move |_| autosave.save_now();

    let saved_class = move || {
        autosave.status.with(|status| {
            if status.starts_with("Saved") {
                SAVED.to_string()
            } else if status.starts_with("Not saved") {
                ERRTEXT.to_string()
            } else {
                format!("{MUTED} text-[13.5px]")
            }
        })
    };
    let saved_text = move || {
        let status = autosave.status.get();
        let saved = status.starts_with("Saved");
        let label = match status.strip_prefix("Saved ") {
            Some(t) => format!("Saved {}", clock(t).unwrap_or_else(|| t.to_string())),
            None => status,
        };
        view! { {saved.then(|| icon(Icon::Check, "h-4 w-4 flex-none"))} {label} }
    };
    let actions = move |live: bool| {
        view! {
            <span class=saved_class role=live.then_some("status")>{saved_text}</span>
            <button type="button" class=BTN on:click=save_now prop:disabled=move || autosave.saving.get()>
                "Save draft"
            </button>
        }
    };
    let revision_note = move || {
        versions.with(|list| {
            list.iter()
                .find(|e| e.event == VersionEvent::RevisionsRequested)
                .cloned()
        })
    };
    let is_revision = memo.status == MemoStatus::RevisionsRequested;

    let nav = move || {
        view! {
            <nav aria-label="Memo sections" class="sticky top-4 hidden rounded-[20px] border border-slate-800 bg-white p-3 lg:block">
                <p class=format!("{KICKER} px-2.5 pt-1 pb-2")>"Sections"</p>
                <ol>
                    {SECTIONS
                        .iter()
                        .map(|s| {
                            let label = match s.number {
                                Some(n) => format!("{n} {}", s.title),
                                None => s.title.to_string(),
                            };
                            view! {
                                <li>
                                    <a href=format!("#{}", s.anchor) class="flex items-center gap-2 rounded-[10px] px-2.5 py-[7px] text-[14px] text-slate-200 no-underline hover:bg-sand-200">
                                        <span class="flex-1">{label}</span>
                                        {s.required.then(|| view! { <span class=HINT aria-hidden="true">"req."</span><span class="sr-only">"Required"</span> })}
                                    </a>
                                </li>
                            }
                        })
                        .collect_view()}
                </ol>
            </nav>
        }
    };

    let card =
        |anchor: &'static str, body: AnyView| section_card(anchor, Signal::from(false), body);

    let header = {
        let author = memo.author_name.clone();
        let matter = matter(&memo);
        let status = memo.status;
        view! {
            <div class="grid gap-x-4 gap-y-3 sm:grid-cols-2 lg:grid-cols-3">
                <div class=FIELD>
                    <label class=LBL for="memo-to">"To"</label>
                    <select
                        id="memo-to"
                        class=INPUT
                        prop:value=move || attorney.get()
                        on:change=move |ev| {
                            attorney.set(event_target_value(&ev));
                            stale_attorney.set(String::new());
                            touch.run(());
                        }
                    >
                        <option value="" selected=move || attorney.get().is_empty()>"Choose an attorney"</option>
                        {move || attorneys
                            .get()
                            .into_iter()
                            .map(|a| {
                                let id = a.id.clone();
                                view! { <option value=a.id selected=move || attorney.get() == id>{a.name}</option> }
                            })
                            .collect_view()}
                    </select>
                    <Show when=move || !stale_attorney.get().is_empty()>
                        <span class=ERRMSG>{move || format!("{} is no longer on this case. Choose another attorney.", stale_attorney.get())}</span>
                    </Show>
                    <Show when=move || !attorneys_error.get().is_empty()>
                        <span class=ERRMSG>{move || format!("Could not load attorneys: {}", attorneys_error.get())}</span>
                    </Show>
                </div>
                <div class=FIELD>
                    <label class=LBL for="memo-from">"From"</label>
                    <input id="memo-from" class=INPUT_READONLY readonly=true value=author />
                    <span class=HINT>"Filled in automatically"</span>
                </div>
                <div class=FIELD>
                    <label class=LBL for="memo-date">"Date"</label>
                    <input
                        id="memo-date"
                        type="date"
                        class=INPUT
                        prop:value=move || content.with(|c| c.memo_date.clone())
                        on:input=move |ev| { let v = event_target_value(&ev); content.update(|c| c.memo_date = v); touch.run(()); }
                    />
                </div>
                <div class=FIELD>
                    <label class=LBL for="memo-matter">"Matter"</label>
                    <input id="memo-matter" class=INPUT_READONLY readonly=true value=matter />
                    <span class=HINT>"Filled in from the case"</span>
                </div>
                <div class=FIELD>
                    <label class=LBL for="memo-status">"Status"</label>
                    <input id="memo-status" class=INPUT_READONLY readonly=true value=status.label() />
                    <span class=HINT>"Changes when you send it or the attorney acts"</span>
                </div>
            </div>
        }
        .into_any()
    };

    let safety = view! {
        <Segmented
            name="memo-safety"
            legend="Safety concern flagged?"
            options=vec![("yes".to_string(), "Yes".to_string()), ("no".to_string(), "No".to_string())]
            value=Signal::derive(move || content.with(|c| match c.safety_flagged {
                Some(true) => "yes".to_string(),
                Some(false) => "no".to_string(),
                None => String::new(),
            }))
            on_change=Callback::new(move |v: String| {
                content.update(|c| {
                    c.safety_flagged = Some(v == "yes");
                    c.safety_answered_on = now_stamp();
                });
                touch.run(());
            })
        />
        <p class=format!("{HINT} mt-2")>
            "If Yes, you'll be asked for the date it was escalated and who it was escalated to. A safety concern goes to a supervisor right away, not only in this memo."
        </p>
        <Show when=move || content.with(|c| c.safety_flagged == Some(true))>
            <div class="mt-3 grid gap-x-4 gap-y-3 sm:grid-cols-2">
                <div class=FIELD>
                    <label class=LBL for="memo-escalated-on">"Date escalated"</label>
                    <input id="memo-escalated-on" type="date" class=INPUT
                        prop:value=move || content.with(|c| c.safety_escalated_on.clone())
                        on:input=move |ev| { let v = event_target_value(&ev); content.update(|c| c.safety_escalated_on = v); touch.run(()); } />
                </div>
                <div class=FIELD>
                    <label class=LBL for="memo-escalated-to">"Escalated to"</label>
                    <input id="memo-escalated-to" class=INPUT maxlength=MAX_SHORT
                        prop:value=move || content.with(|c| c.safety_escalated_to.clone())
                        on:input=move |ev| { let v = event_target_value(&ev); content.update(|c| c.safety_escalated_to = v); touch.run(()); } />
                </div>
            </div>
        </Show>
    }
    .into_any();

    let list_rows = move |get: fn(&LegalMemoContent) -> &Vec<String>,
                          get_mut: fn(&mut LegalMemoContent) -> &mut Vec<String>,
                          noun: &'static str| {
        view! {
            <ul class="flex flex-col gap-2">
                <For each=move || 0..content.with(|c| get(c).len()) key=|i| *i let:i>
                    <li class="flex items-center gap-2">
                        <span class=format!("{MUTED} w-5 flex-none text-[13.5px]")>{format!("{}.", i + 1)}</span>
                        <input
                            class=INPUT
                            maxlength=MAX_LONG
                            aria-label=format!("{noun} {}", i + 1)
                            prop:value=move || content.with(|c| get(c).get(i).cloned().unwrap_or_default())
                            on:input=move |ev| {
                                let v = event_target_value(&ev);
                                content.update(|c| if let Some(row) = get_mut(c).get_mut(i) { *row = v; });
                                touch.run(());
                            }
                        />
                        <button
                            type="button"
                            class=format!("{BTN_SM} flex-none px-2.5")
                            aria-label=format!("Remove {} {}", noun.to_lowercase(), i + 1)
                            on:click=move |_| {
                                content.update(|c| { let rows = get_mut(c); if i < rows.len() { rows.remove(i); } });
                                touch.run(());
                            }
                        >
                            {icon(Icon::Cross, "h-4 w-4")}
                        </button>
                    </li>
                </For>
            </ul>
            {add_row(
                "Add another",
                Signal::derive(move || content.with(|c| get(c).len() >= MAX_ROWS)),
                Callback::new(move |_| {
                    content.update(|c| if get(c).len() < MAX_ROWS { get_mut(c).push(String::new()); });
                    touch.run(());
                }),
            )}
        }
        .into_any()
    };

    let brief = view! {
        <textarea
            class=INPUT
            rows="3"
            maxlength=MAX_LONG
            aria-label="Brief answer"
            prop:value=move || content.with(|c| c.brief_answer.clone())
            on:input=move |ev| { let v = event_target_value(&ev); content.update(|c| c.brief_answer = v); touch.run(()); }
        ></textarea>
    }
    .into_any();

    let analysis = view! {
        <div class="flex flex-col gap-3">
            {ANALYSIS_HEADINGS
                .iter()
                .enumerate()
                .map(|(i, heading)| view! {
                    <div class=FIELD>
                        <label class=LBL for=format!("memo-analysis-{i}")>{*heading}</label>
                        <textarea
                            id=format!("memo-analysis-{i}")
                            class=INPUT
                            rows="2"
                            maxlength=MAX_LONG
                            prop:value=move || content.with(|c| c.analysis[i].clone())
                            on:input=move |ev| { let v = event_target_value(&ev); content.update(|c| c.analysis[i] = v); touch.run(()); }
                        ></textarea>
                    </div>
                })
                .collect_view()}
        </div>
    }
    .into_any();
    let title_input = view! {
        <label for="memo-title" class="sr-only">"Memo title"</label>
        <h1 class="font-serif text-[30px] font-semibold leading-tight text-slate-200">
            "Legal memo: "
            <input
                id="memo-title"
                maxlength=MAX_SHORT
                class="min-w-[10ch] max-w-full rounded-md border-0 border-b-2 border-dashed border-sand-500 bg-transparent p-0 font-serif text-[30px] font-semibold text-slate-200 [field-sizing:content] focus:border-primary-600 focus:outline-none"
                prop:value=move || title.get()
                on:input=move |ev| { title.set(event_target_value(&ev)); touch.run(()); }
            />
        </h1>
    };
    let pills = view! { {status_pill(&memo)} <VolunteersOnlyBadge /> };

    view! {
        <PageHeader back_href=back_href(&memo.case_id) back_label=back_label(&memo) actions=Box::new(move || actions(true).into_any())>
            {title_input}
            <div class="mt-2 flex flex-wrap gap-2">{pills}</div>
        </PageHeader>
        <DarkBanner>"Draft for attorney review. Not for client distribution."</DarkBanner>
        <Show when=move || autosave.failed.with(Option::is_some)>
            <div class="flex flex-wrap items-center gap-3 rounded-2xl border-2 border-danger-600 bg-danger-50 px-5 py-3 text-danger-700" role="alert">
                <p class="font-semibold">{move || format!("Not saved: {}", autosave.failed.get().unwrap_or_default())}</p>
                <Show when=move || draft_error.with(Option::is_none)>
                    <button type="button" class=BTN_SM on:click=move |_| { let _ = on_changed.try_run(()); }>"Reload"</button>
                </Show>
            </div>
        </Show>
        {is_revision.then(|| view! {
            {move || revision_note().map(|entry| view! {
                <div class="rounded-2xl border border-caution-300 bg-caution-50 px-5 py-3.5 text-caution-800" role="note">
                    <p class="font-semibold">{format!("Revisions requested by {}, {}", entry.actor_name, when(&entry.at))}</p>
                    <p class="mt-1 whitespace-pre-line">{entry.comment}</p>
                </div>
            })}
        })}
        <div class="grid items-start gap-6 lg:grid-cols-[250px_minmax(0,1fr)]">
            {nav}
            <div class="flex min-w-0 flex-col gap-4">
                {card("mh", header)}
                {card("ms", safety)}
                {card("s1", list_rows(|c| &c.questions, |c| &mut c.questions, "Question"))}
                {card("s2", brief)}
                {card("s3", view! { {move || content.with(read_tables::facts)} }.into_any())}
                {card("s4", view! { {move || content.with(read_tables::procedure)} }.into_any())}
                {card("s5", view! { {move || content.with(read_tables::evidence)} }.into_any())}
                {card("s6", view! { {move || content.with(read_tables::authorities)} }.into_any())}
                {card("s7", analysis)}
                {card("s8", list_rows(|c| &c.outstanding, |c| &mut c.outstanding, "Outstanding question"))}
                {card("s9", view! { {move || content.with(read_tables::next_steps)} }.into_any())}
                <div class="flex flex-wrap items-center justify-end gap-2">{actions(false)}</div>
            </div>
        </div>
    }
}
