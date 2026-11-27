mod editor;
mod read;
mod review;

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::hooks::use_params_map;

use crate::components::case_overview::month_day;
use crate::components::case_tabs::CaseTab;
use crate::components::design::{
    DarkBanner, Icon, PageHeader, SectionCard, StatusPill, Tone, VolunteersOnlyBadge, MUTED,
    SERIF_H1,
};
use crate::components::guard::require_login;
use crate::components::layout::Layout;
use crate::components::legal_memos::{matter_label, memo_status_tone};
use crate::components::loading::Loading;
use crate::helpers::format::date_parts;
use crate::server_fns::err_text;
use crate::server_fns::legal_memos::{load_legal_memo, LegalMemo, MemoStatus};
use crate::state::AppState;

use editor::MemoEditor;
use read::{content_cards, header_card};
use review::{ReviewPanel, WithdrawButton};

pub(super) struct Section {
    pub anchor: &'static str,
    pub number: Option<&'static str>,
    pub title: &'static str,
    pub required: bool,
}

pub(super) const SECTIONS: &[Section] = &[
    Section {
        anchor: "mh",
        number: None,
        title: "Header",
        required: true,
    },
    Section {
        anchor: "s1",
        number: Some("1"),
        title: "Questions presented",
        required: true,
    },
    Section {
        anchor: "s2",
        number: Some("2"),
        title: "Brief answer",
        required: true,
    },
    Section {
        anchor: "s3",
        number: Some("3"),
        title: "Relevant facts",
        required: false,
    },
    Section {
        anchor: "s4",
        number: Some("4"),
        title: "Analysis",
        required: false,
    },
    Section {
        anchor: "s5",
        number: Some("5"),
        title: "Outstanding questions",
        required: false,
    },
    Section {
        anchor: "s6",
        number: Some("6"),
        title: "Proposed next steps",
        required: false,
    },
];

pub(super) fn section_card(anchor: &str, body: AnyView) -> AnyView {
    let s = SECTIONS
        .iter()
        .find(|s| s.anchor == anchor)
        .unwrap_or(&SECTIONS[0]);
    match s.number {
        Some(n) => view! {
            <SectionCard title=s.title anchor=s.anchor number=n required=s.required>{body}</SectionCard>
        }
        .into_any(),
        None => view! {
            <SectionCard title=s.title anchor=s.anchor required=s.required>{body}</SectionCard>
        }
        .into_any(),
    }
}

pub(super) fn date_long(stamp: &str) -> String {
    date_parts(stamp)
        .map(|(m, d, y)| format!("{m} {d}, {y}"))
        .unwrap_or_else(|| stamp.to_string())
}

pub(super) fn clock(hhmm: &str) -> Option<String> {
    let (h, min) = hhmm.get(..5)?.split_once(':')?;
    let h = h.parse::<u32>().ok()?;
    let suffix = if h >= 12 { "PM" } else { "AM" };
    let h12 = match h % 12 {
        0 => 12,
        other => other,
    };
    Some(format!("{h12}:{min} {suffix}"))
}

pub(super) fn when(stamp: &str) -> String {
    let Some((m, d, _)) = date_parts(stamp) else {
        return stamp.to_string();
    };
    match stamp.get(11..).and_then(clock) {
        Some(t) => format!("{m} {d}, {t}"),
        None => format!("{m} {d}"),
    }
}

pub(super) fn back_href(case_id: &str) -> String {
    CaseTab::Legal.href(case_id)
}

pub(super) fn back_label(memo: &LegalMemo) -> String {
    format!("{} \u{b7} Legal", memo.case_name)
}

pub(super) fn matter(memo: &LegalMemo) -> String {
    matter_label(&memo.case_name, &memo.docket)
}

pub(super) fn status_pill(memo: &LegalMemo) -> AnyView {
    let label = format!("{} \u{b7} version {}", memo.status.label(), memo.version);
    view! { <StatusPill label=label tone=memo_status_tone(memo.status) /> }.into_any()
}

const ALERT: &str = "rounded-2xl border-2 border-danger-600 bg-danger-50 px-5 py-3 text-[14.5px] font-semibold text-danger-700";

