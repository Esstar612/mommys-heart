use crate::helpers::visibility::Visibility;
use crate::server::db::{audit, ids, local_stamp, pool};
use crate::server_fns::cases::CaseStatus;
use crate::server_fns::legal_memos::{
    LegalMemo, LegalMemoContent, LegalMemoSummary, MemoAttorney, MemoEvent, MemoStatus,
    VersionEvent, DOCKET_KEYS, STALE_MEMO_ERROR,
};
use crate::server_fns::users::AccountRole;

type Tx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

struct Locked {
    status: MemoStatus,
    version: i32,
    title: String,
    content: String,
    author_id: String,
    attorney_id: String,
    attorney_name: String,
    revision: String,
}

fn parse_content(raw: &str) -> Result<LegalMemoContent, sqlx::Error> {
    serde_json::from_str(raw)
        .map_err(|_| sqlx::Error::Protocol("This memo couldn't be read.".to_string()))
}

fn to_json<T: serde::Serialize>(value: &T) -> Result<String, sqlx::Error> {
    serde_json::to_string(value)
        .map_err(|_| sqlx::Error::Protocol("This memo couldn't be saved.".to_string()))
}

pub struct MemoHeader {
    pub case_id: String,
    pub author_id: String,
}

pub async fn header(memo_id: &str) -> Result<Option<MemoHeader>, sqlx::Error> {
    sqlx::query_as::<_, (String, String)>(
        "SELECT case_id, author_id FROM legal_memos WHERE id = $1",
    )
    .bind(memo_id)
    .fetch_optional(pool())
    .await
    .map(|row| row.map(|(case_id, author_id)| MemoHeader { case_id, author_id }))
}

#[derive(sqlx::FromRow)]
struct SummaryRow {
    id: String,
    title: String,
    status: String,
    version: i32,
    author_name: String,
    updated_at: chrono::DateTime<chrono::Utc>,
}

pub async fn list(case_id: &str) -> Result<Vec<LegalMemoSummary>, sqlx::Error> {
    let rows = sqlx::query_as::<_, SummaryRow>(
        "SELECT id, title, status, version, author_name, updated_at
         FROM legal_memos WHERE case_id = $1 ORDER BY seq DESC",
    )
    .bind(case_id)
    .fetch_all(pool())
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| LegalMemoSummary {
            id: r.id,
            title: r.title,
            status: MemoStatus::from_slug(&r.status).unwrap_or(MemoStatus::Draft),
            version: r.version,
            author_name: r.author_name,
            updated_on: local_stamp(r.updated_at)[..10].to_string(),
        })
        .collect())
}

#[derive(sqlx::FromRow)]
struct MemoRow {
    id: String,
    case_id: String,
    case_name: String,
    docket: String,
    title: String,
    status: String,
    version: i32,
    attorney_user_id: Option<String>,
    attorney_name: String,
    author_id: String,
    author_name: String,
    content: String,
    approved_by_name: String,
    approved_at: Option<chrono::DateTime<chrono::Utc>>,
    revision: String,
}

