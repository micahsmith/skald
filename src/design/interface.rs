use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::check::{Check, ElementKind, Site};
use super::diff::{self, Diffable};
use super::{Problem, Typed};

/// An interface record as submitted. Unknown keys are collected rather than refused by serde, so
/// that validation can report each of them with its path alongside every other problem.
#[derive(Debug, Deserialize)]
pub struct InterfaceSubmission {
    operations: Vec<OperationSubmission>,
    #[serde(flatten)]
    unknown: Map<String, Value>,
}

#[derive(Debug, Deserialize)]
struct OperationSubmission {
    id: Option<String>,
    name: Option<String>,
    description: Option<String>,
    #[serde(default)]
    inputs: Vec<ParameterSubmission>,
    #[serde(default)]
    outputs: Vec<ParameterSubmission>,
    #[serde(default)]
    errors: Vec<DeclaredErrorSubmission>,
    #[serde(flatten)]
    unknown: Map<String, Value>,
}

#[derive(Debug, Deserialize)]
struct ParameterSubmission {
    id: Option<String>,
    name: Option<String>,
    r#type: Option<String>,
    nullable: Option<bool>,
    #[serde(flatten)]
    unknown: Map<String, Value>,
}

#[derive(Debug, Deserialize)]
struct DeclaredErrorSubmission {
    id: Option<String>,
    name: Option<String>,
    condition: Option<String>,
    #[serde(flatten)]
    unknown: Map<String, Value>,
}

/// The canonical stored form, in which every element has an id.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct InterfaceRecord {
    pub(super) operations: Vec<Operation>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub(super) struct Operation {
    pub(super) id: String,
    pub(super) name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) description: Option<String>,
    pub(super) inputs: Vec<Parameter>,
    pub(super) outputs: Vec<Parameter>,
    pub(super) errors: Vec<DeclaredError>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub(super) struct Parameter {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) r#type: String,
    pub(super) nullable: bool,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub(super) struct DeclaredError {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) condition: String,
}

impl Diffable for InterfaceRecord {
    fn elements(&self) -> Vec<diff::Element> {
        let mut elements = Vec::new();
        for operation in &self.operations {
            elements.push(ElementKind::Operation.for_diff(
                &operation.id,
                [
                    ("name", json!(operation.name)),
                    ("description", json!(operation.description)),
                ],
            ));
            for (element, parameters) in [
                (ElementKind::Input, &operation.inputs),
                (ElementKind::Output, &operation.outputs),
            ] {
                elements.extend(parameters.iter().map(|parameter| {
                    element.for_diff(
                        &parameter.id,
                        [
                            ("parent", json!(operation.id)),
                            ("name", json!(parameter.name)),
                            ("type", json!(parameter.r#type)),
                            ("nullable", json!(parameter.nullable)),
                        ],
                    )
                }));
            }
            elements.extend(operation.errors.iter().map(|error| {
                ElementKind::Error.for_diff(
                    &error.id,
                    [
                        ("parent", json!(operation.id)),
                        ("name", json!(error.name)),
                        ("condition", json!(error.condition)),
                    ],
                )
            }));
        }
        elements
    }

    fn ids(&self) -> Vec<(String, &'static str)> {
        let mut ids = Vec::new();
        for operation in &self.operations {
            ids.push((operation.id.clone(), ElementKind::Operation.noun()));
            for (element, parameters) in [
                (ElementKind::Input, &operation.inputs),
                (ElementKind::Output, &operation.outputs),
            ] {
                ids.extend(
                    parameters
                        .iter()
                        .map(|parameter| (parameter.id.clone(), element.noun())),
                );
            }
            ids.extend(
                operation
                    .errors
                    .iter()
                    .map(|error| (error.id.clone(), ElementKind::Error.noun())),
            );
        }
        ids
    }
}

impl Typed for InterfaceRecord {
    type Submission = InterfaceSubmission;

    fn validate(
        submission: &InterfaceSubmission,
        previous: Option<&Self>,
    ) -> Result<Self, Vec<Problem>> {
        let mut check = Check::new(previous);
        check.unknown_record_keys(&submission.unknown, "an interface record");
        let mut names = HashSet::new();
        let operations = submission
            .operations
            .iter()
            .enumerate()
            .map(|(index, operation)| {
                operation.validate(&mut check, &mut names, &format!("/operations/{index}"))
            })
            .collect();
        check.finish(InterfaceRecord { operations })
    }
}

impl OperationSubmission {
    fn validate<'a>(
        &'a self,
        check: &mut Check<'a>,
        siblings: &mut HashSet<&'a str>,
        path: &str,
    ) -> Operation {
        let supplied = self.id.as_deref();
        let site = Site {
            element: ElementKind::Operation,
            path,
            id: supplied,
        };
        check.id(&site);
        let name = check.name(&site, siblings, self.name.as_deref());
        check.unknown_keys(&site, &self.unknown);

        let inputs = parameters(
            check,
            ElementKind::Input,
            &self.inputs,
            &format!("{path}/inputs"),
        );
        let outputs = parameters(
            check,
            ElementKind::Output,
            &self.outputs,
            &format!("{path}/outputs"),
        );
        let mut names = HashSet::new();
        let errors = self
            .errors
            .iter()
            .enumerate()
            .map(|(index, error)| {
                error.validate(check, &mut names, &format!("{path}/errors/{index}"))
            })
            .collect();
        Operation {
            id: ElementKind::Operation.assign(supplied),
            name,
            description: self.description.clone(),
            inputs,
            outputs,
            errors,
        }
    }
}

/// Validates the inputs or the outputs of one operation, whose names are unique among
/// themselves.
fn parameters<'a>(
    check: &mut Check<'a>,
    element: ElementKind,
    list: &'a [ParameterSubmission],
    path: &str,
) -> Vec<Parameter> {
    let mut names = HashSet::new();
    list.iter()
        .enumerate()
        .map(|(index, parameter)| {
            parameter.validate(check, &mut names, element, &format!("{path}/{index}"))
        })
        .collect()
}

