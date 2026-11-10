use std::collections::{HashMap, HashSet};

use serde::Serialize;
use serde_json::Value;

/// A typed record seen as a flat list of elements, so that records of every kind compare alike.
pub trait Diffable {
    /// Every element in document order, each element before its children.
    fn elements(&self) -> Vec<Element>;

    /// The id and kind of every element, in the order of `elements`.
    fn ids(&self) -> Vec<(String, &'static str)> {
        self.elements()
            .into_iter()
            .map(|element| (element.id, element.kind))
            .collect()
    }
}

/// One element of a record. `attributes` holds every compared attribute, `name` and `parent`
/// included.
#[derive(Clone, Debug)]
pub struct Element {
    pub kind: &'static str,
    pub id: String,
    pub attributes: Vec<(&'static str, Value)>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Item {
    pub kind: &'static str,
    pub id: String,
    pub name: Option<String>,
}

#[derive(Debug, PartialEq, Serialize)]
pub struct Change {
    #[serde(flatten)]
    pub item: Item,
    pub attributes: Vec<&'static str>,
}

#[derive(Debug, Default, PartialEq, Serialize)]
pub struct Diff {
    pub added: Vec<Item>,
    pub removed: Vec<Item>,
    pub changed: Vec<Change>,
}

/// Compares two versions element by element, keyed by id. `removed` follows the document order
/// of `from`; `added` and `changed` follow `to`.
pub fn diff(from: &impl Diffable, to: &impl Diffable) -> Diff {
    let from = from.elements();
    let to = to.elements();
    let older: HashMap<&str, &Element> = from
        .iter()
        .map(|element| (element.id.as_str(), element))
        .collect();
    let newer: HashSet<&str> = to.iter().map(|element| element.id.as_str()).collect();

    let mut diff = Diff {
        removed: from
            .iter()
            .filter(|element| !newer.contains(element.id.as_str()))
            .map(Element::item)
            .collect(),
        ..Diff::default()
    };
    for element in &to {
        let Some(old) = older.get(element.id.as_str()) else {
            diff.added.push(element.item());
            continue;
        };
        let mut attributes: Vec<&'static str> = element
            .attributes
            .iter()
            .filter(|(name, value)| old.attribute(name) != Some(value))
            .map(|(name, _)| *name)
            .collect();
        if !attributes.is_empty() {
            attributes.sort_unstable();
            diff.changed.push(Change {
                item: element.item(),
                attributes,
            });
        }
    }
    diff
}

impl Element {
    fn attribute(&self, name: &str) -> Option<&Value> {
        self.attributes
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value)
    }

    fn item(&self) -> Item {
        Item {
            kind: self.kind,
            id: self.id.clone(),
            name: self
                .attribute("name")
                .and_then(Value::as_str)
                .map(str::to_owned),
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    struct Elements(Vec<Element>);

    impl Diffable for Elements {
        fn elements(&self) -> Vec<Element> {
            self.0.clone()
        }
    }

    fn element(kind: &'static str, id: &str, attributes: &[(&'static str, Value)]) -> Element {
        Element {
            kind,
            id: id.to_owned(),
            attributes: attributes.to_vec(),
        }
    }

    fn entity(id: &str, name: &str, description: Value) -> Element {
        element(
            "entity",
            id,
            &[("name", json!(name)), ("description", description)],
        )
    }

    fn field(id: &str, parent: &str, nullable: bool) -> Element {
        element(
            "field",
            id,
            &[
                ("parent", json!(parent)),
                ("name", json!(id)),
                ("nullable", json!(nullable)),
            ],
        )
    }

    fn item(kind: &'static str, id: &str, name: Option<&str>) -> Item {
        Item {
            kind,
            id: id.to_owned(),
            name: name.map(str::to_owned),
        }
    }

    fn ids(items: &[Item]) -> Vec<&str> {
        items.iter().map(|item| item.id.as_str()).collect()
    }

    #[test]
    fn identical_versions_have_no_differences() {
        let version = Elements(vec![
            entity("e1", "Order", Value::Null),
            field("f1", "e1", false),
        ]);
        assert_eq!(diff(&version, &version), Diff::default());
    }

    #[test]
    fn elements_are_matched_by_id_not_by_name() {
        let from = Elements(vec![entity("e1", "Order", Value::Null)]);
        let to = Elements(vec![entity("e2", "Order", Value::Null)]);
        assert_eq!(
            diff(&from, &to),
            Diff {
                added: vec![item("entity", "e2", Some("Order"))],
                removed: vec![item("entity", "e1", Some("Order"))],
                changed: vec![],
            }
        );
    }

    #[test]
    fn changed_attributes_are_sorted_and_named_from_the_newer_version() {
        let from = Elements(vec![entity("e1", "Order", Value::Null)]);
        let to = Elements(vec![entity("e1", "PurchaseOrder", json!("A purchase"))]);
        assert_eq!(
            diff(&from, &to).changed,
            [Change {
                item: item("entity", "e1", Some("PurchaseOrder")),
                attributes: vec!["description", "name"],
            }]
        );
    }

    #[test]
    fn items_keep_the_document_order_of_their_version() {
        let from = Elements(vec![
            field("f1", "e1", false),
            field("f2", "e1", false),
            field("f3", "e1", false),
            field("f4", "e1", false),
        ]);
        let to = Elements(vec![
            field("f6", "e1", false),
            field("f4", "e1", true),
            field("f5", "e1", false),
            field("f3", "e2", false),
        ]);
        let diff = diff(&from, &to);
        assert_eq!(ids(&diff.removed), ["f1", "f2"]);
        assert_eq!(ids(&diff.added), ["f6", "f5"]);
        let changed: Vec<(&str, &[&str])> = diff
            .changed
            .iter()
            .map(|change| (change.item.id.as_str(), change.attributes.as_slice()))
            .collect();
        assert_eq!(
            changed,
            [("f4", &["nullable"][..]), ("f3", &["parent"][..])]
        );
    }

    #[test]
    fn items_serialize_with_a_null_name_when_unnamed() {
        let from = Elements(vec![]);
        let to = Elements(vec![element("relation", "r1", &[("name", Value::Null)])]);
        assert_eq!(
            serde_json::to_value(diff(&from, &to)).unwrap(),
            json!({
                "added": [{"kind": "relation", "id": "r1", "name": null}],
                "removed": [],
                "changed": []
            })
        );
        let change = Change {
            item: item("entity", "e1", Some("Order")),
            attributes: vec!["name"],
        };
        assert_eq!(
            serde_json::to_value(&change).unwrap(),
            json!({"kind": "entity", "id": "e1", "name": "Order", "attributes": ["name"]})
        );
    }
}
