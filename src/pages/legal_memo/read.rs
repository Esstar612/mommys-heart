use leptos::prelude::*;

use super::{date_long, matter, section_card};
use crate::components::design::{HINT, MUTED};
use crate::server_fns::legal_memos::{LegalMemo, LegalMemoContent};

fn none() -> AnyView {
    view! { <p class=format!("{MUTED} text-[14.5px]")>"None recorded."</p> }.into_any()
}

fn filled<'a>(items: impl Iterator<Item = &'a String>) -> Vec<String> {
    items.filter(|t| !t.trim().is_empty()).cloned().collect()
}

fn numbered(items: Vec<String>) -> AnyView {
    if items.is_empty() {
        return none();
    }
    view! {
        <ol class="flex list-decimal flex-col gap-1.5 pl-[22px]">
            {items.into_iter().map(|t| view! { <li class="whitespace-pre-line">{t}</li> }).collect_view()}
        </ol>
    }
    .into_any()
}

fn bulleted(items: Vec<String>) -> AnyView {
    if items.is_empty() {
        return none();
    }
    view! {
        <ul class="flex list-disc flex-col gap-1.5 pl-[22px]">
            {items.into_iter().map(|t| view! { <li class="whitespace-pre-line">{t}</li> }).collect_view()}
        </ul>
    }
    .into_any()
}

pub(super) fn header_card(memo: &LegalMemo) -> AnyView {
    let rows = vec![
        (
            "To",
            if memo.attorney_name.trim().is_empty() {
                "Not chosen yet".to_string()
            } else {
                memo.attorney_name.clone()
            },
        ),
        ("From", memo.author_name.clone()),
        (
            "Date",
            if memo.content.memo_date.is_empty() {
                "Not set".to_string()
            } else {
                date_long(&memo.content.memo_date)
            },
        ),
        ("Matter", matter(memo)),
        ("Status", memo.status.label().to_string()),
    ];
    section_card(
        "mh",
        view! {
            <dl class="grid gap-x-5 gap-y-3 text-[15px] sm:grid-cols-2 lg:grid-cols-3">
                {rows
                    .into_iter()
                    .map(|(label, value)| view! {
                        <div><dt class=HINT>{label}</dt><dd class="font-semibold">{value}</dd></div>
                    })
                    .collect_view()}
            </dl>
        }
        .into_any(),
    )
}

pub(super) fn content_cards(c: &LegalMemoContent) -> AnyView {
    let brief = if c.brief_answer.trim().is_empty() {
        none()
    } else {
        view! { <p class="whitespace-pre-line text-[15.5px]">{c.brief_answer.clone()}</p> }
            .into_any()
    };
    let parts: Vec<(String, String)> = c
        .analysis
        .iter()
        .filter(|a| !a.heading.trim().is_empty() || !a.body.trim().is_empty())
        .map(|a| (a.heading.clone(), a.body.clone()))
        .collect();
    let analysis = if parts.is_empty() {
        none()
    } else {
        view! {
            <div class="flex flex-col gap-3">
                {parts
                    .into_iter()
                    .map(|(heading, body)| view! {
                        <div>
                            <p class="font-bold">{heading}</p>
                            <p class="mt-0.5 whitespace-pre-line">{body}</p>
                        </div>
                    })
                    .collect_view()}
            </div>
        }
        .into_any()
    };

    view! {
        {section_card("s1", numbered(filled(c.questions.iter().map(|q| &q.text))))}
        {section_card("s2", brief)}
        {section_card("s3", bulleted(filled(c.facts.iter().map(|f| &f.text))))}
        {section_card("s4", analysis)}
        {section_card("s5", bulleted(filled(c.outstanding.iter().map(|q| &q.text))))}
        {section_card("s6", numbered(filled(c.next_steps.iter().map(|n| &n.action))))}
    }
    .into_any()
}