impl ParameterSubmission {
    fn validate<'a>(
        &'a self,
        check: &mut Check<'a>,
        siblings: &mut HashSet<&'a str>,
        element: ElementKind,
        path: &str,
    ) -> Parameter {
        let supplied = self.id.as_deref();
        let site = Site {
            element,
            path,
            id: supplied,
        };
        check.id(&site);
        let name = check.name(&site, siblings, self.name.as_deref());
        let r#type = check.required(&site, "type", "missing-type", self.r#type.as_deref());
        check.unknown_keys(&site, &self.unknown);
        Parameter {
            id: element.assign(supplied),
            name,
            r#type,
            nullable: self.nullable.unwrap_or(false),
        }
    }
}

impl DeclaredErrorSubmission {
    fn validate<'a>(
        &'a self,
        check: &mut Check<'a>,
        siblings: &mut HashSet<&'a str>,
        path: &str,
    ) -> DeclaredError {
        let supplied = self.id.as_deref();
        let site = Site {
            element: ElementKind::Error,
            path,
            id: supplied,
        };
        check.id(&site);
        let name = check.name(&site, siblings, self.name.as_deref());
        let condition = check.required(
            &site,
            "condition",
            "missing-condition",
            self.condition.as_deref(),
        );
        check.unknown_keys(&site, &self.unknown);
        DeclaredError {
            id: ElementKind::Error.assign(supplied),
            name,
            condition,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn submission(value: Value) -> InterfaceSubmission {
        serde_json::from_value(value).unwrap()
    }

    fn example() -> InterfaceSubmission {
        submission(json!({
            "operations": [
                {
                    "name": "createOrder",
                    "description": "Creates an order",
                    "inputs": [{"name": "customer_id", "type": "uuid", "nullable": false}],
                    "outputs": [{"name": "order", "type": "Order", "nullable": false}],
                    "errors": [{"name": "CustomerNotFound", "condition": "no such customer"}]
                },
                {
                    "name": "getOrder",
                    "inputs": [{"name": "order_id", "type": "uuid", "nullable": false}],
                    "outputs": [{"name": "order", "type": "Order", "nullable": false}],
                    "errors": [{"name": "OrderNotFound", "condition": "no such order"}]
                }
            ]
        }))
    }

    fn problems(value: Value, previous: Option<&InterfaceRecord>) -> Vec<(String, &'static str)> {
        InterfaceRecord::validate(&submission(value), previous)
            .unwrap_err()
            .into_iter()
            .map(|problem| (problem.path, problem.code))
            .collect()
    }

    fn pairs(expected: &[(&str, &'static str)]) -> Vec<(String, &'static str)> {
        expected
            .iter()
            .map(|(path, code)| ((*path).to_owned(), *code))
            .collect()
    }

    fn ids(record: &InterfaceRecord) -> Vec<String> {
        record
            .elements()
            .into_iter()
            .map(|element| element.id)
            .collect()
    }

    #[test]
    fn ids_are_assigned_by_element_kind() {
        let record = InterfaceRecord::validate(&example(), None).unwrap();
        let prefixes: String = ids(&record)
            .iter()
            .map(|id| {
                assert_eq!(id.len(), 12, "{id}");
                id.chars().next().unwrap()
            })
            .collect();
        assert_eq!(prefixes, "oppxoppx");
        let mut distinct = ids(&record);
        distinct.sort();
        distinct.dedup();
        assert_eq!(distinct.len(), 8);
    }

    #[test]
    fn record_serializes_in_struct_order_with_defaults() {
        let record = InterfaceRecord::validate(
            &submission(json!({
                "operations": [{
                    "inputs": [{"type": "uuid", "name": "id"}],
                    "name": "getOrder"
                }]
            })),
            None,
        )
        .unwrap();
        let operation = &record.operations[0];
        let expected = json!({
            "operations": [{
                "id": operation.id,
                "name": "getOrder",
                "inputs": [{"id": operation.inputs[0].id, "name": "id", "type": "uuid", "nullable": false}],
                "outputs": [],
                "errors": []
            }]
        });
        assert_eq!(
            serde_json::to_string(&record).unwrap(),
            expected.to_string()
        );
    }

    #[test]
    fn stored_record_reads_back_equal() {
        let record = InterfaceRecord::validate(&example(), None).unwrap();
        let text = serde_json::to_string(&record).unwrap();
        assert_eq!(
            serde_json::from_str::<InterfaceRecord>(&text).unwrap(),
            record
        );
    }

    #[test]
    fn every_problem_is_reported_with_its_path() {
        let problems = problems(
            json!({
                "operations": [
                    {
                        "name": "getOrder",
                        "inputs": [
                            {"name": "id"},
                            {"name": "id", "type": "uuid"},
                            {"type": "uuid"}
                        ],
                        "outputs": [
                            {"name": "id", "type": "uuid"},
                            {"name": "id", "type": " "}
                        ],
                        "errors": [
                            {"name": "NotFound"},
                            {"name": "NotFound", "condition": "no such order"},
                            {"condition": "the order was archived"}
                        ]
                    },
                    {"name": "getOrder"},
                    {}
                ]
            }),
            None,
        );
        assert_eq!(
            problems,
            pairs(&[
                ("/operations/0/inputs/0", "missing-type"),
                ("/operations/0/inputs/1", "duplicate-name"),
                ("/operations/0/inputs/2", "missing-name"),
                ("/operations/0/outputs/1", "duplicate-name"),
                ("/operations/0/outputs/1", "missing-type"),
                ("/operations/0/errors/0", "missing-condition"),
                ("/operations/0/errors/1", "duplicate-name"),
                ("/operations/0/errors/2", "missing-name"),
                ("/operations/1", "duplicate-name"),
                ("/operations/2", "missing-name"),
            ])
        );
    }

    #[test]
    fn unknown_keys_are_reported_at_every_level() {
        let problems = problems(
            json!({
                "operations": [{
                    "name": "getOrder",
                    "verb": "GET",
                    "inputs": [{"name": "id", "type": "uuid", "nullabel": true}],
                    "outputs": [{"name": "order", "type": "Order", "description": "the order"}],
                    "errors": [{"name": "NotFound", "condition": "no such order", "status": 404}]
                }],
                "entities": []
            }),
            None,
        );
        assert_eq!(
            problems,
            pairs(&[
                ("/entities", "unknown-key"),
                ("/operations/0/verb", "unknown-key"),
                ("/operations/0/inputs/0/nullabel", "unknown-key"),
                ("/operations/0/outputs/0/description", "unknown-key"),
                ("/operations/0/errors/0/status", "unknown-key"),
            ])
        );
    }

    #[test]
    fn any_id_on_a_first_submission_is_unknown() {
        let problems = problems(
            json!({"operations": [{"id": "oaaaaaaaaaaa", "name": "getOrder"}]}),
            None,
        );
        assert_eq!(problems, pairs(&[("/operations/0", "unknown-id")]));
    }

    #[test]
    fn ids_agree_with_the_ids_of_elements() {
        let record = InterfaceRecord::validate(&example(), None).unwrap();
        let expected: Vec<_> = record
            .elements()
            .into_iter()
            .map(|element| (element.id, element.kind))
            .collect();
        assert!(!expected.is_empty());
        assert_eq!(record.ids(), expected);
    }

    #[test]
    fn ids_of_the_previous_version_are_kept() {
        let first = InterfaceRecord::validate(&example(), None).unwrap();
        let value = serde_json::to_value(&first).unwrap();
        let next = InterfaceRecord::validate(&submission(value), Some(&first)).unwrap();
        assert_eq!(next, first);
    }

    #[test]
    fn ids_of_another_kind_or_used_twice_are_rejected() {
        let first = InterfaceRecord::validate(&example(), None).unwrap();
        let create = &first.operations[0];
        let next = json!({
            "operations": [
                {
                    "id": create.id,
                    "name": "createOrder",
                    "inputs": [{"id": create.outputs[0].id, "name": "order", "type": "Order"}],
                    "errors": [
                        {"id": create.errors[0].id, "name": "CustomerNotFound", "condition": "none"},
                        {"id": create.errors[0].id, "name": "CustomerGone", "condition": "gone"}
                    ]
                },
                {"id": create.inputs[0].id, "name": "getOrder"}
            ]
        });
        assert_eq!(
            problems(next, Some(&first)),
            pairs(&[
                ("/operations/0/inputs/0", "unknown-id"),
                ("/operations/0/errors/1", "duplicate-id"),
                ("/operations/1", "unknown-id"),
            ])
        );
    }

    #[test]
    fn elements_carry_their_parent_and_compared_attributes() {
        let record = InterfaceRecord::validate(&example(), None).unwrap();
        let create = &record.operations[0];
        let elements: Vec<(&str, Value)> = record
            .elements()
            .into_iter()
            .map(|element| {
                let attributes: Map<String, Value> = element
                    .attributes
                    .into_iter()
                    .map(|(name, value)| (name.to_owned(), value))
                    .collect();
                (element.kind, Value::Object(attributes))
            })
            .collect();
        let kinds: Vec<&str> = elements.iter().map(|(kind, _)| *kind).collect();
        assert_eq!(kinds, ["operation", "input", "output", "error"].repeat(2));
        assert_eq!(
            elements[0].1,
            json!({"name": "createOrder", "description": "Creates an order"})
        );
        assert_eq!(
            elements[2].1,
            json!({"parent": create.id, "name": "order", "type": "Order", "nullable": false})
        );
        assert_eq!(
            elements[3].1,
            json!({"parent": create.id, "name": "CustomerNotFound", "condition": "no such customer"})
        );
    }

    /// Validates the stored form of the example, changed by `edit`, as its next version, and
    /// returns both versions.
    fn next_version(edit: impl FnOnce(&mut Value)) -> (InterfaceRecord, InterfaceRecord) {
        let first = InterfaceRecord::validate(&example(), None).unwrap();
        let mut value = serde_json::to_value(&first).unwrap();
        edit(&mut value);
        let next = InterfaceRecord::validate(&submission(value), Some(&first)).unwrap();
        (first, next)
    }

    #[test]
    fn diff_of_an_input_nullability_change_lists_only_that_input() {
        let (first, next) = next_version(|value| {
            value["operations"][1]["inputs"][0]["nullable"] = json!(true);
        });
        let changes = diff::diff(&first, &next);
        assert!(changes.added.is_empty() && changes.removed.is_empty());
        assert_eq!(changes.changed.len(), 1);
        assert_eq!(changes.changed[0].item.kind, "input");
        assert_eq!(changes.changed[0].item.id, first.operations[1].inputs[0].id);
        assert_eq!(changes.changed[0].attributes, ["nullable"]);
    }

    #[test]
    fn diff_of_an_added_error_is_one_addition() {
        let (first, next) = next_version(|value| {
            value["operations"][1]["errors"]
                .as_array_mut()
                .unwrap()
                .push(json!({"name": "OrderArchived", "condition": "archived"}));
        });
        let changes = diff::diff(&first, &next);
        assert!(changes.changed.is_empty() && changes.removed.is_empty());
        assert_eq!(changes.added.len(), 1);
        assert_eq!(changes.added[0].kind, "error");
        assert_eq!(changes.added[0].name.as_deref(), Some("OrderArchived"));
    }

    #[test]
    fn diff_of_a_moved_output_and_a_new_condition() {
        let (first, next) = next_version(|value| {
            let order = value["operations"][0]["outputs"]
                .as_array_mut()
                .unwrap()
                .remove(0);
            value["operations"][1]["outputs"]
                .as_array_mut()
                .unwrap()
                .push(json!({"name": "created", "type": "Order", "id": order["id"]}));
            value["operations"][1]["errors"][0]["condition"] = json!("the order was deleted");
        });
        let changes = diff::diff(&first, &next);
        assert!(changes.added.is_empty() && changes.removed.is_empty());
        let changed: Vec<(&str, &[&str])> = changes
            .changed
            .iter()
            .map(|change| (change.item.kind, change.attributes.as_slice()))
            .collect();
        assert_eq!(
            changed,
            [
                ("output", &["name", "parent"][..]),
                ("error", &["condition"][..])
            ]
        );
    }
}
