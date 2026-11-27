use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use leptos_router::hooks::use_navigate;

use crate::components::case_overview::month_day;
use crate::components::design::{
    icon, EdgeTable, Icon, StatusPill, Tone, VolunteersOnlyBadge, BTN_PRIMARY, CARD, CARD_HEAD,
    HINT, MUTED, SERIF_PAGE_H2, TD,
};
use crate::server_fns::err_text;
use crate::server_fns::legal_memos::{
    create_legal_memo, list_legal_memos, LegalMemoSummary, MemoStatus,
};

pub const ERRTEXT: &str = "text-[13.5px] font-semibold text-danger-700";

pub fn memo_status_tone(status: MemoStatus) -> Tone {
    match status {
        MemoStatus::Draft => Tone::Sand,
        MemoStatus::InReview => Tone::Yellow,
        MemoStatus::RevisionsRequested => Tone::Red,
        MemoStatus::Approved => Tone::Green,
    }
}

pub fn memo_href(case_id: &str, memo_id: &str) -> String {
    format!("/cases/{case_id}/legal-memos/{memo_id}")
}

pub fn matter_label(case_name: &str, docket: &str) -> String {
    if docket.trim().is_empty() {
        case_name.to_string()
    } else {
        format!("{case_name} \u{b7} {}", docket.trim())
    }
}

#[component]
pub fn LegalTab(case_id: String, can_create: bool) -> impl IntoView {
    let case_sv = StoredValue::new(case_id);
    let memos = RwSignal::new(Vec::<LegalMemoSummary>::new());
    let loading = RwSignal::new(true);
    let load_error = RwSignal::new(String::new());
    let creating = RwSignal::new(false);
    let create_error = RwSignal::new(String::new());
    let navigate = StoredValue::new_local(use_navigate());

    let load_case = case_sv.get_value();
    spawn_local(async move {
        let found = list_legal_memos(load_case).await;
        let _ = loading.try_set(false);
        match found {
            Ok(list) => {
                let _ = memos.try_set(list);
            }
            Err(e) => {
                let _ = load_error.try_set(err_text(e));
            }
        }
    });

    let create = move |_| {
        if creating.get_untracked() {
            return;
        }
        creating.set(true);
        create_error.set(String::new());
        let case_id = case_sv.get_value();
        spawn_local(async move {
            match create_legal_memo(case_id.clone()).await {
                Ok(id) => {
                    let href = memo_href(&case_id, &id);
                    navigate.try_with_value(|go| go(&href, Default::default()));
                }
                Err(e) => {
                    let _ = creating.try_set(false);
                    let _ = create_error.try_set(err_text(e));
                }
            }
        });
    };

    let rows = move || {
        if loading.get() {
            return view! { <p class=format!("{HINT} mt-3")>"Loading\u{2026}"</p> }.into_any();
        }
        if !load_error.get().is_empty() {
            return view! {
                <p class=format!("{ERRTEXT} mt-3") role="alert">
                    {move || format!("Could not load legal memos: {}", load_error.get())}
                </p>
            }
            .into_any();
        }
        let list = memos.get();
        if list.is_empty() {
            return view! { <p class=format!("{MUTED} mt-3 text-[13.5px]")>"No legal memos on this case yet."</p> }.into_any();
        }
        view! {
            <div class="mt-2.5">
                <EdgeTable headers=vec![("Title", None), ("Author", None), ("Updated", None), ("Status", None)]>
                    {list
                        .into_iter()
                        .map(|memo| {
                            let href = memo_href(&case_sv.get_value(), &memo.id);
                            let approved = memo.status == MemoStatus::Approved;
                            let label = if approved {
                                format!("{} \u{b7} V{}", memo.status.label(), memo.version)
                            } else {
                                memo.status.label().to_string()
                            };
                            let tone = memo_status_tone(memo.status);
                            view! {
                                <tr>
                                    <td class=format!("{TD} min-w-[220px]")>
                                        <A href=href attr:class="font-bold text-slate-200 underline underline-offset-2 hover:text-primary-300">{memo.title}</A>
                                    </td>
                                    <td class=TD>{memo.author_name}</td>
                                    <td class=format!("{TD} whitespace-nowrap")>{month_day(&memo.updated_on)}</td>
                                    <td class=TD>
                                        {if approved {
                                            view! { <StatusPill label=label tone=tone icon_kind=Icon::Check /> }.into_any()
                                        } else {
                                            view! { <StatusPill label=label tone=tone /> }.into_any()
                                        }}
                                    </td>
                                </tr>
                            }
                        })
                        .collect_view()}
                </EdgeTable>
            </div>
        }
        .into_any()
    };

    view! {
        <div class="space-y-[18px]">
            <h2 class=SERIF_PAGE_H2>"Legal"</h2>
            <section class=format!("{CARD} px-[22px] pt-5 pb-[18px]") aria-labelledby="legal-memos-heading">
                <div class=CARD_HEAD>
                    <div class="flex flex-wrap items-center gap-2.5">
                        <h3 id="legal-memos-heading" class="font-serif text-[20px] font-semibold">"Legal memos"</h3>
                        <VolunteersOnlyBadge />
                    </div>
                    {can_create.then(|| view! {
                        <button type="button" class=BTN_PRIMARY on:click=create prop:disabled=move || creating.get()>
                            {icon(Icon::Plus, "h-4 w-4 flex-none")}
                            {move || if creating.get() { "Creating\u{2026}" } else { "New legal memo" }}
                        </button>
                    })}
                </div>
                <p class=format!("{MUTED} mt-2.5 text-[13.5px]")>
                    "Clients never see memos. A volunteer writes the memo and sends it to a reviewer on the case, who should be an attorney, to approve."
                </p>
                <Show when=move || !create_error.get().is_empty()>
                    <p class=format!("{ERRTEXT} mt-2") role="alert">{move || create_error.get()}</p>
                </Show>
                {rows}
            </section>
        </div>
    }
}