pub async fn get(memo_id: &str) -> Result<Option<LegalMemo>, sqlx::Error> {
    let row = sqlx::query_as::<_, MemoRow>(
        "SELECT m.id, m.case_id, c.name AS case_name,
                COALESCE((SELECT btrim(p.value) FROM case_properties p
                          WHERE p.case_id = c.id AND p.key = ANY($2) AND btrim(p.value) <> ''
                          ORDER BY array_position($2, p.key), p.ord LIMIT 1), '') AS docket,
                m.title, m.status, m.version,
                m.attorney_user_id, m.attorney_name, m.author_id, m.author_name,
                m.content::text AS content, m.approved_by_name, m.approved_at,
                m.updated_at::text AS revision
         FROM legal_memos m JOIN cases c ON c.id = m.case_id
         WHERE m.id = $1",
    )
    .bind(memo_id)
    .bind(&DOCKET_KEYS[..])
    .fetch_optional(pool())
    .await?;
    let Some(r) = row else {
        return Ok(None);
    };
    Ok(Some(LegalMemo {
        last_sent: latest_event(&r.id, VersionEvent::Sent).await?,
        last_revisions_request: latest_event(&r.id, VersionEvent::RevisionsRequested).await?,
        last_approval: latest_event(&r.id, VersionEvent::Approved).await?,
        id: r.id,
        case_id: r.case_id,
        case_name: r.case_name,
        docket: r.docket,
        title: r.title,
        status: MemoStatus::from_slug(&r.status).unwrap_or(MemoStatus::Draft),
        version: r.version,
        attorney_id: r.attorney_user_id.unwrap_or_default(),
        attorney_name: r.attorney_name,
        author_id: r.author_id,
        author_name: r.author_name,
        content: parse_content(&r.content)?,
        approved_by_name: r.approved_by_name,
        approved_at: r.approved_at.map(local_stamp).unwrap_or_default(),
        revision: r.revision,
        can_edit: false,
        can_review: false,
        can_withdraw: false,
    }))
}

async fn latest_event(
    memo_id: &str,
    event: VersionEvent,
) -> Result<Option<MemoEvent>, sqlx::Error> {
    sqlx::query_as::<_, (String, String, chrono::DateTime<chrono::Utc>)>(
        "SELECT actor_name, comment, created_at FROM legal_memo_versions
         WHERE memo_id = $1 AND event = $2 ORDER BY seq DESC LIMIT 1",
    )
    .bind(memo_id)
    .bind(event.slug())
    .fetch_optional(pool())
    .await
    .map(|row| {
        row.map(|(actor_name, comment, at)| MemoEvent {
            actor_name,
            comment,
            at: local_stamp(at),
        })
    })
}

pub async fn attorney_choices(case_id: &str) -> Result<Vec<MemoAttorney>, sqlx::Error> {
    sqlx::query_as::<_, (String, String)>(&format!(
        "SELECT u.id, btrim(u.first_name || ' ' || u.last_name) AS name
         FROM users u
         WHERE u.role IN {staff}
           AND EXISTS (SELECT 1 FROM case_assignments a
                       WHERE a.case_id = $1 AND a.user_id = u.id
                         AND a.capability = 'view_evidence')
         ORDER BY name",
        staff = AccountRole::STAFF_ROLES_SQL
    ))
    .bind(case_id)
    .fetch_all(pool())
    .await
    .map(|rows| {
        rows.into_iter()
            .map(|(id, name)| MemoAttorney { id, name })
            .collect()
    })
}

