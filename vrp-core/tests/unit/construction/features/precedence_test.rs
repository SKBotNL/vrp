use super::*;
use crate::helpers::models::domain::{TestGoalContextBuilder, test_random};
use crate::helpers::models::problem::{FleetBuilder, TestSingleBuilder, test_driver, test_vehicle_with_id};
use crate::helpers::models::solution::{ActivityBuilder, RouteBuilder, RouteContextBuilder, RouteStateBuilder};
use crate::models::problem::{Fleet, Single};
use crate::models::solution::Registry;
use std::collections::HashMap;
use std::sync::Arc;

const VIOLATION_CODE: ViolationCode = ViolationCode(1);

type Precedence = (&'static str, i32);

fn create_test_precedence_feature() -> Feature {
    create_precedence_feature("precedence", VIOLATION_CODE).unwrap()
}

fn create_test_fleet() -> Fleet {
    FleetBuilder::default()
        .add_driver(test_driver())
        .add_vehicle(test_vehicle_with_id("v1"))
        .add_vehicle(test_vehicle_with_id("v2"))
        .build()
}

fn create_test_single(precedence: Option<Precedence>) -> Arc<Single> {
    let mut builder = TestSingleBuilder::default();

    if let Some((scope, order)) = precedence {
        builder.dimens_mut().set_job_precedence((scope.to_string(), order));
    }

    builder.build_shared()
}

fn create_test_solution_context(fleet: &Fleet, routes: Vec<(&str, Vec<Option<Precedence>>)>) -> SolutionContext {
    SolutionContext {
        required: vec![],
        ignored: vec![],
        unassigned: Default::default(),
        locked: Default::default(),
        routes: routes
            .into_iter()
            .map(|(vehicle, precedences)| {
                RouteContextBuilder::default()
                    .with_state(
                        RouteStateBuilder::default()
                            .set_route_state(|state| {
                                state.set_current_scopes(
                                    precedences
                                        .iter()
                                        .filter_map(|p| *p)
                                        .map(|(scope, _)| scope.to_string())
                                        .collect::<HashSet<_>>(),
                                )
                            })
                            .build(),
                    )
                    .with_route(
                        RouteBuilder::default()
                            .with_vehicle(fleet, vehicle)
                            .add_activities(precedences.into_iter().map(|precedence| {
                                ActivityBuilder::with_location(1).job(Some(create_test_single(precedence))).build()
                            }))
                            .build(),
                    )
                    .build()
            })
            .collect(),
        registry: RegistryContext::new(&TestGoalContextBuilder::default().build(), Registry::new(fleet, test_random())),
        state: Default::default(),
    }
}

fn get_actor(fleet: &Fleet, vehicle: &str) -> Arc<Actor> {
    fleet.actors.iter().find(|actor| actor.vehicle.dimens.get_vehicle_id().unwrap() == vehicle).unwrap().clone()
}

fn get_actor_scopes(solution_ctx: &SolutionContext) -> HashMap<String, Arc<Actor>> {
    solution_ctx
        .routes
        .iter()
        .filter_map(|route_ctx| {
            route_ctx.state().get_current_scopes().map(|scopes| (route_ctx.route().actor.clone(), scopes.clone()))
        })
        .fold(HashMap::default(), |mut acc, (actor, scopes)| {
            scopes.into_iter().for_each(|scope| {
                acc.insert(scope, actor.clone());
            });
            acc
        })
}

fn compare_actor_scopes(fleet: &Fleet, original: HashMap<String, Arc<Actor>>, expected: Vec<(&str, &str)>) {
    let test = expected
        .iter()
        .map(|(scope, vehicle)| (scope.to_string(), get_actor(fleet, vehicle)))
        .collect::<HashMap<_, _>>();

    assert_eq!(original.len(), test.len());
    assert!(original.keys().all(|scope| test[scope] == original[scope]));
}

parameterized_test! {can_accept_insertion, (routes, precedence, expected), {
    can_accept_insertion_impl(routes, precedence, expected);
}}

can_accept_insertion! {
    case_01: (
        vec![("v1", vec![None])],
        Some(("s1", 1)),
        vec![("s1", "v1")]
    ),
    case_02: (
        vec![("v1", vec![None]), ("v2", vec![Some(("s2", 1))])],
        Some(("s1", 1)),
        vec![("s1", "v1"), ("s2", "v2")]
    ),
    case_03: (
        vec![("v1", vec![Some(("s1", 1))])],
        Some(("s1", 2)),
        vec![("s1", "v1")]
    ),
}

fn can_accept_insertion_impl(
    routes: Vec<(&str, Vec<Option<Precedence>>)>,
    precedence: Option<Precedence>,
    expected: Vec<(&str, &str)>,
) {
    let fleet = create_test_fleet();

    let state = create_test_precedence_feature().state.unwrap();

    let mut solution_ctx = create_test_solution_context(&fleet, routes);

    state.accept_solution_state(&mut solution_ctx);
    state.accept_insertion(&mut solution_ctx, 0, &Job::Single(create_test_single(precedence)));

    compare_actor_scopes(&fleet, get_actor_scopes(&solution_ctx), expected);
}

parameterized_test! {can_accept_solution_state, (routes, expected), {
    can_accept_solution_state_impl(routes, expected);
}}

can_accept_solution_state! {
    case_01: (
        vec![("v1", vec![Some(("s1", 1))])],
        vec![("s1", "v1")]
    ),
    case_02: (
        vec![
            ("v1", vec![Some(("s1", 1))]),
            ("v2", vec![Some(("s2", 1))])
        ],
        vec![("s1", "v1"), ("s2", "v2")]
    ),
    case_03: (
        vec![
            ("v1", vec![Some(("s1", 1)), Some(("s2", 2))])
        ],
        vec![("s1", "v1"), ("s2", "v1")]
    ),
    case_04: (
        vec![("v1", vec![None])],
        vec![]
    ),
}

fn can_accept_solution_state_impl(routes: Vec<(&str, Vec<Option<Precedence>>)>, expected: Vec<(&str, &str)>) {
    let fleet = create_test_fleet();

    let state = create_test_precedence_feature().state.unwrap();

    let mut solution_ctx = create_test_solution_context(&fleet, routes);

    state.accept_solution_state(&mut solution_ctx);

    compare_actor_scopes(&fleet, get_actor_scopes(&solution_ctx), expected);
}

parameterized_test! {can_evaluate_job, (routes, route_idx, precedence, expected), {
    can_evaluate_job_impl(routes, route_idx, precedence, expected);
}}

can_evaluate_job! {
    case_01: (
        vec![
            ("v1", vec![]),
            ("v2", vec![Some(("s1", 1))])
        ],
        0,
        Some(("s1", 2)),
        Some(VIOLATION_CODE)
    ),

    case_02: (
        vec![
            ("v1", vec![]),
            ("v2", vec![Some(("s1", 1))])
        ],
        0,
        Some(("s2", 1)),
        None
    ),

    case_03: (
        vec![
            ("v1", vec![]),
            ("v2", vec![Some(("s1", 1))])
        ],
        0,
        None,
        None
    ),

    case_04: (
        vec![
            ("v1", vec![Some(("s1", 1))]),
            ("v2", vec![])
        ],
        0,
        Some(("s1", 2)),
        None
    ),
}

fn can_evaluate_job_impl(
    routes: Vec<(&str, Vec<Option<Precedence>>)>,
    route_idx: usize,
    precedence: Option<Precedence>,
    expected: Option<ViolationCode>,
) {
    let fleet = create_test_fleet();

    let mut solution_ctx = create_test_solution_context(&fleet, routes);

    let state = create_test_precedence_feature().state.unwrap();

    state.accept_solution_state(&mut solution_ctx);

    let route_ctx = solution_ctx.routes.get(route_idx).unwrap();

    let job = Job::Single(create_test_single(precedence));

    let constraint = create_test_precedence_feature().constraint.unwrap();

    let result = constraint.evaluate(&MoveContext::route(&solution_ctx, route_ctx, &job));

    assert_eq!(result, expected.map(|code| ConstraintViolation { code, stopped: true }));
}

parameterized_test! {can_merge_precedence, (source, candidate, expected), {
    can_merge_precedence_impl(
        Job::Single(create_test_single(source)),
        Job::Single(create_test_single(candidate)),
        expected,
    );
}}

can_merge_precedence! {
    case_01: (
        Some(("s1", 1)),
        Some(("s1", 1)),
        Ok(())
    ),

    case_02: (
        Some(("s1", 1)),
        Some(("s1", 2)),
        Err(VIOLATION_CODE)
    ),

    case_03: (
        Some(("s1", 1)),
        Some(("s2", 1)),
        Err(VIOLATION_CODE)
    ),

    case_04: (
        None,
        Some(("s1", 1)),
        Err(VIOLATION_CODE)
    ),

    case_05: (
        Some(("s1", 1)),
        None,
        Err(VIOLATION_CODE)
    ),

    case_06: (
        None,
        None,
        Ok(())
    ),
}

fn can_merge_precedence_impl(source: Job, candidate: Job, expected: Result<(), ViolationCode>) {
    let constraint = create_test_precedence_feature().constraint.unwrap();

    let result = constraint.merge(source, candidate).map(|_| ());

    assert_eq!(result, expected);
}

parameterized_test! {can_evaluate_activity_order, (jobs, index, target, expected), {
    can_evaluate_activity_order_impl(jobs, index, target, expected);
}}

can_evaluate_activity_order! {
    case_01: (
        vec![Some(("s1", 1)), Some(("s1", 3))],
        1,
        Some(("s1", 2)),
        None
    ),

    case_02: (
        vec![Some(("s1", 1)), Some(("s1", 3))],
        2,
        Some(("s1", 1)),
        Some(true)
    ),

    case_03: (
        vec![Some(("s1", 1)), Some(("s1", 3))],
        0,
        Some(("s1", 3)),
        Some(false)
    ),

    case_04: (
        vec![Some(("s1", 2)), Some(("s1", 2))],
        1,
        Some(("s1", 2)),
        None
    ),

    case_05: (
        vec![Some(("s2", 5)), Some(("s2", 1))],
        1,
        Some(("s1", 3)),
        None
    ),

    case_06: (
        vec![Some(("s1", 1)), Some(("s1", 3))],
        2,
        None,
        None
    ),

    case_07: (
        vec![None, Some(("s1", 2)), None],
        3,
        Some(("s1", 1)),
        Some(true)
    ),

    case_08: (
        vec![],
        0,
        Some(("s1", 1)),
        None
    ),

    case_09: (
        vec![Some(("s1", 5)), Some(("s1", 9))],
        1,
        Some(("s1", 2)),
        Some(true)
    ),

    case_10: (
        vec![Some(("s1", 1)), Some(("s2", 7)), Some(("s1", 3))],
        2,
        Some(("s1", 2)),
        None
    ),
}

fn can_evaluate_activity_order_impl(
    jobs: Vec<Option<Precedence>>,
    index: usize,
    target: Option<Precedence>,
    expected: Option<bool>,
) {
    let fleet = create_test_fleet();
    let routes = vec![("v1", jobs)];
    let solution_ctx = create_test_solution_context(&fleet, routes);
    let route_ctx = solution_ctx.routes.first().unwrap();
    let tour = &route_ctx.route().tour;

    let target_activity = ActivityBuilder::with_location(1).job(Some(create_test_single(target))).build();
    let activity_ctx =
        ActivityContext { index, prev: tour.get(index).unwrap(), target: &target_activity, next: tour.get(index + 1) };

    let constraint = create_test_precedence_feature().constraint.unwrap();

    let result = constraint.evaluate(&MoveContext::activity(&solution_ctx, route_ctx, &activity_ctx));

    assert_eq!(result, expected.map(|stopped| ConstraintViolation { code: VIOLATION_CODE, stopped }));
}
