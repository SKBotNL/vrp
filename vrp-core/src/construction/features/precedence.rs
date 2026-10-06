//! Provides way to enforce job ordering across scoped jobs.

#[cfg(test)]
#[path = "../../../tests/unit/construction/features/precedence_test.rs"]
mod job_order_test;

use super::*;
use crate::models::solution::Activity;
use std::collections::HashSet;

custom_dimension!(pub JobPrecedence typeof (String, i32));
custom_tour_state!(CurrentScopes typeof HashSet<String>);

/// Creates a precedence feature as hard constraint.
pub fn create_precedence_feature(name: &str, code: ViolationCode) -> Result<Feature, GenericError> {
    FeatureBuilder::default()
        .with_name(name)
        .with_constraint(PrecedenceConstraint { code })
        .with_state(PrecedenceState {})
        .build()
}

struct PrecedenceConstraint {
    code: ViolationCode,
}

fn get_precedence(activity: Option<&Activity>) -> Option<&(String, i32)> {
    activity.and_then(|a| a.job.as_ref()).and_then(|j| j.dimens.get_job_precedence())
}

impl FeatureConstraint for PrecedenceConstraint {
    fn evaluate(&self, move_ctx: &MoveContext<'_>) -> Option<ConstraintViolation> {
        match move_ctx {
            MoveContext::Activity { route_ctx, activity_ctx, .. } => {
                let (scope, rank) = get_precedence(Some(activity_ctx.target))?;

                let early = (0..=activity_ctx.index)
                    .filter_map(|idx| get_precedence(route_ctx.route().tour.get(idx)))
                    .filter(|(s, _)| s == scope)
                    .map(|p| (p, true));

                let late = (activity_ctx.index + 1..route_ctx.route().tour.total())
                    .filter_map(|idx| get_precedence(route_ctx.route().tour.get(idx)))
                    .filter(|(s, _)| s == scope)
                    .map(|p| (p, false));

                let violation = early
                    .chain(late)
                    .find(|((_, r), is_early)| if *is_early { r > rank } else { r < rank })
                    .map(|(_, is_early)| ConstraintViolation { code: self.code, stopped: is_early });

                violation
            }
            MoveContext::Route { solution_ctx, route_ctx, job } => {
                job.dimens().get_job_precedence().and_then(|(scope, _)| {
                    let other_route = solution_ctx
                        .routes
                        .iter()
                        .filter(|rc| rc.route().actor != route_ctx.route().actor)
                        .filter_map(|rc| rc.state().get_current_scopes())
                        .any(|scopes| scopes.contains(scope));

                    if other_route { ConstraintViolation::fail(self.code) } else { None }
                })
            }
        }
    }

    fn merge(&self, source: Job, candidate: Job) -> Result<Job, ViolationCode> {
        match (source.dimens().get_job_precedence(), candidate.dimens().get_job_precedence()) {
            (None, None) => Ok(source),
            (Some(s), Some(c)) if s == c => Ok(source),
            _ => Err(self.code),
        }
    }
}

struct PrecedenceState {}

impl FeatureState for PrecedenceState {
    fn accept_insertion(&self, solution_ctx: &mut SolutionContext, route_index: usize, job: &Job) {
        if let Some((scope, _)) = job.dimens().get_job_precedence() {
            let route_ctx = solution_ctx.routes.get_mut(route_index).unwrap();

            let mut scopes = get_scopes(route_ctx);
            scopes.insert(scope.clone());

            route_ctx.state_mut().set_current_scopes(scopes);
        }
    }

    fn accept_route_state(&self, _: &mut RouteContext) {}

    fn accept_solution_state(&self, solution_ctx: &mut SolutionContext) {
        solution_ctx.routes.iter_mut().for_each(|route_ctx| {
            let scopes = get_scopes(route_ctx);
            route_ctx.state_mut().set_current_scopes(scopes);
        });
    }
}

fn get_scopes(route_ctx: &RouteContext) -> HashSet<String> {
    route_ctx
        .route()
        .tour
        .jobs()
        .filter_map(|job| job.dimens().get_job_precedence())
        .map(|(scope, _)| scope.clone())
        .collect()
}