pub async fn create(
    case_id: &str,
    title: &str,
    author_id: &str,
    author_name: &str,
) -> Result<String, sqlx::Error> {
    let mut tx = pool().begin().await?;
    audit::set_actor_in_transaction(&mut tx, author_id).await?;
    let case_status: Option<String> =
        sqlx::query_scalar("SELECT status FROM cases WHERE id = $1 FOR SHARE")
            .bind(case_id)
            .fetch_optional(&mut *tx)
            .await?;
    if !case_status
        .as_deref()
        .and_then(CaseStatus::from_slug)
        .is_some_and(CaseStatus::accepts_changes)
    {
        return Err(sqlx::Error::Protocol(
            "This case can no longer be changed.".into(),
        ));
    }
    let id = ids::next(&mut *tx, "lm").await?;
    sqlx::query(
        "INSERT INTO legal_memos (id, case_id, title, author_id, author_name)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(&id)
    .bind(case_id)
    .bind(title)
    .bind(author_id)
    .bind(author_name)
    .execute(&mut *tx)
    .await?;
    record(&mut tx, case_id, &id, author_name, "", "created").await?;
    tx.commit().await?;
    Ok(id)
}

pub async fn save_draft(
    memo_id: &str,
    author_id: &str,
    revision: &str,
    title: &str,
    attorney: Option<&MemoAttorney>,
    content: &LegalMemoContent,
) -> Result<String, sqlx::Error> {
    let mut tx = pool().begin().await?;
    audit::set_actor_in_transaction(&mut tx, author_id).await?;
    let locked = lock(&mut tx, memo_id).await?;
    require_author(&locked, author_id)?;
    require_revision(&locked, revision)?;
    require_editable(&locked)?;
    let next = write_draft(&mut tx, memo_id, title, attorney, content).await?;
    tx.commit().await?;
    Ok(next)
}

#[allow(clippy::too_many_arguments)]
pub async fn send_for_review(
    memo_id: &str,
    case_id: &str,
    actor_id: &str,
    actor: &str,
    revision: &str,
    title: &str,
    attorney: &MemoAttorney,
    content: &LegalMemoContent,
) -> Result<(), sqlx::Error> {
    let mut tx = pool().begin().await?;
    audit::set_actor_in_transaction(&mut tx, actor_id).await?;
    let locked = lock(&mut tx, memo_id).await?;
    require_author(&locked, actor_id)?;
    require_revision(&locked, revision)?;
    require_editable(&locked)?;
    write_draft(&mut tx, memo_id, title, Some(attorney), content).await?;
    sqlx::query(
        "UPDATE legal_memos SET status = 'in_review', updated_at = clock_timestamp() WHERE id = $1",
    )
    .bind(memo_id)
    .execute(&mut *tx)
    .await?;
    add_version(
        &mut tx,
        memo_id,
        locked.version,
        VersionEvent::Sent,
        title,
        &to_json(content)?,
        &attorney.name,
        actor_id,
        actor,
        "",
    )
    .await?;
    record(
        &mut tx,
        case_id,
        memo_id,
        actor,
        locked.status.label(),
        &format!(
            "sent to {} for review (version {})",
            attorney.name, locked.version
        ),
    )
    .await?;
    tx.commit().await
}

#[allow(clippy::too_many_arguments)]
pub async fn request_revisions(
    memo_id: &str,
    case_id: &str,
    version: i32,
    actor_id: &str,
    actor: &str,
    comment: &str,
) -> Result<(), sqlx::Error> {
    let mut tx = pool().begin().await?;
    audit::set_actor_in_transaction(&mut tx, actor_id).await?;
    let locked = lock(&mut tx, memo_id).await?;
    require_in_review(&locked, version)?;
    require_reviewer(&locked, actor_id)?;
    add_version(
        &mut tx,
        memo_id,
        locked.version,
        VersionEvent::RevisionsRequested,
        &locked.title,
        &locked.content,
        &locked.attorney_name,
        actor_id,
        actor,
        comment,
    )
    .await?;
    sqlx::query(
        "UPDATE legal_memos
         SET status = 'revisions_requested', version = version + 1, updated_at = now()
         WHERE id = $1",
    )
    .bind(memo_id)
    .execute(&mut *tx)
    .await?;
    record(
        &mut tx,
        case_id,
        memo_id,
        actor,
        locked.status.label(),
        &format!("revisions requested on version {}", locked.version),
    )
    .await?;
    tx.commit().await
}

#[allow(clippy::too_many_arguments)]
pub async fn approve(
    memo_id: &str,
    case_id: &str,
    version: i32,
    actor_id: &str,
    actor: &str,
    comment: &str,
) -> Result<(), sqlx::Error> {
    let mut tx = pool().begin().await?;
    audit::set_actor_in_transaction(&mut tx, actor_id).await?;
    let locked = lock(&mut tx, memo_id).await?;
    require_in_review(&locked, version)?;
    require_reviewer(&locked, actor_id)?;
    add_version(
        &mut tx,
        memo_id,
        locked.version,
        VersionEvent::Approved,
        &locked.title,
        &locked.content,
        &locked.attorney_name,
        actor_id,
        actor,
        comment,
    )
    .await?;
    sqlx::query(
        "UPDATE legal_memos
         SET status = 'approved', approved_by_name = $2, approved_at = now(), updated_at = now()
         WHERE id = $1",
    )
    .bind(memo_id)
    .bind(actor)
    .execute(&mut *tx)
    .await?;
    record(
        &mut tx,
        case_id,
        memo_id,
        actor,
        locked.status.label(),
        &format!("approved version {}", locked.version),
    )
    .await?;
    tx.commit().await
}

pub async fn withdraw_from_review(
    memo_id: &str,
    case_id: &str,
    actor_id: &str,
    actor: &str,
) -> Result<(), sqlx::Error> {
    let mut tx = pool().begin().await?;
    audit::set_actor_in_transaction(&mut tx, actor_id).await?;
    let locked = lock(&mut tx, memo_id).await?;
    require_author(&locked, actor_id)?;
    if locked.status != MemoStatus::InReview {
        return Err(sqlx::Error::Protocol(
            "This memo isn't waiting for attorney review.".into(),
        ));
    }
    add_version(
        &mut tx,
        memo_id,
        locked.version,
        VersionEvent::Withdrawn,
        &locked.title,
        &locked.content,
        &locked.attorney_name,
        actor_id,
        actor,
        "",
    )
    .await?;
    sqlx::query(
        "UPDATE legal_memos SET status = 'draft', version = version + 1, updated_at = now()
         WHERE id = $1",
    )
    .bind(memo_id)
    .execute(&mut *tx)
    .await?;
    record(
        &mut tx,
        case_id,
        memo_id,
        actor,
        locked.status.label(),
        &format!("withdrew version {} from review", locked.version),
    )
    .await?;
    tx.commit().await
}

pub async fn delete_draft(
    memo_id: &str,
    case_id: &str,
    actor_id: &str,
    actor: &str,
) -> Result<(), sqlx::Error> {
    let mut tx = pool().begin().await?;
    audit::set_actor_in_transaction(&mut tx, actor_id).await?;
    let locked = lock(&mut tx, memo_id).await?;
    require_author(&locked, actor_id)?;
    let sent: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM legal_memo_versions WHERE memo_id = $1)")
            .bind(memo_id)
            .fetch_one(&mut *tx)
            .await?;
    if sent || locked.status != MemoStatus::Draft {
        return Err(sqlx::Error::Protocol(
            "Only a draft that was never sent can be deleted.".into(),
        ));
    }
    sqlx::query("DELETE FROM legal_memos WHERE id = $1")
        .bind(memo_id)
        .execute(&mut *tx)
        .await?;
    record(
        &mut tx,
        case_id,
        memo_id,
        actor,
        &locked.title,
        "deleted draft",
    )
    .await?;
    tx.commit().await
}

