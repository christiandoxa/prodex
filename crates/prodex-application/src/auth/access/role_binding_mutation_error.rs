use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ApplicationRoleBindingMutationErrorStatus {
    ServiceUnavailable,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApplicationRoleBindingMutationErrorResponsePlan {
    pub status: ApplicationRoleBindingMutationErrorStatus,
    pub code: &'static str,
    pub message: &'static str,
}

pub fn plan_application_role_binding_mutation_error_response(
    error: &ApplicationRoleBindingMutationError,
) -> ApplicationRoleBindingMutationErrorResponsePlan {
    match error {
        ApplicationRoleBindingMutationError::Postgres(error) => {
            application_role_binding_mutation_response_from_postgres(
                plan_postgres_storage_error_response(error),
            )
        }
        ApplicationRoleBindingMutationError::Sqlite(error) => {
            application_role_binding_mutation_response_from_sqlite(
                plan_sqlite_storage_error_response(error),
            )
        }
    }
}

fn application_role_binding_mutation_response_from_postgres(
    response: PostgresStorageErrorResponsePlan,
) -> ApplicationRoleBindingMutationErrorResponsePlan {
    ApplicationRoleBindingMutationErrorResponsePlan {
        status: match response.status {
            PostgresStorageErrorStatus::ServiceUnavailable => {
                ApplicationRoleBindingMutationErrorStatus::ServiceUnavailable
            }
        },
        code: "role_binding_storage_unavailable",
        message: "role-binding storage is temporarily unavailable",
    }
}

fn application_role_binding_mutation_response_from_sqlite(
    response: SqliteStorageErrorResponsePlan,
) -> ApplicationRoleBindingMutationErrorResponsePlan {
    ApplicationRoleBindingMutationErrorResponsePlan {
        status: match response.status {
            SqliteStorageErrorStatus::ServiceUnavailable => {
                ApplicationRoleBindingMutationErrorStatus::ServiceUnavailable
            }
        },
        code: "role_binding_storage_unavailable",
        message: "role-binding storage is temporarily unavailable",
    }
}