#[component]
pub fn LegalMemoPage() -> impl IntoView {
    let state = expect_context::<AppState>();
    let params = use_params_map();
    let memo = RwSignal::new(None::<LegalMemo>);
    let loading = RwSignal::new(true);
    let load_error = RwSignal::new(String::new());
    let reload = RwSignal::new(0u32);
    let generation = RwSignal::new(0u32);

    Effect::new(move |_| {
        reload.track();
        let (case_id, memo_id) = params.with(|p| {
            (
                p.get("case_id").unwrap_or_default(),
                p.get("memo_id").unwrap_or_default(),
            )
        });
        if !state.is_authenticated() {
            return;
        }
        generation.update(|g| *g += 1);
        let mine = generation.get_untracked();
        spawn_local(async move {
            let result = load_legal_memo(memo_id).await;
            if generation.try_get_untracked() != Some(mine) {
                return;
            }
            let _ = loading.try_set(false);
            match result {
                Ok(found) if found.case_id == case_id => {
                    let _ = load_error.try_set(String::new());
                    let _ = memo.try_set(Some(found));
                }
                Ok(_) => {
                    let _ = load_error.try_set("Legal memo not found.".to_string());
                }
                Err(e) => {
                    let _ = load_error.try_set(err_text(e));
                }
            }
        });
    });

    let on_changed = Callback::new(move |_: ()| reload.update(|r| *r += 1));
    let current = Memo::new(move |_| memo.get());

    require_login(state, move || {
        view! {
            <Layout title="Legal memo".to_string() hide_heading=true>
                <div class="flex flex-col gap-4">
                    <Show when=move || current.with(Option::is_some) && !load_error.get().is_empty()>
                        <p class=ALERT role="alert">
                            {move || format!("Couldn't refresh this memo: {}", load_error.get())}
                        </p>
                    </Show>
                    {move || {
                        if let Some(found) = current.get() {
                            return if found.can_edit {
                                view! { <MemoEditor memo=found on_changed=on_changed /> }.into_any()
                            } else {
                                view! { <MemoView memo=found on_changed=on_changed /> }.into_any()
                            };
                        }
                        if loading.get() {
                            return view! { <Loading label="Loading legal memo\u{2026}" /> }.into_any();
                        }
                        view! { <p class=ALERT role="alert">{move || load_error.get()}</p> }.into_any()
                    }}
                </div>
            </Layout>
        }
        .into_any()
    })
}

#[component]
fn MemoView(memo: LegalMemo, on_changed: Callback<()>) -> impl IntoView {
    let approved = memo.status == MemoStatus::Approved;
    let pills = if approved {
        let line = format!(
            "Approved by {}, {}",
            memo.approved_by_name,
            month_day(&memo.approved_at)
        );
        view! {
            <StatusPill label=line tone=Tone::Green icon_kind=Icon::Check />
            <StatusPill label=format!("Version {}", memo.version) tone=Tone::Sand />
            <VolunteersOnlyBadge />
        }
        .into_any()
    } else {
        view! { {status_pill(&memo)} <VolunteersOnlyBadge /> }.into_any()
    };
    let sent_line = (memo.status == MemoStatus::InReview)
        .then(|| memo.last_sent.as_ref())
        .flatten()
        .map(|e| format!("Sent by {}, {}", e.actor_name, when(&e.at)));
    let banner = if approved {
        "Approved for internal use. Not for client distribution."
    } else {
        "Draft for attorney review. Not for client distribution."
    };
    let title = format!("Legal memo: {}", memo.title);
    let has_aside = memo.can_review;
    let approval_note = approved
        .then(|| memo.last_approval.clone())
        .flatten()
        .filter(|e| !e.comment.trim().is_empty());
    let withdraw = memo.can_withdraw.then(|| (memo.id.clone(), on_changed));

    view! {
        <PageHeader back_href=back_href(&memo.case_id) back_label=back_label(&memo) actions=Box::new(move || view! {
            <span class=format!("{MUTED} text-[13.5px]")>{sent_line}</span>
            {withdraw.map(|(memo_id, on_changed)| view! { <WithdrawButton memo_id=memo_id on_changed=on_changed /> })}
        }.into_any())>
            <h1 class=SERIF_H1>{title}</h1>
            <div class="mt-2 flex flex-wrap gap-2">{pills}</div>
        </PageHeader>
        <DarkBanner>{banner}</DarkBanner>
        {approval_note.map(|e| view! {
            <div class="rounded-2xl border border-ok-200 bg-ok-50 px-5 py-3.5 text-ok-700" role="note">
                <p class="font-semibold">{format!("Comment from {}, {}", e.actor_name, when(&e.at))}</p>
                <p class="mt-1 whitespace-pre-line">{e.comment}</p>
            </div>
        })}
        <div class=format!("grid items-start gap-6 {}", if has_aside { "lg:grid-cols-[minmax(0,1fr)_380px]" } else { "" })>
            <div class="flex min-w-0 flex-col gap-4">
                {header_card(&memo)}
                {content_cards(&memo.content)}
            </div>
            {has_aside.then(|| view! {
                <aside class="flex flex-col gap-3.5 lg:sticky lg:top-4" aria-label="Attorney review">
                    <ReviewPanel memo_id=memo.id.clone() version=memo.version on_changed=on_changed />
                </aside>
            })}
        </div>
    }
}
