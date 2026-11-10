use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::check::{Check, ElementKind, Site};
use super::diff::{self, Diffable};
use super::{Problem, Typed};

/// A data-model record as submitted. Unknown keys are collected rather than refused by serde, so
/// that validation can report each of them with its path alongside every other problem.
#[derive(Debug, Deserialize)]
pub struct DataModelSubmission {
    entities: Vec<EntitySubmission>,
    #[serde(flatten)]
    unknown: Map<String, Value>,
}

#[derive(Debug, Deserialize)]
struct EntitySubmission {
    id: Option<String>,
    name: Option<String>,
    description: Option<String>,
    #[serde(default)]
    fields: Vec<FieldSubmission>,
    #[serde(default)]
    relations: Vec<RelationSubmission>,
    #[serde(flatten)]
    unknown: Map<String, Value>,
}

#[derive(Debug, Deserialize)]
struct FieldSubmission {
    id: Option<String>,
    name: Option<String>,
    r#type: Option<String>,
    nullable: Option<bool>,
    description: Option<String>,
    #[serde(flatten)]
    unknown: Map<String, Value>,
}

#[derive(Debug, Deserialize)]
struct RelationSubmission {
    id: Option<String>,
    target: Option<String>,
    cardinality: Option<String>,
    name: Option<String>,
    #[serde(flatten)]
    unknown: Map<String, Value>,
}

/// The canonical stored form: every element has an id and every relation targets an entity id.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct DataModelRecord {
    pub(super) entities: Vec<Entity>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub(super) struct Entity {
    pub(super) id: String,
    pub(super) name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) description: Option<String>,
    pub(super) fields: Vec<Field>,
    pub(super) relations: Vec<Relation>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub(super) struct Field {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) r#type: String,
    pub(super) nullable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub(super) struct Relation {
    pub(super) id: String,
    pub(super) target: String,
    pub(super) cardinality: Cardinality,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) name: Option<String>,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(super) enum Cardinality {
    One,
    Many,
}

