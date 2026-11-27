use leptos::prelude::*;
use leptos::task::spawn_local;

use crate::components::design::{
    Icon, StatusPill, Tone, BTN, BTN_GREEN, CARD, FIELD, HINT, INPUT, LBL, SERIF_H2,
};
use crate::components::legal_memos::ERRTEXT;
use crate::server_fns::crm::MAX_LONG_TEXT;
use crate::server_fns::err_text;
use crate::server_fns::legal_memos::{
    approve_legal_memo, request_memo_revisions, withdraw_legal_memo,
};

#[component]
pub(super) fn ReviewPanel(
    memo_id: String,
    version: i32,
    on_changed: Callback<()>,
) -> impl IntoView {
    let memo_id = StoredValue::new(memo_id);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let comment = RwSignal::new(String::new());
    let confirming = RwSignal::new(false);

    let act = move |approve: bool| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        error.set(String::new());
        let id = memo_id.get_value();
        let note = comment.get_untracked();
        spawn_local(async move {
            let result = if approve {
                approve_legal_memo(id, version, note).await
            } else {
                request_memo_revisions(id, version, note).await
            };
            match result {
                Ok(()) => {
                    let _ = on_changed.try_run(());
                }
                Err(e) => {
                    let _ = error.try_set(err_text(e));
                    let _ = busy.try_set(false);
                    let _ = confirming.try_set(false);
                }
            }
        });
    };

    view! {
        <section class=format!("{CARD} border-2! border-info-600! px-5 pt-[18px] pb-5") aria-labelledby="review-h">
            <div class="-mx-5 flex flex-wrap items-center justify-between gap-2 border-b border-slate-800 px-5 pb-3">
                <h2 id="review-h" class=SERIF_H2>"Attorney review"</h2>
                <StatusPill label="Assigned reviewer" tone=Tone::Dark icon_kind=Icon::Lock />
            </div>
            <div class=format!("{FIELD} mt-3")>
                <label class=LBL for="review-comment">"Comment for the author"</label>
                <textarea
                    id="review-comment"
                    class=INPUT
                    rows="3"
                    maxlength=MAX_LONG_TEXT
                    prop:value=move || comment.get()
                    on:input=move |ev| comment.set(event_target_value(&ev))
                ></textarea>
                <span class=HINT>"Required when requesting revisions. Kept with the approval if you approve."</span>
            </div>
            {move || if confirming.get() {
                view! {
                    <p class="mt-3 text-[14.5px] font-semibold">{format!("Approve version {version}? It can't be changed after this.")}</p>
                    <div class="mt-2 grid grid-cols-2 gap-2">
                        <button type="button" class=BTN prop:disabled=move || busy.get() on:click=move |_| confirming.set(false)>
                            "Cancel"
                        </button>
                        <button type="button" class=BTN_GREEN prop:disabled=move || busy.get() on:click=move |_| act(true)>
                            "Yes, approve"
                        </button>
                    </div>
                }
                .into_any()
            } else {
                view! {
                    <div class="mt-3 grid grid-cols-2 gap-2">
                        <button type="button" class=BTN prop:disabled=move || busy.get() || comment.with(|c| c.trim().is_empty()) on:click=move |_| act(false)>
                            "Request revisions"
                        </button>
                        <button type="button" class=BTN_GREEN prop:disabled=move || busy.get() on:click=move |_| confirming.set(true)>
                            "Approve"
                        </button>
                    </div>
                }
                .into_any()
            }}
            <p class=format!("{HINT} mt-2")>"Approving locks the memo. Requesting revisions sends it back to the author with your comment. The reviewer should be an attorney."</p>
            <Show when=move || !error.get().is_empty()>
                <p class=format!("{ERRTEXT} mt-2") role="alert">{move || error.get()}</p>
            </Show>
        </section>
    }
}

#[component]
pub(super) fn WithdrawButton(memo_id: String, on_changed: Callback<()>) -> impl IntoView {
    let memo_id = StoredValue::new(memo_id);
    let confirming = RwSignal::new(false);
    let busy = RwSignal::new(false);
    let error = RwSignal::new(String::new());
    let withdraw = move |_| {
        if busy.get_untracked() {
            return;
        }
        busy.set(true);
        error.set(String::new());
        let id = memo_id.get_value();
        spawn_local(async move {
            match withdraw_legal_memo(id).await {
                Ok(()) => {
                    let _ = on_changed.try_run(());
                }
                Err(e) => {
                    let _ = error.try_set(err_text(e));
                    let _ = busy.try_set(false);
                    let _ = confirming.try_set(false);
                }
            }
        });
    };
    view! {
        {move || if confirming.get() {
            view! {
                <span class="text-[13.5px] font-semibold">"Take it back to edit? The attorney can't act on it until you send it again."</span>
                <button type="button" class=BTN prop:disabled=move || busy.get() on:click=withdraw>"Yes, withdraw"</button>
                <button type="button" class=BTN prop:disabled=move || busy.get() on:click=move |_| confirming.set(false)>"Cancel"</button>
            }
            .into_any()
        } else {
            view! { <button type="button" class=BTN on:click=move |_| confirming.set(true)>"Withdraw from review"</button> }.into_any()
        }}
        <Show when=move || !error.get().is_empty()>
            <span class=ERRTEXT role="alert">{move || error.get()}</span>
        </Show>
    }
}
