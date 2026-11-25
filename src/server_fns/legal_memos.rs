use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::helpers::dates::parse_iso;
use crate::server_fns::crm::{coded_enum, MAX_SHORT_TEXT};

coded_enum!(MemoStatus {
    Draft => ("draft", "Draft"),
    InReview => ("in_review", "Ready for attorney review"),
    RevisionsRequested => ("revisions_requested", "Revisions requested"),
    Approved => ("approved", "Approved"),
});

coded_enum!(VersionEvent {
    Sent => ("sent", "Sent for attorney review"),
    RevisionsRequested => ("revisions_requested", "Revisions requested"),
    Approved => ("approved", "Approved"),
    Withdrawn => ("withdrawn", "Withdrawn from review"),
});

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoLine {
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct MemoFact {
    pub text: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct AnalysisSection {
    pub heading: String,
    pub body: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NextStep {
    pub action: String,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct LegalMemoContent {
    pub memo_date: String,
    pub questions: Vec<MemoLine>,
    pub brief_answer: String,
    pub facts: Vec<MemoFact>,
    pub analysis: Vec<AnalysisSection>,
    pub outstanding: Vec<MemoLine>,
    pub next_steps: Vec<NextStep>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalMemoSummary {
    pub id: String,
    pub title: String,
    pub status: MemoStatus,
    pub version: i32,
    pub author_name: String,
    pub updated_on: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MemoEvent {
    pub actor_name: String,
    pub comment: String,
    pub at: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalMemo {
    pub id: String,
    pub case_id: String,
    pub case_name: String,
    pub docket: String,
    pub title: String,
    pub status: MemoStatus,
    pub version: i32,
    pub attorney_id: String,
    pub attorney_name: String,
    pub author_id: String,
    pub author_name: String,
    pub content: LegalMemoContent,
    pub approved_by_name: String,
    pub approved_at: String,
    pub revision: String,
    pub last_sent: Option<MemoEvent>,
    pub last_revisions_request: Option<MemoEvent>,
    pub last_approval: Option<MemoEvent>,
    pub can_edit: bool,
    pub can_review: bool,
    pub can_withdraw: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct MemoAttorney {
    pub id: String,
    pub name: String,
}

pub const STALE_MEMO_ERROR: &str =
    "This memo was changed in another window. Reload to get the latest version.";

pub const DOCKET_KEYS: [&str; 2] = ["Index or docket number", "Docket number 1"];

pub const MAX_MEMO_TEXT: usize = 10_000;
pub const MAX_ROWS: usize = 50;

pub fn validate_draft(title: &str, content: &LegalMemoContent) -> Result<(), String> {
    if title.trim().is_empty() {
        return Err("Give the memo a title.".to_string());
    }
    let short = |value: &str, label: &str| check_len(value, label, MAX_SHORT_TEXT);
    let long = |value: &str, label: &str| check_len(value, label, MAX_MEMO_TEXT);
    short(title, "title")?;
    if !content.memo_date.is_empty() && parse_iso(&content.memo_date).is_none() {
        return Err("The date must be a real date.".to_string());
    }
    long(&content.brief_answer, "brief answer")?;
    rows(content.questions.len(), "questions")?;
    rows(content.facts.len(), "facts")?;
    rows(content.analysis.len(), "analysis")?;
    rows(content.outstanding.len(), "outstanding questions")?;
    rows(content.next_steps.len(), "next steps")?;
    for q in &content.questions {
        long(&q.text, "question")?;
    }
    for f in &content.facts {
        long(&f.text, "fact")?;
    }
    for a in &content.analysis {
        short(&a.heading, "analysis heading")?;
        long(&a.body, "analysis")?;
    }
    for q in &content.outstanding {
        long(&q.text, "outstanding question")?;
    }
    for n in &content.next_steps {
        long(&n.action, "next step")?;
    }
    Ok(())
}

pub fn problems_before_review(attorney_id: &str, content: &LegalMemoContent) -> Vec<&'static str> {
    let mut out = Vec::new();
    if attorney_id.trim().is_empty() {
        out.push("Choose the reviewer this memo goes to.");
    }
    if !content.questions.iter().any(|q| !q.text.trim().is_empty()) {
        out.push("Add at least one question presented.");
    }
    if content.brief_answer.trim().is_empty() {
        out.push("Add a brief answer.");
    }
    out
}

fn check_len(value: &str, label: &str, max: usize) -> Result<(), String> {
    if value.chars().count() > max {
        Err(format!("The {label} must be {max} characters or fewer."))
    } else {
        Ok(())
    }
}

fn rows(count: usize, label: &str) -> Result<(), String> {
    if count > MAX_ROWS {
        Err(format!("The {label} can have at most {MAX_ROWS} rows."))
    } else {
        Ok(())
    }
}

#[server(prefix = "/api")]
pub async fn list_legal_memos(case_id: String) -> Result<Vec<LegalMemoSummary>, ServerFnError> {
    use crate::server::db::legal_memos;
    use crate::server::permissions::{require_cap, require_user};
    use crate::server_fns::capabilities::CaseCapability;

    let user = require_user().await?;
    crate::server_fns::crm::require_staff(&user)?;
    require_cap(&user, &case_id, CaseCapability::ViewEvidence).await?;
    legal_memos::list(&case_id).await.map_err(memo_error)
}

#[server(prefix = "/api")]
pub async fn load_legal_memo(memo_id: String) -> Result<LegalMemo, ServerFnError> {
    use crate::server::db::legal_memos;
    use crate::server::permissions::require_user;

    let user = require_user().await?;
    let case_id = authorize_memo_read(&user, &memo_id).await?.case_id;
    let mut memo = legal_memos::get(&memo_id)
        .await
        .map_err(memo_error)?
        .ok_or_else(|| ServerFnError::new("Legal memo not found."))?;
    let writable = case_accepts_changes(&case_id).await?;
    memo.can_edit = writable
        && memo.author_id == user.id
        && matches!(
            memo.status,
            MemoStatus::Draft | MemoStatus::RevisionsRequested
        )
        && can_add_notes(&user, &case_id).await;
    memo.can_withdraw = writable
        && memo.author_id == user.id
        && memo.status == MemoStatus::InReview
        && can_add_notes(&user, &case_id).await;
    memo.can_review = writable
        && memo.status == MemoStatus::InReview
        && memo.author_id != user.id
        && memo.attorney_id == user.id;
    Ok(memo)
}

#[server(prefix = "/api")]
pub async fn memo_attorney_choices(case_id: String) -> Result<Vec<MemoAttorney>, ServerFnError> {
    use crate::server::db::legal_memos;
    use crate::server::permissions::{require_cap, require_user};
    use crate::server_fns::capabilities::CaseCapability;

    let user = require_user().await?;
    crate::server_fns::crm::require_staff(&user)?;
    require_cap(&user, &case_id, CaseCapability::ViewEvidence).await?;
    Ok(legal_memos::attorney_choices(&case_id)
        .await
        .map_err(memo_error)?
        .into_iter()
        .filter(|a| a.id != user.id)
        .collect())
}

#[server(prefix = "/api")]
pub async fn create_legal_memo(case_id: String) -> Result<String, ServerFnError> {
    use crate::server::db::legal_memos;
    use crate::server::permissions::{require_cap, require_user};
    use crate::server_fns::capabilities::CaseCapability;

    let user = require_user().await?;
    crate::server_fns::crm::require_staff(&user)?;
    require_cap(&user, &case_id, CaseCapability::ViewEvidence).await?;
    require_cap(&user, &case_id, CaseCapability::AddNotes).await?;
    if !case_accepts_changes(&case_id).await? {
        return Err(ServerFnError::new("This case can no longer be changed."));
    }
    legal_memos::create(&case_id, "Untitled memo", &user.id, &user.full_name())
        .await
        .map_err(memo_error)
}

#[server(prefix = "/api")]
pub async fn save_legal_memo_draft(
    memo_id: String,
    title: String,
    attorney_id: String,
    content: LegalMemoContent,
    revision: String,
) -> Result<String, ServerFnError> {
    use crate::server::db::legal_memos;
    use crate::server::permissions::require_user;

    let user = require_user().await?;
    let case_id = authorize_memo_author(&user, &memo_id).await?;
    let (title, attorney) = clean_header(&case_id, &title, &attorney_id, &user.id).await?;
    validate_draft(&title, &content).map_err(ServerFnError::new)?;
    legal_memos::save_draft(
        &memo_id,
        &user.id,
        &revision,
        &title,
        attorney.as_ref(),
        &content,
    )
    .await
    .map_err(memo_error)
}

#[server(prefix = "/api")]
pub async fn send_legal_memo_for_review(
    memo_id: String,
    title: String,
    attorney_id: String,
    content: LegalMemoContent,
    revision: String,
) -> Result<(), ServerFnError> {
    use crate::server::db::legal_memos;
    use crate::server::permissions::require_user;

    let user = require_user().await?;
    let case_id = authorize_memo_author(&user, &memo_id).await?;
    let (title, attorney) = clean_header(&case_id, &title, &attorney_id, &user.id).await?;
    validate_draft(&title, &content).map_err(ServerFnError::new)?;
    let problems = problems_before_review(&attorney_id, &content);
    if !problems.is_empty() {
        return Err(ServerFnError::new(problems.join("\n")));
    }
    let attorney =
        attorney.ok_or_else(|| ServerFnError::new("Choose the reviewer this memo goes to."))?;
    legal_memos::send_for_review(
        &memo_id,
        &case_id,
        &user.id,
        &user.full_name(),
        &revision,
        &title,
        &attorney,
        &content,
    )
    .await
    .map_err(memo_error)
}

#[server(prefix = "/api")]
pub async fn request_memo_revisions(
    memo_id: String,
    version: i32,
    comment: String,
) -> Result<(), ServerFnError> {
    use crate::server::db::legal_memos;
    use crate::server::permissions::require_user;
    use crate::server_fns::crm::{clean_text, MAX_LONG_TEXT};

    let user = require_user().await?;
    let case_id = authorize_memo_review(&user, &memo_id).await?;
    let comment = clean_text(&comment, "comment", MAX_LONG_TEXT).map_err(ServerFnError::new)?;
    if comment.is_empty() {
        return Err(ServerFnError::new("Say what needs to change."));
    }
    legal_memos::request_revisions(
        &memo_id,
        &case_id,
        version,
        &user.id,
        &user.full_name(),
        &comment,
    )
    .await
    .map_err(memo_error)
}

#[server(prefix = "/api")]
pub async fn approve_legal_memo(
    memo_id: String,
    version: i32,
    comment: String,
) -> Result<(), ServerFnError> {
    use crate::server::db::legal_memos;
    use crate::server::permissions::require_user;
    use crate::server_fns::crm::{clean_text, MAX_LONG_TEXT};

    let user = require_user().await?;
    let case_id = authorize_memo_review(&user, &memo_id).await?;
    let comment = clean_text(&comment, "comment", MAX_LONG_TEXT).map_err(ServerFnError::new)?;
    legal_memos::approve(
        &memo_id,
        &case_id,
        version,
        &user.id,
        &user.full_name(),
        &comment,
    )
    .await
    .map_err(memo_error)
}

#[server(prefix = "/api")]
pub async fn withdraw_legal_memo(memo_id: String) -> Result<(), ServerFnError> {
    use crate::server::db::legal_memos;
    use crate::server::permissions::require_user;

    let user = require_user().await?;
    let case_id = authorize_memo_author(&user, &memo_id).await?;
    if !case_accepts_changes(&case_id).await? {
        return Err(ServerFnError::new("This case can no longer be changed."));
    }
    legal_memos::withdraw_from_review(&memo_id, &case_id, &user.id, &user.full_name())
        .await
        .map_err(memo_error)
}

#[server(prefix = "/api")]
pub async fn delete_legal_memo_draft(memo_id: String) -> Result<(), ServerFnError> {
    use crate::server::db::legal_memos;
    use crate::server::permissions::require_user;

    let user = require_user().await?;
    let case_id = authorize_memo_author(&user, &memo_id).await?;
    legal_memos::delete_draft(&memo_id, &case_id, &user.id, &user.full_name())
        .await
        .map_err(memo_error)
}

#[cfg(feature = "ssr")]
async fn authorize_memo_read(
    user: &crate::server_fns::users::User,
    memo_id: &str,
) -> Result<crate::server::db::legal_memos::MemoHeader, ServerFnError> {
    use crate::server::db::legal_memos;
    use crate::server::permissions::require_cap;
    use crate::server_fns::capabilities::CaseCapability;

    crate::server_fns::crm::require_staff(user)?;
    let not_found = || ServerFnError::new("Legal memo not found.");
    let header = legal_memos::header(memo_id)
        .await
        .map_err(memo_error)?
        .ok_or_else(not_found)?;
    require_cap(user, &header.case_id, CaseCapability::ViewEvidence)
        .await
        .map_err(|_| not_found())?;
    Ok(header)
}

#[cfg(feature = "ssr")]
async fn authorize_memo_author(
    user: &crate::server_fns::users::User,
    memo_id: &str,
) -> Result<String, ServerFnError> {
    use crate::server::permissions::require_cap;
    use crate::server_fns::capabilities::CaseCapability;

    let header = authorize_memo_read(user, memo_id).await?;
    require_cap(user, &header.case_id, CaseCapability::AddNotes).await?;
    if header.author_id != user.id {
        return Err(ServerFnError::new("Only the memo's author can change it."));
    }
    Ok(header.case_id)
}

#[cfg(feature = "ssr")]
async fn authorize_memo_review(
    user: &crate::server_fns::users::User,
    memo_id: &str,
) -> Result<String, ServerFnError> {
    let header = authorize_memo_read(user, memo_id).await?;
    if !case_accepts_changes(&header.case_id).await? {
        return Err(ServerFnError::new("This case can no longer be changed."));
    }
    Ok(header.case_id)
}

#[cfg(feature = "ssr")]
async fn can_add_notes(user: &crate::server_fns::users::User, case_id: &str) -> bool {
    crate::server::permissions::require_cap(
        user,
        case_id,
        crate::server_fns::capabilities::CaseCapability::AddNotes,
    )
    .await
    .is_ok()
}

#[cfg(feature = "ssr")]
async fn case_accepts_changes(case_id: &str) -> Result<bool, ServerFnError> {
    Ok(crate::server::db::cases::status(case_id)
        .await
        .map_err(memo_error)?
        .is_some_and(|status| status.accepts_changes()))
}

#[cfg(feature = "ssr")]
async fn clean_header(
    case_id: &str,
    title: &str,
    attorney_id: &str,
    author_id: &str,
) -> Result<(String, Option<MemoAttorney>), ServerFnError> {
    use crate::server::db::legal_memos;
    use crate::server_fns::crm::clean_text;

    let title = clean_text(title, "title", MAX_SHORT_TEXT).map_err(ServerFnError::new)?;
    let attorney_id = attorney_id.trim();
    if attorney_id.is_empty() {
        return Ok((title, None));
    }
    if attorney_id == author_id {
        return Err(ServerFnError::new(
            "Choose a reviewer other than yourself. Someone else has to review your memo.",
        ));
    }
    let attorney = legal_memos::attorney_choices(case_id)
        .await
        .map_err(memo_error)?
        .into_iter()
        .find(|a| a.id == attorney_id)
        .ok_or_else(|| ServerFnError::new("Choose a reviewer who is on this case."))?;
    Ok((title, Some(attorney)))
}

#[cfg(feature = "ssr")]
fn memo_error(error: sqlx::Error) -> ServerFnError {
    match error {
        sqlx::Error::Protocol(message) => ServerFnError::new(message),
        other => {
            tracing::warn!("legal memo request failed: {other}");
            ServerFnError::new("Something went wrong with the legal memo. Try again.")
        }
    }
}
