use std::collections::HashMap;

use super::data_model::{Cardinality, DataModelRecord, Entity, Relation};
use super::interface::{InterfaceRecord, Operation, Parameter};

const MARGIN: usize = 30;
const GAP: usize = 60;
const BOX_WIDTH: usize = 240;
const TITLE_HEIGHT: usize = 32;
const FIELD_HEIGHT: usize = 20;
const TEXT_INSET: usize = 12;
const NAME_BASELINE: usize = 21;
const FIELD_BASELINE: usize = 14;
const LOOP_WIDTH: f64 = 20.0;
const LOOP_LABEL_GAP: f64 = 10.0;
const LABEL_BACK: f64 = 16.0;
const LABEL_SIDE: f64 = 8.0;
const EDGE_SPACING: f64 = 8.0;
const EDGE_SPREAD: f64 = 28.0;
const LOOP_SPAN: f64 = 24.0;
const LOOP_GAP: f64 = 4.0;
const LOOP_HEIGHT: f64 = 16.0;

const PAGE_STYLE: &str = "\
body { margin: 24px; font: 14px/1.4 system-ui, sans-serif; color: #1f2328; background: #fff; }
h1 { margin: 0 0 16px; font-size: 20px; font-weight: 600; }
h1 span { margin-left: 8px; color: #656d76; font-size: 14px; font-weight: 400; }
.name { font-weight: 600; }
";

const DIAGRAM_STYLE: &str = "\
svg { display: block; max-width: 100%; height: auto; }
rect { fill: #f6f8fa; stroke: #8c959f; }
text { fill: #1f2328; font-size: 13px; }
[data-kind=\"relation\"] { fill: none; stroke: #656d76; stroke-width: 1.5; marker-end: url(#arrow); }
#arrow path { fill: #656d76; }
.cardinality { fill: #656d76; text-anchor: middle; dominant-baseline: central; }
";

const TABLE_STYLE: &str = "\
table { border-collapse: collapse; }
th, td { padding: 6px 12px; border: 1px solid #d0d7de; text-align: left; vertical-align: top; }
th { background: #f6f8fa; font-weight: 600; }
ul { margin: 0; padding: 0; list-style: none; }
li + li { margin-top: 4px; }
.none { color: #656d76; }
";

const COLUMNS: [&str; 5] = ["Operation", "Description", "Inputs", "Outputs", "Errors"];

const ARROW: &str = "<defs><marker id=\"arrow\" viewBox=\"0 0 10 10\" refX=\"10\" refY=\"5\" \
                     markerWidth=\"6\" markerHeight=\"6\" orient=\"auto\">\
                     <path d=\"M 0 0 L 10 5 L 0 10 z\"/></marker></defs>";

/// What a rendered page shows of a design besides one version's record.
pub struct DesignHeader<'a> {
    pub name: &'a str,
    pub kind: &'a str,
}

pub enum Record {
    DataModel(DataModelRecord),
    Interface(InterfaceRecord),
}

/// One standalone HTML page. It holds no timestamp, random value, script, or external
/// reference, so equal inputs give byte-identical pages.
pub fn render(design: &DesignHeader, version: u32, record: &Record) -> String {
    let (style, body) = match record {
        Record::DataModel(model) => (DIAGRAM_STYLE, diagram(model)),
        Record::Interface(interface) => (TABLE_STYLE, table(interface)),
    };
    let name = escape(design.name);
    let kind = escape(design.kind);
    format!(
        "<!doctype html>\n<html lang=\"en\">\n<head>\n<meta charset=\"utf-8\">\n\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\n\
         <title>{name}, version {version}</title>\n<style>\n{PAGE_STYLE}{style}</style>\n</head>\n\
         <body>\n<h1>{name} <span>{kind}, version {version}</span></h1>\n{body}</body>\n</html>\n"
    )
}

fn diagram(model: &DataModelRecord) -> String {
    let entities = &model.entities;
    let columns = columns(entities.len());
    let frames = layout(entities, columns);
    let width = 2 * MARGIN + columns * BOX_WIDTH + columns.saturating_sub(1) * GAP;
    let height = frames.iter().map(Frame::bottom).max().unwrap_or(MARGIN) + MARGIN;
    let mut svg =
        format!("<svg width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n");
    svg.push_str(ARROW);
    svg.push('\n');

    // Edges come first so that boxes cover the edges that cross them.
    let index: HashMap<&str, usize> = entities
        .iter()
        .enumerate()
        .map(|(index, entity)| (entity.id.as_str(), index))
        .collect();
    let key = |source: usize, target: usize| (source.min(target), source.max(target));
    let mut sizes: HashMap<(usize, usize), usize> = HashMap::new();
    for (source, entity) in entities.iter().enumerate() {
        for target in entity
            .relations
            .iter()
            .filter_map(|r| index.get(r.target.as_str()))
        {
            *sizes.entry(key(source, *target)).or_default() += 1;
        }
    }
    let mut seen: HashMap<(usize, usize), usize> = HashMap::new();
    for (source, (entity, from)) in entities.iter().zip(&frames).enumerate() {
        for relation in &entity.relations {
            let Some(&target) = index.get(relation.target.as_str()) else {
                continue;
            };
            let pair = key(source, target);
            let position = seen.entry(pair).or_default();
            let (slot, count) = (*position, sizes[&pair]);
            *position += 1;
            let edge = if target == source {
                Edge::self_loop(from, slot, count)
            } else {
                // The offset is measured from the lower entity index to the higher.
                let side = if source < target { 1.0 } else { -1.0 };
                let spacing = EDGE_SPACING.min(EDGE_SPREAD / (count - 1).max(1) as f64);
                let offset = side * (slot as f64 - (count - 1) as f64 / 2.0) * spacing;
                Edge::straight(from, &frames[target], offset)
            };
            svg.push_str(&edge.markup(relation));
        }
    }
    for (entity, frame) in entities.iter().zip(&frames) {
        svg.push_str(&node(entity, frame));
    }
    svg.push_str("</svg>\n");
    svg
}

/// `ceil(sqrt(entities))`, computed on integers.
fn columns(entities: usize) -> usize {
    match entities {
        0 => 0,
        n => (n - 1).isqrt() + 1,
    }
}

/// The box of one entity. Every box is `BOX_WIDTH` wide.
struct Frame {
    x: usize,
    y: usize,
    height: usize,
}

impl Frame {
    fn bottom(&self) -> usize {
        self.y + self.height
    }

    fn center(&self) -> (f64, f64) {
        (
            (self.x as f64) + (BOX_WIDTH as f64) / 2.0,
            (self.y as f64) + (self.height as f64) / 2.0,
        )
    }

    /// The fraction of the vector (dx, dy) that leads from the point (ox, oy), measured from the
    /// center of this box, to its edge.
    fn exit(&self, dx: f64, dy: f64, ox: f64, oy: f64) -> f64 {
        let half_width = (BOX_WIDTH as f64) / 2.0;
        let half_height = (self.height as f64) / 2.0;
        ((half_width - ox * dx.signum()) / dx.abs())
            .min((half_height - oy * dy.signum()) / dy.abs())
    }
}

/// Places entities in record order, row by row. Each row starts below the tallest box of the
/// row above.
fn layout(entities: &[Entity], columns: usize) -> Vec<Frame> {
    let height = |entity: &Entity| TITLE_HEIGHT + FIELD_HEIGHT * entity.fields.len();
    let mut frames = Vec::with_capacity(entities.len());
    let mut y = MARGIN;
    for row in entities.chunks(columns.max(1)) {
        frames.extend(row.iter().enumerate().map(|(column, entity)| Frame {
            x: MARGIN + column * (BOX_WIDTH + GAP),
            y,
            height: height(entity),
        }));
        y += row.iter().map(height).max().unwrap_or_default() + GAP;
    }
    frames
}

fn node(entity: &Entity, frame: &Frame) -> String {
    let Frame { x, y, height } = *frame;
    let text_x = x + TEXT_INSET;
    let mut node = format!(
        "<g data-kind=\"entity\" data-id=\"{}\">{}\
         <rect x=\"{x}\" y=\"{y}\" width=\"{BOX_WIDTH}\" height=\"{height}\"/>\
         <text class=\"name\" x=\"{text_x}\" y=\"{}\">{}</text>",
        escape(&entity.id),
        title(entity.description.as_deref()),
        y + NAME_BASELINE,
        escape(&entity.name),
    );
    for (row, field) in entity.fields.iter().enumerate() {
        node.push_str(&format!(
            "<text data-kind=\"field\" data-id=\"{}\" x=\"{text_x}\" y=\"{}\">{}{}</text>",
            escape(&field.id),
            y + TITLE_HEIGHT + FIELD_HEIGHT * row + FIELD_BASELINE,
            title(field.description.as_deref()),
            typed(&field.name, &field.r#type, field.nullable),
        ));
    }
    node.push_str("</g>\n");
    node
}

/// A relation line and the position of its cardinality label.
struct Edge {
    points: Vec<(f64, f64)>,
    label: (f64, f64),
}

impl Edge {
    /// The segment between the box centers, cut at each box edge and shifted sideways by
    /// `offset`. The label sits beside the target end.
    fn straight(from: &Frame, to: &Frame, offset: f64) -> Self {
        let (x1, y1) = from.center();
        let (x2, y2) = to.center();
        let (dx, dy) = (x2 - x1, y2 - y1);
        let length = (dx * dx + dy * dy).sqrt();
        let (ux, uy) = (dx / length, dy / length);
        let (ox, oy) = (-uy * offset, ux * offset);
        let out = from.exit(dx, dy, ox, oy);
        let into = to.exit(-dx, -dy, ox, oy);
        let end = (x2 + ox - dx * into, y2 + oy - dy * into);
        Self {
            points: vec![(x1 + ox + dx * out, y1 + oy + dy * out), end],
            label: (
                end.0 - LABEL_BACK * ux + LABEL_SIDE * uy,
                end.1 - LABEL_BACK * uy - LABEL_SIDE * ux,
            ),
        }
    }

    /// A loop out of the right side of the box and back, because a straight segment from a box
    /// to itself has no length. Loop `slot` of `count` is stacked below the one before it.
    fn self_loop(frame: &Frame, slot: usize, count: usize) -> Self {
        let right = (frame.x + BOX_WIDTH) as f64;
        let outer = right + LOOP_WIDTH;
        let gaps = LOOP_GAP * (count - 1) as f64;
        let height = LOOP_HEIGHT.min((LOOP_SPAN - gaps) / count as f64);
        let first = (TITLE_HEIGHT as f64 - (count as f64 * height + gaps)) / 2.0;
        let top = frame.y as f64 + first + slot as f64 * (height + LOOP_GAP);
        let bottom = top + height;
        Self {
            points: vec![(right, top), (outer, top), (outer, bottom), (right, bottom)],
            label: (outer + LOOP_LABEL_GAP, (top + bottom) / 2.0),
        }
    }

    fn markup(&self, relation: &Relation) -> String {
        let d: Vec<String> = self
            .points
            .iter()
            .enumerate()
            .map(|(index, (x, y))| {
                let command = if index == 0 { "M" } else { "L" };
                format!("{command} {x:.0} {y:.0}")
            })
            .collect();
        let cardinality = match relation.cardinality {
            Cardinality::One => "1",
            Cardinality::Many => "*",
        };
        let (x, y) = self.label;
        format!(
            "<path data-kind=\"relation\" data-id=\"{}\" d=\"{}\">{}</path>\
             <text class=\"cardinality\" x=\"{x:.0}\" y=\"{y:.0}\">{cardinality}</text>\n",
            escape(&relation.id),
            d.join(" "),
            title(relation.name.as_deref()),
        )
    }
}

fn table(interface: &InterfaceRecord) -> String {
    let header = COLUMNS.map(|column| format!("<th>{column}</th>")).concat();
    let rows: String = interface.operations.iter().map(row).collect();
    format!("<table>\n<thead><tr>{header}</tr></thead>\n<tbody>\n{rows}</tbody>\n</table>\n")
}

fn row(operation: &Operation) -> String {
    let description = operation.description.as_deref().map(escape);
    let errors = list(operation.errors.iter().map(|error| {
        let text = format!("{}: {}", escape(&error.name), escape(&error.condition));
        item("error", &error.id, &text)
    }));
    format!(
        "<tr data-kind=\"operation\" data-id=\"{}\"><td class=\"name\">{}</td>{}{}{}{}</tr>\n",
        escape(&operation.id),
        escape(&operation.name),
        cell(&description.unwrap_or_default()),
        cell(&parameters("input", &operation.inputs)),
        cell(&parameters("output", &operation.outputs)),
        cell(&errors),
    )
}

fn parameters(kind: &str, parameters: &[Parameter]) -> String {
    list(parameters.iter().map(|parameter| {
        let text = typed(&parameter.name, &parameter.r#type, parameter.nullable);
        item(kind, &parameter.id, &text)
    }))
}

/// A `<li>` element. `text` must already be escaped.
fn item(kind: &str, id: &str, text: &str) -> String {
    format!(
        "<li data-kind=\"{kind}\" data-id=\"{}\">{text}</li>",
        escape(id)
    )
}

/// A `<ul>` of `items`, or nothing when there are no items.
fn list(items: impl Iterator<Item = String>) -> String {
    let items: String = items.collect();
    if items.is_empty() {
        items
    } else {
        format!("<ul>{items}</ul>")
    }
}

fn cell(content: &str) -> String {
    if content.is_empty() {
        "<td class=\"none\">none</td>".to_owned()
    } else {
        format!("<td>{content}</td>")
    }
}

/// A field or parameter as `name: type`, with a trailing `?` when it is nullable.
fn typed(name: &str, r#type: &str, nullable: bool) -> String {
    let marker = if nullable { "?" } else { "" };
    format!("{}: {}{marker}", escape(name), escape(r#type))
}

/// A tooltip that a browser shows on hover without JavaScript.
fn title(text: Option<&str>) -> String {
    text.map(|text| format!("<title>{}</title>", escape(text)))
        .unwrap_or_default()
}

fn escape(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            c => escaped.push(c),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    const HEADER: DesignHeader = DesignHeader {
        name: "orders",
        kind: "data-model",
    };

    fn page(record: Value) -> String {
        render(
            &HEADER,
            1,
            &Record::DataModel(serde_json::from_value(record).unwrap()),
        )
    }

    /// Two entities, Order and Customer, joined by one relation, with fixed ids.
    fn example() -> Value {
        json!({
            "entities": [
                {
                    "id": "e1",
                    "name": "Order",
                    "description": "A customer order",
                    "fields": [
                        {"id": "f1", "name": "id", "type": "uuid", "nullable": false},
                        {"id": "f2", "name": "total", "type": "decimal", "nullable": false}
                    ],
                    "relations": [
                        {"id": "r1", "target": "e2", "cardinality": "one", "name": "customer"}
                    ]
                },
                {
                    "id": "e2",
                    "name": "Customer",
                    "fields": [{"id": "f3", "name": "id", "type": "uuid", "nullable": false}],
                    "relations": []
                }
            ]
        })
    }

    /// `count` entities named `E0`, `E1`, and so on, with no fields and no relations.
    fn entities(count: usize) -> Value {
        let entities: Vec<Value> = (0..count)
            .map(|index| {
                json!({
                    "id": format!("e{index}"),
                    "name": format!("E{index}"),
                    "fields": [],
                    "relations": []
                })
            })
            .collect();
        json!({ "entities": entities })
    }

    #[test]
    fn page_is_self_contained() {
        let page = page(example());
        assert!(page.starts_with("<!doctype html>\n"), "{page}");
        assert!(page.ends_with("</html>\n"), "{page}");
        for banned in ["<script", "<link", " src=", "http://", "https://"] {
            assert!(!page.contains(banned), "{banned} in:\n{page}");
        }
        assert_eq!(page.matches("<style>").count(), 1, "{page}");
    }

    #[test]
    fn title_names_the_design_its_kind_and_the_version() {
        let page = render(
            &HEADER,
            3,
            &Record::DataModel(serde_json::from_value(example()).unwrap()),
        );
        assert!(page.contains("<title>orders, version 3</title>"), "{page}");
        assert!(
            page.contains("<h1>orders <span>data-model, version 3</span></h1>"),
            "{page}"
        );
    }

    #[test]
    fn columns_are_the_ceiling_of_the_square_root() {
        let columns: Vec<usize> = (0..=10).map(columns).collect();
        assert_eq!(columns, [0, 1, 2, 2, 2, 3, 3, 3, 3, 3, 4]);
    }

    #[test]
    fn view_box_is_computed_from_the_grid() {
        let cases = [
            (example(), (600, 132)),
            (entities(5), (900, 184)),
            (entities(0), (60, 60)),
        ];
        for (record, (width, height)) in cases {
            let svg = format!(
                r#"<svg width="{width}" height="{height}" viewBox="0 0 {width} {height}">"#
            );
            let page = page(record);
            assert!(page.contains(&svg), "{svg} in:\n{page}");
        }
    }

    #[test]
    fn rows_start_below_the_tallest_box_of_the_row_above() {
        let mut record = entities(3);
        record["entities"][0]["fields"] = json!([
            {"id": "f1", "name": "a", "type": "t", "nullable": false},
            {"id": "f2", "name": "b", "type": "t", "nullable": false}
        ]);
        let page = page(record);
        for rect in [
            r#"<rect x="30" y="30" width="240" height="72"/>"#,
            r#"<rect x="330" y="30" width="240" height="32"/>"#,
            r#"<rect x="30" y="162" width="240" height="32"/>"#,
        ] {
            assert!(page.contains(rect), "{rect} in:\n{page}");
        }
    }

    #[test]
    fn entity_is_a_group_with_a_box_a_name_and_one_text_per_field() {
        let page = page(example());
        let order = concat!(
            r#"<g data-kind="entity" data-id="e1"><title>A customer order</title>"#,
            r#"<rect x="30" y="30" width="240" height="72"/>"#,
            r#"<text class="name" x="42" y="51">Order</text>"#,
            r#"<text data-kind="field" data-id="f1" x="42" y="76">id: uuid</text>"#,
            r#"<text data-kind="field" data-id="f2" x="42" y="96">total: decimal</text>"#,
            "</g>"
        );
        assert!(page.contains(order), "{page}");
        let customer = concat!(
            r#"<g data-kind="entity" data-id="e2">"#,
            r#"<rect x="330" y="30" width="240" height="52"/>"#,
        );
        assert!(page.contains(customer), "{page}");
    }

    #[test]
    fn nullable_field_ends_with_a_marker_and_its_description_is_a_title() {
        let mut record = entities(1);
        record["entities"][0]["fields"] = json!([
            {"id": "f1", "name": "note", "type": "text", "nullable": true, "description": "free text"}
        ]);
        let page = page(record);
        assert!(
            page.contains(r#"<text data-kind="field" data-id="f1" x="42" y="76"><title>free text</title>note: text?</text>"#),
            "{page}"
        );
    }

    #[test]
    fn relation_runs_from_box_edge_to_box_edge_with_a_cardinality_label() {
        let page = page(example());
        let edge = concat!(
            r#"<path data-kind="relation" data-id="r1" d="M 270 62 L 330 60"><title>customer</title></path>"#,
            r#"<text class="cardinality" x="314" y="53">1</text>"#,
        );
        assert!(page.contains(edge), "{page}");
    }

    #[test]
    fn many_is_labelled_with_a_star() {
        let mut record = example();
        record["entities"][0]["relations"][0]["cardinality"] = json!("many");
        assert!(page(record).contains(">*</text>"));
    }

    #[test]
    fn self_relation_loops_beside_its_box() {
        let mut record = entities(1);
        record["entities"][0]["relations"] =
            json!([{"id": "r1", "target": "e0", "cardinality": "many"}]);
        let page = page(record);
        let edge = concat!(
            r#"<path data-kind="relation" data-id="r1" d="M 270 38 L 290 38 L 290 54 L 270 54"></path>"#,
            r#"<text class="cardinality" x="300" y="46">*</text>"#,
        );
        assert!(page.contains(edge), "{page}");
    }

    /// The `d` value and the label position of each relation, in document order.
    fn strokes(page: &str) -> Vec<(String, String)> {
        let mut strokes = Vec::new();
        for path in page.split("<path data-kind=\"relation\"").skip(1) {
            let d = path
                .split(" d=\"")
                .nth(1)
                .unwrap()
                .split('"')
                .next()
                .unwrap();
            let label = path.split("<text class=\"cardinality\" ").nth(1).unwrap();
            let label = label.split('>').next().unwrap();
            // Sorted points, so that a reversed path counts as the same stroke.
            let mut points: Vec<&str> = d.split(['M', 'L']).map(str::trim).collect();
            points.sort_unstable();
            strokes.push((points.join(","), label.to_string()));
        }
        strokes
    }

    fn assert_distinct(page: &str, expected: usize) {
        let strokes = strokes(page);
        assert_eq!(strokes.len(), expected, "{page}");
        for (index, a) in strokes.iter().enumerate() {
            for b in &strokes[index + 1..] {
                assert_ne!(a.0, b.0, "{page}");
                assert_ne!(a.1, b.1, "{page}");
            }
        }
    }

    #[test]
    fn parallel_relations_get_their_own_stroke_and_label() {
        let mut record = example();
        record["entities"][0]["relations"] = json!([
            {"id": "r1", "target": "e2", "cardinality": "one", "name": "billing"},
            {"id": "r2", "target": "e2", "cardinality": "many", "name": "shipping"}
        ]);
        assert_distinct(&page(record), 2);
    }

    #[test]
    fn opposite_relations_get_their_own_stroke_and_label() {
        let mut record = entities(2);
        record["entities"][0]["relations"] =
            json!([{"id": "r1", "target": "e1", "cardinality": "one"}]);
        record["entities"][1]["relations"] =
            json!([{"id": "r2", "target": "e0", "cardinality": "many"}]);
        assert_distinct(&page(record), 2);
    }

    #[test]
    fn self_relations_on_one_entity_do_not_overlap() {
        let mut record = entities(1);
        record["entities"][0]["relations"] = json!([
            {"id": "r1", "target": "e0", "cardinality": "many"},
            {"id": "r2", "target": "e0", "cardinality": "one"}
        ]);
        assert_distinct(&page(record), 2);
    }

    #[test]
    fn edges_are_drawn_beneath_the_boxes() {
        let page = page(example());
        let edge = page.find(r#"data-kind="relation""#).unwrap();
        let entity = page.find(r#"data-kind="entity""#).unwrap();
        assert!(edge < entity, "{page}");
    }

    fn interface_page(record: Value) -> String {
        let header = DesignHeader {
            name: "orders-api",
            kind: "interface",
        };
        render(
            &header,
            2,
            &Record::Interface(serde_json::from_value(record).unwrap()),
        )
    }

    /// One operation with one input, one output, and one error, with fixed ids.
    fn interface() -> Value {
        json!({
            "operations": [{
                "id": "o1",
                "name": "createOrder",
                "description": "Creates an order",
                "inputs": [{"id": "p1", "name": "customer_id", "type": "uuid", "nullable": false}],
                "outputs": [{"id": "p2", "name": "order", "type": "Order", "nullable": false}],
                "errors": [{"id": "x1", "name": "CustomerNotFound", "condition": "no customer has customer_id"}]
            }]
        })
    }

    #[test]
    fn interface_page_is_self_contained_and_names_the_design() {
        let page = interface_page(interface());
        assert!(page.starts_with("<!doctype html>\n"), "{page}");
        for banned in ["<script", "<link", " src=", "http://", "https://", "<svg"] {
            assert!(!page.contains(banned), "{banned} in:\n{page}");
        }
        assert_eq!(page.matches("<style>").count(), 1, "{page}");
        assert!(
            page.contains("<h1>orders-api <span>interface, version 2</span></h1>"),
            "{page}"
        );
    }

    #[test]
    fn table_has_one_header_row_of_five_columns() {
        let page = interface_page(interface());
        let header = concat!(
            "<table>\n<thead><tr><th>Operation</th><th>Description</th>",
            "<th>Inputs</th><th>Outputs</th><th>Errors</th></tr></thead>\n<tbody>\n"
        );
        assert!(page.contains(header), "{page}");
        assert!(page.contains("</tbody>\n</table>\n"), "{page}");
    }

    #[test]
    fn operation_is_a_row_whose_items_carry_ids() {
        let page = interface_page(interface());
        let row = concat!(
            r#"<tr data-kind="operation" data-id="o1">"#,
            r#"<td class="name">createOrder</td>"#,
            "<td>Creates an order</td>",
            r#"<td><ul><li data-kind="input" data-id="p1">customer_id: uuid</li></ul></td>"#,
            r#"<td><ul><li data-kind="output" data-id="p2">order: Order</li></ul></td>"#,
            r#"<td><ul><li data-kind="error" data-id="x1">CustomerNotFound: no customer has customer_id</li></ul></td>"#,
            "</tr>\n"
        );
        assert!(page.contains(row), "{page}");
    }

    #[test]
    fn empty_cells_say_none_and_nullable_parameters_end_with_a_marker() {
        let page = interface_page(json!({
            "operations": [{
                "id": "o1",
                "name": "listOrders",
                "inputs": [],
                "outputs": [
                    {"id": "p1", "name": "orders", "type": "Order[]", "nullable": true},
                    {"id": "p2", "name": "next", "type": "cursor", "nullable": false}
                ],
                "errors": []
            }]
        }));
        let row = concat!(
            r#"<tr data-kind="operation" data-id="o1">"#,
            r#"<td class="name">listOrders</td>"#,
            r#"<td class="none">none</td>"#,
            r#"<td class="none">none</td>"#,
            r#"<td><ul><li data-kind="output" data-id="p1">orders: Order[]?</li>"#,
            r#"<li data-kind="output" data-id="p2">next: cursor</li></ul></td>"#,
            r#"<td class="none">none</td>"#,
            "</tr>\n"
        );
        assert!(page.contains(row), "{page}");
    }

    #[test]
    fn interface_text_is_escaped() {
        let mut record = interface();
        record["operations"][0]["name"] = json!("<b>get</b>");
        record["operations"][0]["errors"][0]["condition"] = json!("a & b");
        let page = interface_page(record);
        assert!(page.contains("&lt;b&gt;get&lt;/b&gt;"), "{page}");
        assert!(page.contains("CustomerNotFound: a &amp; b"), "{page}");
        assert!(!page.contains("<b>"), "{page}");
    }

    #[test]
    fn markup_characters_are_escaped() {
        assert_eq!(
            escape(r#"<a href="x">&'"#),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;"
        );
        let mut record = entities(1);
        record["entities"][0]["name"] = json!("<b>Bold</b>");
        record["entities"][0]["description"] = json!("a & b");
        let page = page(record);
        assert!(page.contains("&lt;b&gt;Bold&lt;/b&gt;"), "{page}");
        assert!(page.contains("<title>a &amp; b</title>"), "{page}");
        assert!(!page.contains("<b>"), "{page}");
    }
}