async fn lock(tx: &mut Tx<'_>, memo_id: &str) -> Result<Locked, sqlx::Error> {
    #[derive(sqlx::FromRow)]
    struct Row {
        status: String,
        version: i32,
        title: String,
        content: String,
        author_id: String,
        attorney_user_id: Option<String>,
        attorney_name: String,
        revision: String,
        case_status: String,
    }
    let row = sqlx::query_as::<_, Row>(
        "SELECT m.status, m.version, m.title, m.content::text AS content, m.author_id,
                m.attorney_user_id, m.attorney_name, m.updated_at::text AS revision,
                c.status AS case_status
         FROM legal_memos m JOIN cases c ON c.id = m.case_id
         WHERE m.id = $1
         FOR UPDATE OF m FOR SHARE OF c",
    )
    .bind(memo_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| sqlx::Error::Protocol("Legal memo not found.".into()))?;
    if !CaseStatus::from_slug(&row.case_status).is_some_and(CaseStatus::accepts_changes) {
        return Err(sqlx::Error::Protocol(
            "This case can no longer be changed.".into(),
        ));
    }
    Ok(Locked {
        status: MemoStatus::from_slug(&row.status).unwrap_or(MemoStatus::Draft),
        version: row.version,
        title: row.title,
        content: row.content,
        author_id: row.author_id,
        attorney_id: row.attorney_user_id.unwrap_or_default(),
        attorney_name: row.attorney_name,
        revision: row.revision,
    })
}

