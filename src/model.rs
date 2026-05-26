use std::collections::{BTreeMap, BTreeSet};

use kicad_ipc_rs::{
    BoardLayerInfo, BoardNet, ItemBoundingBox, NetClassInfo, PadNetEntry, PcbItem, PcbPad, PcbVia,
    SelectionSummary, Vector2Nm,
};
use serde::Serialize;

use crate::units::nm_to_mm;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct LayerSummary {
    pub id: i32,
    pub name: String,
}

impl From<&BoardLayerInfo> for LayerSummary {
    fn from(value: &BoardLayerInfo) -> Self {
        Self {
            id: value.id,
            name: value.name.clone(),
        }
    }
}

pub fn is_known_layer_name(name: &str) -> bool {
    !name.contains("UNKNOWN")
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct NetSummary {
    pub code: i32,
    pub name: String,
}

impl From<&BoardNet> for NetSummary {
    fn from(value: &BoardNet) -> Self {
        Self {
            code: value.code,
            name: value.name.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Serialize)]
pub struct PointSummary {
    pub x_nm: i64,
    pub y_nm: i64,
    pub x_mm: f64,
    pub y_mm: f64,
}

impl From<Vector2Nm> for PointSummary {
    fn from(value: Vector2Nm) -> Self {
        Self {
            x_nm: value.x_nm,
            y_nm: value.y_nm,
            x_mm: nm_to_mm(value.x_nm),
            y_mm: nm_to_mm(value.y_nm),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CountRow {
    pub name: String,
    pub count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
pub struct SelectionReplaceSemantics {
    pub preflighted_item_ids: bool,
    pub atomic: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct SelectionMutationReport {
    pub action: String,
    pub requested_item_ids: Vec<String>,
    pub selected_total: usize,
    pub selected_item_ids: Vec<String>,
    pub type_counts: Vec<CountRow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_items: Vec<ItemSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replace: Option<SelectionReplaceSemantics>,
}

pub fn selection_mutation_report(
    action: impl Into<String>,
    requested_item_ids: Vec<String>,
    selected_items: Vec<PcbItem>,
    summary: SelectionSummary,
    replace: Option<SelectionReplaceSemantics>,
) -> SelectionMutationReport {
    let selected_item_ids = selected_items
        .iter()
        .filter_map(item_id)
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let selected_items = selected_items
        .iter()
        .map(ItemSummary::from)
        .collect::<Vec<_>>();

    SelectionMutationReport {
        action: action.into(),
        requested_item_ids,
        selected_total: summary.total_items,
        selected_item_ids,
        type_counts: summary
            .type_url_counts
            .into_iter()
            .map(|entry| CountRow {
                name: entry.type_url,
                count: entry.count,
            })
            .collect(),
        selected_items,
        replace,
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ItemSummary {
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub net: Option<NetSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub layer: Option<LayerSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub layers: Vec<LayerSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub position: Option<PointSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locked: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub details: BTreeMap<String, String>,
}

impl From<&PcbItem> for ItemSummary {
    fn from(item: &PcbItem) -> Self {
        let mut details = BTreeMap::new();
        match item {
            PcbItem::Track(track) => {
                details.insert("width_nm".to_string(), opt_i64(track.width_nm));
                Self {
                    kind: "track".to_string(),
                    id: track.id.clone(),
                    reference: None,
                    value: None,
                    number: None,
                    net: track.net.as_ref().map(NetSummary::from),
                    layer: Some(LayerSummary::from(&track.layer)),
                    layers: Vec::new(),
                    position: track.start_nm.map(PointSummary::from),
                    locked: Some(format!("{:?}", track.locked)),
                    details,
                }
            }
            PcbItem::Arc(arc) => {
                details.insert("width_nm".to_string(), opt_i64(arc.width_nm));
                Self {
                    kind: "arc".to_string(),
                    id: arc.id.clone(),
                    reference: None,
                    value: None,
                    number: None,
                    net: arc.net.as_ref().map(NetSummary::from),
                    layer: Some(LayerSummary::from(&arc.layer)),
                    layers: Vec::new(),
                    position: arc.start_nm.map(PointSummary::from),
                    locked: Some(format!("{:?}", arc.locked)),
                    details,
                }
            }
            PcbItem::Via(via) => Self::from_via(via),
            PcbItem::Footprint(footprint) => {
                details.insert("pad_count".to_string(), footprint.pad_count.to_string());
                details.insert(
                    "definition_item_count".to_string(),
                    footprint.definition_item_count.to_string(),
                );
                Self {
                    kind: "footprint".to_string(),
                    id: footprint.id.clone(),
                    reference: footprint.reference.clone(),
                    value: footprint.value.clone(),
                    number: None,
                    net: None,
                    layer: Some(LayerSummary::from(&footprint.layer)),
                    layers: Vec::new(),
                    position: footprint.position_nm.map(PointSummary::from),
                    locked: Some(format!("{:?}", footprint.locked)),
                    details,
                }
            }
            PcbItem::Pad(pad) => Self::from_pad(pad),
            PcbItem::BoardGraphicShape(shape) => {
                if let Some(kind) = &shape.geometry_kind {
                    details.insert("geometry".to_string(), kind.clone());
                }
                Self {
                    kind: "shape".to_string(),
                    id: shape.id.clone(),
                    reference: None,
                    value: None,
                    number: None,
                    net: shape.net.as_ref().map(NetSummary::from),
                    layer: Some(LayerSummary::from(&shape.layer)),
                    layers: Vec::new(),
                    position: None,
                    locked: Some(format!("{:?}", shape.locked)),
                    details,
                }
            }
            PcbItem::BoardText(text) => Self {
                kind: "text".to_string(),
                id: text.id.clone(),
                reference: None,
                value: text.text.clone(),
                number: None,
                net: None,
                layer: Some(LayerSummary::from(&text.layer)),
                layers: Vec::new(),
                position: text.position_nm.map(PointSummary::from),
                locked: Some(format!("{:?}", text.locked)),
                details,
            },
            PcbItem::BoardTextBox(text_box) => Self {
                kind: "text-box".to_string(),
                id: text_box.id.clone(),
                reference: None,
                value: text_box.text.clone(),
                number: None,
                net: None,
                layer: Some(LayerSummary::from(&text_box.layer)),
                layers: Vec::new(),
                position: text_box.top_left_nm.map(PointSummary::from),
                locked: Some(format!("{:?}", text_box.locked)),
                details,
            },
            PcbItem::Field(field) => Self {
                kind: "field".to_string(),
                id: None,
                reference: Some(field.name.clone()),
                value: field.text.clone(),
                number: None,
                net: None,
                layer: None,
                layers: Vec::new(),
                position: None,
                locked: None,
                details,
            },
            PcbItem::Zone(zone) => {
                details.insert("priority".to_string(), zone.priority.to_string());
                details.insert("filled".to_string(), zone.filled.to_string());
                details.insert("polygon_count".to_string(), zone.polygon_count.to_string());
                Self {
                    kind: "zone".to_string(),
                    id: zone.id.clone(),
                    reference: None,
                    value: Some(zone.name.clone()),
                    number: None,
                    net: None,
                    layer: None,
                    layers: zone.layers.iter().map(LayerSummary::from).collect(),
                    position: None,
                    locked: Some(format!("{:?}", zone.locked)),
                    details,
                }
            }
            PcbItem::Dimension(dimension) => Self {
                kind: "dimension".to_string(),
                id: dimension.id.clone(),
                reference: None,
                value: dimension.text.clone(),
                number: None,
                net: None,
                layer: Some(LayerSummary::from(&dimension.layer)),
                layers: Vec::new(),
                position: None,
                locked: Some(format!("{:?}", dimension.locked)),
                details,
            },
            PcbItem::ReferenceImage(image) => Self {
                kind: "reference-image".to_string(),
                id: image.id.clone(),
                reference: None,
                value: None,
                number: None,
                net: None,
                layer: Some(LayerSummary::from(&image.layer)),
                layers: Vec::new(),
                position: image.position_nm.map(PointSummary::from),
                locked: Some(format!("{:?}", image.locked)),
                details,
            },
            PcbItem::Barcode(barcode) => Self {
                kind: "barcode".to_string(),
                id: barcode.id.clone(),
                reference: None,
                value: Some(barcode.text.clone()),
                number: None,
                net: None,
                layer: Some(LayerSummary::from(&barcode.layer)),
                layers: Vec::new(),
                position: barcode.position_nm.map(PointSummary::from),
                locked: Some(format!("{:?}", barcode.locked)),
                details,
            },
            PcbItem::Group(group) => {
                details.insert("item_count".to_string(), group.item_count.to_string());
                Self {
                    kind: "group".to_string(),
                    id: group.id.clone(),
                    reference: None,
                    value: Some(group.name.clone()),
                    number: None,
                    net: None,
                    layer: None,
                    layers: Vec::new(),
                    position: None,
                    locked: None,
                    details,
                }
            }
            PcbItem::Unknown(unknown) => {
                details.insert("type_url".to_string(), unknown.type_url.clone());
                details.insert("raw_len".to_string(), unknown.raw_len.to_string());
                Self {
                    kind: "unknown".to_string(),
                    id: None,
                    reference: None,
                    value: None,
                    number: None,
                    net: None,
                    layer: None,
                    layers: Vec::new(),
                    position: None,
                    locked: None,
                    details,
                }
            }
        }
    }
}

impl ItemSummary {
    fn from_via(via: &PcbVia) -> Self {
        let mut details = BTreeMap::new();
        details.insert("via_type".to_string(), format!("{:?}", via.via_type));
        let layers = via
            .layers
            .as_ref()
            .map(|layers| {
                layers
                    .padstack_layers
                    .iter()
                    .map(LayerSummary::from)
                    .collect()
            })
            .unwrap_or_default();
        Self {
            kind: "via".to_string(),
            id: via.id.clone(),
            reference: None,
            value: None,
            number: None,
            net: via.net.as_ref().map(NetSummary::from),
            layer: None,
            layers,
            position: via.position_nm.map(PointSummary::from),
            locked: Some(format!("{:?}", via.locked)),
            details,
        }
    }

    fn from_pad(pad: &PcbPad) -> Self {
        let mut details = BTreeMap::new();
        details.insert("pad_type".to_string(), format!("{:?}", pad.pad_type));
        let layers = pad
            .pad_stack
            .as_ref()
            .map(|stack| stack.layers.iter().map(LayerSummary::from).collect())
            .unwrap_or_default();
        Self {
            kind: "pad".to_string(),
            id: pad.id.clone(),
            reference: None,
            value: None,
            number: Some(pad.number.clone()),
            net: pad.net.as_ref().map(NetSummary::from),
            layer: None,
            layers,
            position: pad.position_nm.map(PointSummary::from),
            locked: Some(format!("{:?}", pad.locked)),
            details,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct PadSummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footprint_reference: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub footprint_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pad_id: Option<String>,
    pub pad_number: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub net_code: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub net_name: Option<String>,
}

impl From<&PadNetEntry> for PadSummary {
    fn from(value: &PadNetEntry) -> Self {
        Self {
            footprint_reference: value.footprint_reference.clone(),
            footprint_id: value.footprint_id.clone(),
            pad_id: value.pad_id.clone(),
            pad_number: value.pad_number.clone(),
            net_code: value.net_code,
            net_name: value.net_name.clone(),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct NetClassSummary {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub priority: Option<i32>,
    pub class_type: String,
    pub constituents: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub board: BTreeMap<String, String>,
}

impl From<&NetClassInfo> for NetClassSummary {
    fn from(value: &NetClassInfo) -> Self {
        let mut board = BTreeMap::new();
        if let Some(settings) = &value.board {
            board.insert("clearance_nm".to_string(), opt_i64(settings.clearance_nm));
            board.insert(
                "track_width_nm".to_string(),
                opt_i64(settings.track_width_nm),
            );
            board.insert(
                "diff_pair_track_width_nm".to_string(),
                opt_i64(settings.diff_pair_track_width_nm),
            );
            board.insert(
                "diff_pair_gap_nm".to_string(),
                opt_i64(settings.diff_pair_gap_nm),
            );
        }
        Self {
            name: value.name.clone(),
            priority: value.priority,
            class_type: format!("{:?}", value.class_type),
            constituents: value.constituents.clone(),
            board,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct BoundingBoxSummary {
    pub item_id: String,
    pub x_nm: i64,
    pub y_nm: i64,
    pub width_nm: i64,
    pub height_nm: i64,
}

impl From<&ItemBoundingBox> for BoundingBoxSummary {
    fn from(value: &ItemBoundingBox) -> Self {
        Self {
            item_id: value.item_id.clone(),
            x_nm: value.x_nm,
            y_nm: value.y_nm,
            width_nm: value.width_nm,
            height_nm: value.height_nm,
        }
    }
}

pub fn item_kind(item: &PcbItem) -> &'static str {
    match item {
        PcbItem::Track(_) => "track",
        PcbItem::Arc(_) => "arc",
        PcbItem::Via(_) => "via",
        PcbItem::Footprint(_) => "footprint",
        PcbItem::Pad(_) => "pad",
        PcbItem::BoardGraphicShape(_) => "shape",
        PcbItem::BoardText(_) => "text",
        PcbItem::BoardTextBox(_) => "text-box",
        PcbItem::Field(_) => "field",
        PcbItem::Zone(_) => "zone",
        PcbItem::Dimension(_) => "dimension",
        PcbItem::ReferenceImage(_) => "reference-image",
        PcbItem::Barcode(_) => "barcode",
        PcbItem::Group(_) => "group",
        PcbItem::Unknown(_) => "unknown",
    }
}

pub fn item_id(item: &PcbItem) -> Option<&str> {
    match item {
        PcbItem::Track(item) => item.id.as_deref(),
        PcbItem::Arc(item) => item.id.as_deref(),
        PcbItem::Via(item) => item.id.as_deref(),
        PcbItem::Footprint(item) => item.id.as_deref(),
        PcbItem::Pad(item) => item.id.as_deref(),
        PcbItem::BoardGraphicShape(item) => item.id.as_deref(),
        PcbItem::BoardText(item) => item.id.as_deref(),
        PcbItem::BoardTextBox(item) => item.id.as_deref(),
        PcbItem::Field(_) => None,
        PcbItem::Zone(item) => item.id.as_deref(),
        PcbItem::Dimension(item) => item.id.as_deref(),
        PcbItem::ReferenceImage(item) => item.id.as_deref(),
        PcbItem::Barcode(item) => item.id.as_deref(),
        PcbItem::Group(item) => item.id.as_deref(),
        PcbItem::Unknown(_) => None,
    }
}

fn opt_i64(value: Option<i64>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "-".to_string())
}
