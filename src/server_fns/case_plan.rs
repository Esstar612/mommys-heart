//! The approved case plan: staff-agreed actions and the tasks assigned for them.
//!
//! Staff only. Reading needs `ViewCase`; adding an action needs `EditCase`; a
//! task's outcome can be recorded by its assignee or by anyone who can edit.

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::server_fns::crm::coded_enum;

coded_enum!(TaskOutcome {
    Completed => ("completed", "Completed"),
    DeclinedByClient => ("declined_by_client", "Declined by client"),
    CouldNotReach => ("could_not_reach", "Couldn't reach client"),
    NoLongerNeeded => ("no_longer_needed", "No longer needed"),
});

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CaseTask {
    pub id: String,
    pub assignee_id: String,
    pub assignee_name: String,
    pub follow_up_on: String,
    pub outcome: Option<TaskOutcome>,
    pub outcome_notes: String,
    pub outcome_recorded_on: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CasePlanAction {
    pub id: String,
    pub title: String,
    pub added_by_name: String,
    pub added_on: String,
    pub tasks: Vec<CaseTask>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PlanAssignee {
    pub id: String,
    pub name: String,
}

#[server(prefix = "/api")]
pub async fn list_case_plan(case_id: String) -> Result<Vec<CasePlanAction>, ServerFnError> {
    use crate::server::db::case_plan;
    use crate::server::permissions::{require_cap, require_user};
    use crate::server_fns::capabilities::CaseCapability;

    let user = require_user().await?;
    crate::server_fns::crm::require_staff(&user)?;
    require_cap(&user, &case_id, CaseCapability::ViewCase).await?;
    case_plan::list(&case_id).await.map_err(ServerFnError::new)
}

#[server(prefix = "/api")]
pub async fn list_plan_assignees(case_id: String) -> Result<Vec<PlanAssignee>, ServerFnError> {
    use crate::server::db::case_plan;
    use crate::server::permissions::{require_cap, require_user};
    use crate::server_fns::capabilities::CaseCapability;

    let user = require_user().await?;
    crate::server_fns::crm::require_staff(&user)?;
    require_cap(&user, &case_id, CaseCapability::EditCase).await?;
    case_plan::assignees(&case_id)
        .await
        .map_err(ServerFnError::new)
}

#[server(prefix = "/api")]
pub async fn add_plan_action(
    case_id: String,
    title: String,
    assignee_id: String,
    follow_up_on: String,
) -> Result<(), ServerFnError> {
    use crate::server::db::case_plan;
    use crate::server::permissions::{require_cap, require_user};
    use crate::server_fns::capabilities::CaseCapability;
    use crate::server_fns::crm::{clean_date, clean_text, MAX_SHORT_TEXT};

    let user = require_user().await?;
    crate::server_fns::crm::require_staff(&user)?;
    require_cap(&user, &case_id, CaseCapability::EditCase).await?;
    let title = clean_text(&title, "action", MAX_SHORT_TEXT).map_err(ServerFnError::new)?;
    if title.is_empty() {
        return Err(ServerFnError::new("Describe the action."));
    }
    let follow_up_on = clean_date(&follow_up_on, "follow-up date").map_err(ServerFnError::new)?;
    let assignee = case_plan::assignees(&case_id)
        .await
        .map_err(ServerFnError::new)?
        .into_iter()
        .find(|a| a.id == assignee_id)
        .ok_or_else(|| ServerFnError::new("Choose someone who is on this case."))?;
    case_plan::add_action(
        &case_id,
        &title,
        &assignee,
        &follow_up_on,
        &user.id,
        &user.full_name(),
    )
    .await
    .map_err(ServerFnError::new)
}

#[server(prefix = "/api")]
pub async fn record_task_outcome(
    task_id: String,
    outcome: Option<TaskOutcome>,
    notes: String,
    follow_up_on: String,
) -> Result<(), ServerFnError> {
    use crate::server::db::case_plan;
    use crate::server::permissions::{require_cap, require_user};
    use crate::server_fns::capabilities::CaseCapability;
    use crate::server_fns::crm::{clean_date, clean_text, MAX_LONG_TEXT};

    let user = require_user().await?;
    crate::server_fns::crm::require_staff(&user)?;
    let (case_id, assignee_id) = case_plan::task_owner(&task_id)
        .await
        .map_err(ServerFnError::new)?
        .ok_or_else(|| ServerFnError::new("Task not found."))?;
    let cap = if assignee_id.as_deref() == Some(user.id.as_str()) {
        CaseCapability::AddNotes
    } else {
        CaseCapability::EditCase
    };
    require_cap(&user, &case_id, cap).await?;
    let notes = clean_text(&notes, "outcome notes", MAX_LONG_TEXT).map_err(ServerFnError::new)?;
    let follow_up_on = clean_date(&follow_up_on, "follow-up date").map_err(ServerFnError::new)?;
    case_plan::record_outcome(
        &task_id,
        &case_id,
        outcome,
        &notes,
        &follow_up_on,
        &user.id,
        &user.full_name(),
    )
    .await
    .map_err(ServerFnError::new)
}