fn require_author(locked: &Locked, actor_id: &str) -> Result<(), sqlx::Error> {
    if locked.author_id == actor_id {
        Ok(())
    } else {
        Err(sqlx::Error::Protocol(
            "Only the memo's author can change it.".into(),
        ))
    }
}

fn require_revision(locked: &Locked, revision: &str) -> Result<(), sqlx::Error> {
    if locked.revision == revision {
        Ok(())
    } else {
        Err(sqlx::Error::Protocol(STALE_MEMO_ERROR.into()))
    }
}

fn require_editable(locked: &Locked) -> Result<(), sqlx::Error> {
    match locked.status {
        MemoStatus::Draft | MemoStatus::RevisionsRequested => Ok(()),
        MemoStatus::InReview => Err(sqlx::Error::Protocol(
            "This memo is with the attorney for review and can't be edited right now.".into(),
        )),
        MemoStatus::Approved => Err(sqlx::Error::Protocol(
            "This memo is approved and can't be changed.".into(),
        )),
    }
}

fn require_reviewer(locked: &Locked, actor_id: &str) -> Result<(), sqlx::Error> {
    if locked.author_id == actor_id {
        return Err(sqlx::Error::Protocol(
            "You wrote this memo, so someone else has to review it.".into(),
        ));
    }
    if !locked.attorney_id.is_empty() && locked.attorney_id == actor_id {
        Ok(())
    } else {
        Err(sqlx::Error::Protocol(
            "Only the reviewer this memo was sent to can review it.".into(),
        ))
    }
}

fn require_in_review(locked: &Locked, version: i32) -> Result<(), sqlx::Error> {
    if locked.status != MemoStatus::InReview {
        return Err(sqlx::Error::Protocol(
            "This memo isn't waiting for attorney review.".into(),
        ));
    }
    if locked.version != version {
        return Err(sqlx::Error::Protocol(STALE_MEMO_ERROR.into()));
    }
    Ok(())
}

async fn write_draft(
    tx: &mut Tx<'_>,
    memo_id: &str,
    title: &str,
    attorney: Option<&MemoAttorney>,
    content: &LegalMemoContent,
) -> Result<String, sqlx::Error> {
    sqlx::query_scalar(
        "UPDATE legal_memos
         SET title = $2, attorney_user_id = $3, attorney_name = $4,
             content = $5::jsonb, updated_at = clock_timestamp()
         WHERE id = $1
         RETURNING updated_at::text",
    )
    .bind(memo_id)
    .bind(title)
    .bind(attorney.map(|a| a.id.as_str()))
    .bind(attorney.map(|a| a.name.as_str()).unwrap_or_default())
    .bind(to_json(content)?)
    .fetch_one(&mut **tx)
    .await
}

#[allow(clippy::too_many_arguments)]
async fn add_version(
    tx: &mut Tx<'_>,
    memo_id: &str,
    version: i32,
    event: VersionEvent,
    title: &str,
    content_json: &str,
    attorney_name: &str,
    actor_id: &str,
    actor: &str,
    comment: &str,
) -> Result<(), sqlx::Error> {
    let id = ids::next(&mut **tx, "lv").await?;
    sqlx::query(
        "INSERT INTO legal_memo_versions
             (id, memo_id, version, event, title, content, attorney_name, actor_id, actor_name,
              comment)
         VALUES ($1, $2, $3, $4, $5, $6::jsonb, $7, $8, $9, $10)",
    )
    .bind(&id)
    .bind(memo_id)
    .bind(version)
    .bind(event.slug())
    .bind(title)
    .bind(content_json)
    .bind(attorney_name)
    .bind(actor_id)
    .bind(actor)
    .bind(comment)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

async fn record(
    tx: &mut Tx<'_>,
    case_id: &str,
    memo_id: &str,
    actor: &str,
    old: &str,
    new: &str,
) -> Result<(), sqlx::Error> {
    audit::record_in_transaction_with_visibility(
        tx,
        audit::Entity::Case,
        case_id,
        actor,
        &format!("legal memo {memo_id}"),
        old,
        new,
        Visibility::VolunteerOnly,
    )
    .await
}