impl Diffable for DataModelRecord {
    fn elements(&self) -> Vec<diff::Element> {
        let mut elements = Vec::new();
        for entity in &self.entities {
            elements.push(ElementKind::Entity.for_diff(
                &entity.id,
                [
                    ("name", json!(entity.name)),
                    ("description", json!(entity.description)),
                ],
            ));
            elements.extend(entity.fields.iter().map(|field| {
                ElementKind::Field.for_diff(
                    &field.id,
                    [
                        ("parent", json!(entity.id)),
                        ("name", json!(field.name)),
                        ("type", json!(field.r#type)),
                        ("nullable", json!(field.nullable)),
                        ("description", json!(field.description)),
                    ],
                )
            }));
            elements.extend(entity.relations.iter().map(|relation| {
                ElementKind::Relation.for_diff(
                    &relation.id,
                    [
                        ("parent", json!(entity.id)),
                        ("target", json!(relation.target)),
                        ("cardinality", json!(relation.cardinality)),
                        ("name", json!(relation.name)),
                    ],
                )
            }));
        }
        elements
    }

    fn ids(&self) -> Vec<(String, &'static str)> {
        let mut ids = Vec::new();
        for entity in &self.entities {
            ids.push((entity.id.clone(), ElementKind::Entity.noun()));
            ids.extend(
                entity
                    .fields
                    .iter()
                    .map(|field| (field.id.clone(), ElementKind::Field.noun())),
            );
            ids.extend(
                entity
                    .relations
                    .iter()
                    .map(|relation| (relation.id.clone(), ElementKind::Relation.noun())),
            );
        }
        ids
    }
}

impl Typed for DataModelRecord {
    type Submission = DataModelSubmission;

    fn validate(
        submission: &DataModelSubmission,
        previous: Option<&Self>,
    ) -> Result<Self, Vec<Problem>> {
        let mut check = Check::new(previous);
        check.unknown_record_keys(&submission.unknown, "a data-model record");

        // Entity ids come first, so that a relation can target an entity listed after it.
        let entity_ids: Vec<String> = submission
            .entities
            .iter()
            .map(|entity| ElementKind::Entity.assign(entity.id.as_deref()))
            .collect();
        let targets: HashMap<&str, &str> = submission
            .entities
            .iter()
            .zip(&entity_ids)
            .filter_map(|(entity, id)| {
                let key = entity.id.as_deref().or(entity.name.as_deref())?;
                Some((key, id.as_str()))
            })
            .collect();

        let mut names = HashSet::new();
        let entities = submission
            .entities
            .iter()
            .zip(&entity_ids)
            .enumerate()
            .map(|(index, (entity, id))| {
                let path = format!("/entities/{index}");
                entity.validate(&mut check, &mut names, &path, id, &targets)
            })
            .collect();
        check.finish(DataModelRecord { entities })
    }
}

impl EntitySubmission {
    fn validate<'a>(
        &'a self,
        check: &mut Check<'a>,
        siblings: &mut HashSet<&'a str>,
        path: &str,
        id: &str,
        targets: &HashMap<&str, &str>,
    ) -> Entity {
        let supplied = self.id.as_deref();
        let site = Site {
            element: ElementKind::Entity,
            path,
            id: supplied,
        };
        check.id(&site);
        let name = check.name(&site, siblings, self.name.as_deref());
        check.unknown_keys(&site, &self.unknown);

        let mut names = HashSet::new();
        let fields = self
            .fields
            .iter()
            .enumerate()
            .map(|(index, field)| {
                field.validate(check, &mut names, &format!("{path}/fields/{index}"))
            })
            .collect();
        let mut names = HashSet::new();
        let relations = self
            .relations
            .iter()
            .enumerate()
            .filter_map(|(index, relation)| {
                let path = format!("{path}/relations/{index}");
                relation.validate(check, &mut names, &path, targets)
            })
            .collect();
        Entity {
            id: id.to_owned(),
            name,
            description: self.description.clone(),
            fields,
            relations,
        }
    }
}

impl FieldSubmission {
    fn validate<'a>(
        &'a self,
        check: &mut Check<'a>,
        siblings: &mut HashSet<&'a str>,
        path: &str,
    ) -> Field {
        let supplied = self.id.as_deref();
        let site = Site {
            element: ElementKind::Field,
            path,
            id: supplied,
        };
        check.id(&site);
        let name = check.name(&site, siblings, self.name.as_deref());
        let r#type = check.required(&site, "type", "missing-type", self.r#type.as_deref());
        check.unknown_keys(&site, &self.unknown);
        Field {
            id: ElementKind::Field.assign(supplied),
            name,
            r#type,
            nullable: self.nullable.unwrap_or(false),
            description: self.description.clone(),
        }
    }
}

