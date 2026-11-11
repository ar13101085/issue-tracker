//! Who may do what in a repository.
//!
//! Site admins and the owner are `Admin`. Collaborators have their stored role.
//! A public repository gives every signed-in account `Read` (enough to open
//! issues) and lets anonymous visitors look. A private repository the caller
//! has no role in answers 404, so its existence is not revealed.

use crate::error::ApiError;
use crate::models::{RepoDoc, UserDoc};
use crate::state::AppState;
use mongodb::bson::doc;
use shared::Role;

pub struct RepoAccess {
    pub repo: RepoDoc,
    pub viewer: Option<UserDoc>,
    /// `None` only for an anonymous visitor of a public repository.
    pub role: Option<Role>,
}

impl RepoAccess {
    pub fn user(&self) -> Result<&UserDoc, ApiError> {
        self.viewer.as_ref().ok_or(ApiError::Unauthorized)
    }

    pub fn has(&self, min: Role) -> bool {
        self.role.is_some_and(|r| r >= min)
    }

    pub fn require(&self, min: Role) -> Result<&UserDoc, ApiError> {
        let user = self.user()?;
        if self.has(min) {
            Ok(user)
        } else {
            Err(ApiError::forbidden("You do not have permission to do this in this repository"))
        }
    }
}

pub async fn role_in(state: &AppState, repo: &RepoDoc, viewer: Option<&UserDoc>) -> Result<Option<Role>, ApiError> {
    let Some(user) = viewer else {
        return Ok(None);
    };
    if user.is_admin || user.id == repo.owner_id {
        return Ok(Some(Role::Admin));
    }
    let collab = state.db.collaborators.find_one(doc! {"repo_id": repo.id, "user_id": user.id}).await?;
    Ok(match collab {
        Some(c) => Some(c.role),
        None if repo.private => None,
        None => Some(Role::Read),
    })
}

pub async fn load_repo(
    state: &AppState,
    owner: &str,
    name: &str,
    viewer: Option<UserDoc>,
) -> Result<RepoAccess, ApiError> {
    let filter = doc! {"owner_lower": owner.to_lowercase(), "name_lower": name.to_lowercase()};
    let repo = state.db.repos.find_one(filter).await?.ok_or(ApiError::NotFound)?;
    let role = role_in(state, &repo, viewer.as_ref()).await?;
    if repo.private && role.is_none() {
        return Err(ApiError::NotFound);
    }
    Ok(RepoAccess { repo, viewer, role })
}