impl RelationSubmission {
    /// Returns `None` when the target or the cardinality is invalid; `check` holds the problem.
    fn validate<'a>(
        &'a self,
        check: &mut Check<'a>,
        siblings: &mut HashSet<&'a str>,
        path: &str,
        targets: &HashMap<&str, &str>,
    ) -> Option<Relation> {
        let supplied = self.id.as_deref();
        let site = Site {
            element: ElementKind::Relation,
            path,
            id: supplied,
        };
        check.id(&site);
        let target = match self.target.as_deref() {
            Some(target) => targets.get(target).copied().ok_or_else(|| {
                format!(
                    "`{target}` names no entity in this submission; target an entity by its id, \
                     or by its name when it has no id"
                )
            }),
            None => Err("relation has no target".to_owned()),
        };
        let target = target
            .map_err(|message| check.report(&site, path, "unknown-target", message))
            .ok();
        let cardinality = match self.cardinality.as_deref() {
            Some("one") => Ok(Cardinality::One),
            Some("many") => Ok(Cardinality::Many),
            Some(other) => Err(format!("cardinality `{other}` is not `one` or `many`")),
            None => Err("relation has no cardinality; use `one` or `many`".to_owned()),
        };
        let cardinality = cardinality
            .map_err(|message| check.report(&site, path, "invalid-cardinality", message))
            .ok();
        if let Some(name) = self.name.as_deref().filter(|name| !name.is_empty()) {
            check.unique(&site, siblings, name);
        }
        check.unknown_keys(&site, &self.unknown);
        Some(Relation {
            id: ElementKind::Relation.assign(supplied),
            target: target?.to_owned(),
            cardinality: cardinality?,
            name: self.name.clone(),
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn submission(value: Value) -> DataModelSubmission {
        serde_json::from_value(value).unwrap()
    }

    fn example() -> DataModelSubmission {
        submission(json!({
            "entities": [
                {
                    "name": "Order",
                    "description": "A customer order",
                    "fields": [
                        {"name": "id", "type": "uuid", "nullable": false},
                        {"name": "total", "type": "decimal", "nullable": false}
                    ],
                    "relations": [{"target": "Customer", "cardinality": "one", "name": "customer"}]
                },
                {"name": "Customer", "fields": [{"name": "id", "type": "uuid", "nullable": false}]}
            ]
        }))
    }

    fn problems(value: Value, previous: Option<&DataModelRecord>) -> Vec<(String, &'static str)> {
        DataModelRecord::validate(&submission(value), previous)
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

    #[test]
    fn ids_are_assigned_by_element_kind() {
        let record = DataModelRecord::validate(&example(), None).unwrap();
        let order = &record.entities[0];
        let ids = [
            (&order.id, 'e'),
            (&order.fields[0].id, 'f'),
            (&order.fields[1].id, 'f'),
            (&order.relations[0].id, 'r'),
            (&record.entities[1].id, 'e'),
            (&record.entities[1].fields[0].id, 'f'),
        ];
        for (id, prefix) in ids {
            assert!(id.starts_with(prefix) && id.len() == 12, "{id}");
        }
        let mut distinct: Vec<&String> = ids.iter().map(|(id, _)| *id).collect();
        distinct.sort();
        distinct.dedup();
        assert_eq!(distinct.len(), ids.len());
    }

    #[test]
    fn relation_target_by_name_resolves_to_the_entity_id() {
        let record = DataModelRecord::validate(&example(), None).unwrap();
        assert_eq!(
            record.entities[0].relations[0].target,
            record.entities[1].id
        );
    }

    #[test]
    fn record_serializes_in_struct_order_with_defaults() {
        let record = DataModelRecord::validate(
            &submission(json!({
                "entities": [{
                    "name": "Order",
                    "fields": [{"type": "uuid", "name": "id"}],
                    "relations": [{"cardinality": "many", "target": "Order"}]
                }]
            })),
            None,
        )
        .unwrap();
        let entity = &record.entities[0];
        let expected = json!({
            "entities": [{
                "id": entity.id,
                "name": "Order",
                "fields": [{"id": entity.fields[0].id, "name": "id", "type": "uuid", "nullable": false}],
                "relations": [{"id": entity.relations[0].id, "target": entity.id, "cardinality": "many"}]
            }]
        });
        assert_eq!(
            serde_json::to_string(&record).unwrap(),
            expected.to_string()
        );
    }

    #[test]
    fn every_problem_is_reported_with_its_path() {
        let problems = problems(
            json!({
                "entities": [
                    {
                        "name": "Order",
                        "fields": [
                            {"name": "id"},
                            {"name": " ", "type": "uuid"},
                            {"name": "id", "type": "uuid"}
                        ],
                        "relations": [
                            {"target": "Nowhere", "cardinality": "one"},
                            {"target": "Order", "cardinality": "lots"},
                            {"target": "Order"},
                            {"cardinality": "one"},
                            {"target": "Order", "cardinality": "one", "name": "self"},
                            {"target": "Order", "cardinality": "many", "name": "self"}
                        ]
                    },
                    {"name": "Order"},
                    {}
                ]
            }),
            None,
        );
        assert_eq!(
            problems,
            pairs(&[
                ("/entities/0/fields/0", "missing-type"),
                ("/entities/0/fields/1", "missing-name"),
                ("/entities/0/fields/2", "duplicate-name"),
                ("/entities/0/relations/0", "unknown-target"),
                ("/entities/0/relations/1", "invalid-cardinality"),
                ("/entities/0/relations/2", "invalid-cardinality"),
                ("/entities/0/relations/3", "unknown-target"),
                ("/entities/0/relations/5", "duplicate-name"),
                ("/entities/1", "duplicate-name"),
                ("/entities/2", "missing-name"),
            ])
        );
    }

    #[test]
    fn unknown_keys_are_reported_at_every_level() {
        let problems = problems(
            json!({
                "entities": [{
                    "name": "Order",
                    "colour": "red",
                    "fields": [{"name": "id", "type": "uuid", "nullabel": true}],
                    "relations": [{"target": "Order", "cardinality": "one", "a/b~c": 1}]
                }],
                "version": 2
            }),
            None,
        );
        assert_eq!(
            problems,
            pairs(&[
                ("/version", "unknown-key"),
                ("/entities/0/colour", "unknown-key"),
                ("/entities/0/fields/0/nullabel", "unknown-key"),
                ("/entities/0/relations/0/a~1b~0c", "unknown-key"),
            ])
        );
    }

    #[test]
    fn any_id_on_a_first_submission_is_unknown() {
        let problems = DataModelRecord::validate(
            &submission(json!({
                "entities": [{"id": "eaaaaaaaaaaa", "name": "Order"}]
            })),
            None,
        )
        .unwrap_err();
        assert_eq!(problems.len(), 1);
        assert_eq!(problems[0].path, "/entities/0");
        assert_eq!(problems[0].code, "unknown-id");
        assert_eq!(problems[0].id.as_deref(), Some("eaaaaaaaaaaa"));
    }

    #[test]
    fn ids_agree_with_the_ids_of_elements() {
        let record = DataModelRecord::validate(&example(), None).unwrap();
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
        let first = DataModelRecord::validate(&example(), None).unwrap();
        let order = &first.entities[0];
        let next = json!({
            "entities": [{
                "id": order.id,
                "name": "PurchaseOrder",
                "fields": [{"id": order.fields[0].id, "name": "id", "type": "uuid"}],
                "relations": [{"id": order.relations[0].id, "target": order.id, "cardinality": "many"}]
            }]
        });

        let record = DataModelRecord::validate(&submission(next), Some(&first)).unwrap();
        let entity = &record.entities[0];
        assert_eq!(entity.id, order.id);
        assert_eq!(entity.fields[0].id, order.fields[0].id);
        assert_eq!(entity.relations[0].id, order.relations[0].id);
        assert_eq!(entity.relations[0].target, order.id);
    }

    #[test]
    fn stored_record_reads_back_equal() {
        let record = DataModelRecord::validate(&example(), None).unwrap();
        let text = serde_json::to_string(&record).unwrap();
        assert_eq!(
            serde_json::from_str::<DataModelRecord>(&text).unwrap(),
            record
        );
    }

    #[test]
    fn elements_carry_their_parent_and_compared_attributes() {
        let record = DataModelRecord::validate(&example(), None).unwrap();
        let order = &record.entities[0];
        let elements: Vec<(&str, String, Value)> = record
            .elements()
            .into_iter()
            .map(|element| {
                let attributes: Map<String, Value> = element
                    .attributes
                    .into_iter()
                    .map(|(name, value)| (name.to_owned(), value))
                    .collect();
                (element.kind, element.id, Value::Object(attributes))
            })
            .collect();
        let kinds: Vec<&str> = elements.iter().map(|(kind, _, _)| *kind).collect();
        assert_eq!(
            kinds,
            ["entity", "field", "field", "relation", "entity", "field"]
        );
        assert_eq!(
            elements[0],
            (
                "entity",
                order.id.clone(),
                json!({"name": "Order", "description": "A customer order"})
            )
        );
        assert_eq!(
            elements[2].2,
            json!({"parent": order.id, "name": "total", "type": "decimal", "nullable": false, "description": null})
        );
        assert_eq!(
            elements[3].2,
            json!({"parent": order.id, "target": record.entities[1].id, "cardinality": "one", "name": "customer"})
        );
    }

    /// Validates the stored form of the example, changed by `edit`, as its next version, and
    /// returns both versions.
    fn next_version(edit: impl FnOnce(&mut Value)) -> (DataModelRecord, DataModelRecord) {
        let first = DataModelRecord::validate(&example(), None).unwrap();
        let mut value = serde_json::to_value(&first).unwrap();
        edit(&mut value);
        let next = DataModelRecord::validate(&submission(value), Some(&first)).unwrap();
        (first, next)
    }

    #[test]
    fn diff_of_a_kept_id_with_a_new_name_is_one_change() {
        let (first, next) = next_version(|value| {
            value["entities"][0]["name"] = json!("PurchaseOrder");
        });
        let changes = diff::diff(&first, &next);
        assert!(changes.added.is_empty() && changes.removed.is_empty());
        assert_eq!(changes.changed.len(), 1);
        assert_eq!(changes.changed[0].item.kind, "entity");
        assert_eq!(changes.changed[0].item.id, first.entities[0].id);
        assert_eq!(changes.changed[0].attributes, ["name"]);
    }

    #[test]
    fn diff_of_a_nullability_change_lists_only_that_field() {
        let (first, next) = next_version(|value| {
            value["entities"][0]["fields"][1]["nullable"] = json!(true);
        });
        let changes = diff::diff(&first, &next);
        assert!(changes.added.is_empty() && changes.removed.is_empty());
        assert_eq!(changes.changed.len(), 1);
        assert_eq!(changes.changed[0].item.kind, "field");
        assert_eq!(changes.changed[0].item.id, first.entities[0].fields[1].id);
        assert_eq!(changes.changed[0].attributes, ["nullable"]);
    }

    #[test]
    fn diff_of_a_resubmission_without_ids_replaces_every_element() {
        let first = DataModelRecord::validate(&example(), None).unwrap();
        let next = DataModelRecord::validate(&example(), Some(&first)).unwrap();
        let changes = diff::diff(&first, &next);
        let ids = |record: &DataModelRecord| -> Vec<String> {
            record
                .elements()
                .into_iter()
                .map(|element| element.id)
                .collect()
        };
        let item_ids = |items: &[diff::Item]| -> Vec<String> {
            items.iter().map(|item| item.id.clone()).collect()
        };
        assert_eq!(item_ids(&changes.removed), ids(&first));
        assert_eq!(item_ids(&changes.added), ids(&next));
        assert!(changes.changed.is_empty());
    }

    #[test]
    fn diff_of_a_moved_field_and_a_retargeted_relation() {
        let (first, next) = next_version(|value| {
            let total = value["entities"][0]["fields"]
                .as_array_mut()
                .unwrap()
                .remove(1);
            value["entities"][1]["fields"]
                .as_array_mut()
                .unwrap()
                .push(total);
            let order = value["entities"][0]["id"].clone();
            let relation = &mut value["entities"][0]["relations"][0];
            relation["target"] = order;
            relation["cardinality"] = json!("many");
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
                ("relation", &["cardinality", "target"][..]),
                ("field", &["parent"][..])
            ]
        );
    }

    #[test]
    fn ids_of_another_kind_or_used_twice_are_rejected() {
        let first = DataModelRecord::validate(&example(), None).unwrap();
        let order = &first.entities[0];
        let customer = &first.entities[1];
        let next = json!({
            "entities": [
                {"id": order.fields[0].id, "name": "Order"},
                {"id": customer.id, "name": "Customer"},
                {"id": customer.id, "name": "Client"},
                {"name": "Invoice", "relations": [{"target": "Customer", "cardinality": "one"}]}
            ]
        });
        assert_eq!(
            problems(next, Some(&first)),
            pairs(&[
                ("/entities/0", "unknown-id"),
                ("/entities/2", "duplicate-id"),
                ("/entities/3/relations/0", "unknown-target"),
            ])
        );
    }
}
