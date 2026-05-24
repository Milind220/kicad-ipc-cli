use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use anyhow::{bail, Context};
use kicad_ipc_rs::{
    BoardNet, DocumentSpecifier, DocumentType, KiCadClientBlocking, KiCadError,
    NetClassForNetEntry, PcbItem, SelectionItemDetail, TextObjectSpec, TextShapeGeometry, TextSpec,
};
use serde::Serialize;

use crate::cli::{
    NetReportArgs, OutputFormat, SelectionArgs, SnapshotArgs, SnapshotScope, TextShapesArgs,
};
use crate::model::{
    count_rows_from_typed_groups, item_id, item_kind, BoundingBoxSummary, CountRow, ItemSummary,
    LayerSummary, NetClassSummary, NetSummary, PadSummary, PointSummary,
};
use crate::output;

use super::all_type_codes;

pub fn doctor(client: &KiCadClientBlocking, format: OutputFormat) -> anyhow::Result<()> {
    client.ping().context("KiCad ping failed")?;
    let version = client
        .get_version()
        .context("failed to read KiCad version")?;
    let board_open = client
        .has_open_board()
        .context("failed to check board state")?;
    let project_path = client
        .get_current_project_path()
        .ok()
        .map(|path| path.display().to_string());

    let mut open_documents = Vec::new();
    for document_type in document_types() {
        let docs = client
            .get_open_documents(document_type)
            .with_context(|| format!("failed to list open {document_type} documents"))?;
        open_documents.extend(docs.iter().map(DocumentSummary::from));
    }

    let report = DoctorReport {
        ok: true,
        socket_uri: client.socket_uri().to_string(),
        version: VersionSummary {
            major: version.major,
            minor: version.minor,
            patch: version.patch,
            full_version: version.full_version,
        },
        board_open,
        project_path,
        open_documents,
    };

    output::print(format, &report, || {
        let project = report.project_path.as_deref().unwrap_or("-");
        format!(
            "KiCad IPC: ok\nversion: {}\nsocket: {}\nboard open: {}\nproject: {}\nopen documents: {}",
            report.version.full_version,
            report.socket_uri,
            yes_no(report.board_open),
            project,
            report.open_documents.len()
        )
    })
}

pub fn board_summary(client: &KiCadClientBlocking, format: OutputFormat) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let enabled_layers = client
        .get_board_enabled_layers()
        .context("failed to read enabled layers")?;
    let visible_layers = client
        .get_visible_layers()
        .context("failed to read visible layers")?;
    let active_layer = client
        .get_active_layer()
        .context("failed to read active layer")?;
    let grid_origin = client
        .get_board_origin(kicad_ipc_rs::BoardOriginKind::Grid)
        .context("failed to read grid origin")?;
    let drill_origin = client
        .get_board_origin(kicad_ipc_rs::BoardOriginKind::Drill)
        .context("failed to read drill origin")?;
    let stackup = client
        .get_board_stackup()
        .context("failed to read board stackup")?;
    let nets = client.get_nets().context("failed to read nets")?;
    let all_items = client
        .get_all_pcb_items()
        .context("failed to inventory PCB items")?;

    let report = BoardSummaryReport {
        copper_layer_count: enabled_layers.copper_layer_count,
        enabled_layers: enabled_layers
            .layers
            .iter()
            .map(LayerSummary::from)
            .collect(),
        visible_layers: visible_layers.iter().map(LayerSummary::from).collect(),
        active_layer: LayerSummary::from(&active_layer),
        grid_origin: PointSummary::from(grid_origin),
        drill_origin: PointSummary::from(drill_origin),
        stackup: StackupSummary {
            finish_type_name: stackup.finish_type_name,
            impedance_controlled: stackup.impedance_controlled,
            edge_has_connector: stackup.edge_has_connector,
            edge_has_castellated_pads: stackup.edge_has_castellated_pads,
            edge_has_edge_plating: stackup.edge_has_edge_plating,
            layer_count: stackup.layers.len(),
            copper_layer_count: stackup
                .layers
                .iter()
                .filter(|layer| {
                    matches!(
                        layer.layer_type,
                        kicad_ipc_rs::BoardStackupLayerType::Copper
                    )
                })
                .count(),
            dielectric_layer_count: stackup
                .layers
                .iter()
                .filter(|layer| {
                    matches!(
                        layer.layer_type,
                        kicad_ipc_rs::BoardStackupLayerType::Dielectric
                    )
                })
                .count(),
            layers: stackup
                .layers
                .iter()
                .map(|layer| StackupLayerSummary {
                    id: layer.layer.id,
                    name: layer.layer.name.clone(),
                    user_name: layer.user_name.clone(),
                    material_name: layer.material_name.clone(),
                    enabled: layer.enabled,
                    layer_type: format!("{:?}", layer.layer_type),
                    thickness_nm: layer.thickness_nm,
                })
                .collect(),
        },
        net_count: nets.len(),
        item_counts_by_api_type: count_rows_from_typed_groups(&all_items),
        decoded_item_counts: decoded_count_rows(&all_items),
    };

    output::print(format, &report, || {
        format!(
            "board summary\nlayers: {} copper, {} enabled, {} visible\nactive layer: {} ({})\nnets: {}\nstackup: {} layers, finish `{}`\nitems: {} decoded",
            report.copper_layer_count,
            report.enabled_layers.len(),
            report.visible_layers.len(),
            report.active_layer.name,
            report.active_layer.id,
            report.net_count,
            report.stackup.layer_count,
            report.stackup.finish_type_name,
            report
                .decoded_item_counts
                .iter()
                .map(|row| row.count)
                .sum::<usize>()
        )
    })
}

pub fn inventory(client: &KiCadClientBlocking, format: OutputFormat) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let nets = client.get_nets().context("failed to read nets")?;
    let pad_rows = client
        .get_pad_netlist()
        .context("failed to read pad netlist")?;
    let all_items = client
        .get_all_pcb_items()
        .context("failed to inventory PCB items")?;
    let flat = flatten_items(&all_items);
    let footprints = flat
        .iter()
        .filter(|item| matches!(item, PcbItem::Footprint(_)))
        .map(ItemSummary::from)
        .collect::<Vec<_>>();
    let tracks = flat
        .iter()
        .filter(|item| matches!(item, PcbItem::Track(_)))
        .count();
    let arcs = flat
        .iter()
        .filter(|item| matches!(item, PcbItem::Arc(_)))
        .count();
    let vias = flat
        .iter()
        .filter(|item| matches!(item, PcbItem::Via(_)))
        .count();
    let zones = flat
        .iter()
        .filter(|item| matches!(item, PcbItem::Zone(_)))
        .count();
    let groups = flat
        .iter()
        .filter(|item| matches!(item, PcbItem::Group(_)))
        .count();

    let report = InventoryReport {
        nets: nets.iter().map(NetSummary::from).collect(),
        pads: pad_rows.iter().map(PadSummary::from).collect(),
        footprints,
        counts: InventoryCounts {
            nets: nets.len(),
            pads: pad_rows.len(),
            tracks,
            arcs,
            vias,
            zones,
            groups,
            total_decoded_items: flat.len(),
        },
        item_counts_by_api_type: count_rows_from_typed_groups(&all_items),
    };

    output::print(format, &report, || {
        format!(
            "inventory\nfootprints: {}\npads: {}\nnets: {}\ntracks: {} (+{} arcs)\nvias: {}\nzones: {}\ngroups: {}",
            report.footprints.len(),
            report.pads.len(),
            report.nets.len(),
            report.counts.tracks,
            report.counts.arcs,
            report.counts.vias,
            report.counts.zones,
            report.counts.groups
        )
    })
}

pub fn selection(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &SelectionArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let summary = client
        .get_selection_summary(Vec::new())
        .context("failed to read selection summary")?;
    let items = client
        .get_selection(Vec::new())
        .context("failed to read selected items")?;
    let details = if args.details {
        client
            .get_selection_details(Vec::new())
            .context("failed to read selection details")?
    } else {
        Vec::new()
    };
    let bboxes = selected_bounding_boxes(client, &items)?;

    let report = SelectionReport {
        total_items: summary.total_items,
        type_counts: summary
            .type_url_counts
            .iter()
            .map(|row| TypeCountSummary {
                type_url: row.type_url.clone(),
                count: row.count,
            })
            .collect(),
        items: items.iter().map(ItemSummary::from).collect(),
        details: details.iter().map(SelectionDetailSummary::from).collect(),
        bounding_boxes: bboxes.iter().map(BoundingBoxSummary::from).collect(),
    };

    output::print(format, &report, || {
        let kinds = report
            .items
            .iter()
            .fold(BTreeMap::<String, usize>::new(), |mut acc, item| {
                *acc.entry(item.kind.clone()).or_default() += 1;
                acc
            });
        let kind_text = kinds
            .into_iter()
            .map(|(name, count)| format!("{name}={count}"))
            .collect::<Vec<_>>()
            .join(", ");
        format!(
            "selection\nitems: {}\n{}",
            report.total_items,
            if kind_text.is_empty() {
                "no selected items".to_string()
            } else {
                kind_text
            }
        )
    })
}

pub fn net_report(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &NetReportArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let nets = client.get_nets().context("failed to read nets")?;
    let selected_nets = resolve_nets(&nets, &args.nets)?;
    let pad_rows = client
        .get_pad_netlist()
        .context("failed to read pad netlist")?;
    let net_classes = client
        .get_net_classes()
        .context("failed to read net classes")?;
    let class_rows = if selected_nets.is_empty() {
        client
            .get_netclass_for_nets(nets.clone())
            .context("failed to resolve net classes for nets")?
    } else {
        client
            .get_netclass_for_nets(selected_nets.clone())
            .context("failed to resolve net classes for selected nets")?
    };
    let class_by_net = class_rows
        .iter()
        .map(|row| (row.net_name.clone(), row))
        .collect::<BTreeMap<_, _>>();
    let overviews = nets
        .iter()
        .map(|net| NetOverview {
            net: NetSummary::from(net),
            pad_count: pad_rows
                .iter()
                .filter(|row| row.net_name.as_deref() == Some(net.name.as_str()))
                .count(),
            net_class: class_by_net
                .get(&net.name)
                .map(|row| row.net_class.name.clone()),
        })
        .collect::<Vec<_>>();

    let mut details = Vec::new();
    for net in &selected_nets {
        let pads = pad_rows
            .iter()
            .filter(|row| row.net_name.as_deref() == Some(net.name.as_str()))
            .map(PadSummary::from)
            .collect::<Vec<_>>();
        let items = client
            .get_items_by_net(all_type_codes(), vec![net.clone()])
            .with_context(|| format!("failed to read items on net `{}`", net.name))?;
        let connected_items = if args.connected {
            let ids = items
                .iter()
                .filter_map(item_id)
                .map(str::to_string)
                .collect::<Vec<_>>();
            client
                .get_connected_items(ids, all_type_codes())
                .with_context(|| format!("failed to read connected items on net `{}`", net.name))?
        } else {
            Vec::new()
        };
        details.push(NetDetail {
            net: NetSummary::from(net),
            net_class: class_by_net
                .get(&net.name)
                .map(|row| NetClassForNetSummary::from(*row)),
            pad_count: pads.len(),
            pads,
            item_count: items.len(),
            items: items.iter().map(ItemSummary::from).collect(),
            connected_item_count: if args.connected {
                Some(connected_items.len())
            } else {
                None
            },
            connected_items: connected_items.iter().map(ItemSummary::from).collect(),
        });
    }

    let report = NetReport {
        net_count: nets.len(),
        net_classes: net_classes.iter().map(NetClassSummary::from).collect(),
        nets: overviews,
        selected_nets: details,
    };

    output::print(format, &report, || {
        if report.selected_nets.is_empty() {
            format!(
                "net report\nnets: {}\nnet classes: {}\nuse --net <name> for item and pad details",
                report.net_count,
                report.net_classes.len()
            )
        } else {
            let lines = report
                .selected_nets
                .iter()
                .map(|net| {
                    format!(
                        "{}: pads={} items={}",
                        net.net.name, net.pad_count, net.item_count
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("net report\n{lines}")
        }
    })
}

pub fn snapshot(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &SnapshotArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let (contents, ids) = match args.scope {
        SnapshotScope::Board => (
            client
                .get_board_as_string()
                .context("failed to snapshot board")?,
            Vec::new(),
        ),
        SnapshotScope::Selection => {
            let selection = client
                .get_selection_as_string()
                .context("failed to snapshot selection")?;
            (selection.contents, selection.ids)
        }
    };
    fs::write(&args.output, &contents)
        .with_context(|| format!("failed to write snapshot to `{}`", args.output.display()))?;
    let report = SnapshotReport {
        scope: format!("{:?}", args.scope).to_ascii_lowercase(),
        output: args.output.display().to_string(),
        bytes: contents.len(),
        selection_ids: ids,
    };

    output::print(format, &report, || {
        format!("snapshot wrote {} bytes to {}", report.bytes, report.output)
    })
}

pub fn text_shapes(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &TextShapesArgs,
) -> anyhow::Result<()> {
    let extents = client
        .get_text_extents(TextSpec::plain(args.text.clone()))
        .context("failed to measure text extents")?;
    let entries = client
        .get_text_as_shapes(vec![TextObjectSpec::Text(TextSpec::plain(
            args.text.clone(),
        ))])
        .context("failed to convert text to shapes")?;
    let mut counts = BTreeMap::<String, usize>::new();
    for entry in &entries {
        for shape in &entry.shapes {
            *counts
                .entry(shape_geometry_name(&shape.geometry).to_string())
                .or_default() += 1;
        }
    }
    let report = TextShapesReport {
        text: args.text.clone(),
        extents: TextExtentsSummary {
            x_nm: extents.x_nm,
            y_nm: extents.y_nm,
            width_nm: extents.width_nm,
            height_nm: extents.height_nm,
        },
        entry_count: entries.len(),
        shape_count: entries.iter().map(|entry| entry.shapes.len()).sum(),
        shape_counts: counts
            .into_iter()
            .map(|(name, count)| CountRow { name, count })
            .collect(),
    };

    output::print(format, &report, || {
        format!(
            "text shapes\nextents: {} x {} nm\nshapes: {}",
            report.extents.width_nm, report.extents.height_nm, report.shape_count
        )
    })
}

pub fn ensure_board_open(client: &KiCadClientBlocking) -> anyhow::Result<()> {
    if client
        .has_open_board()
        .context("failed to check board state")?
    {
        Ok(())
    } else {
        Err(KiCadError::BoardNotOpen.into())
    }
}

pub fn flatten_items(groups: &[(kicad_ipc_rs::PcbObjectTypeCode, Vec<PcbItem>)]) -> Vec<PcbItem> {
    groups
        .iter()
        .flat_map(|(_, items)| items.iter().cloned())
        .collect()
}

pub fn resolve_nets(all_nets: &[BoardNet], requested: &[String]) -> anyhow::Result<Vec<BoardNet>> {
    let mut resolved = Vec::new();
    let mut seen = BTreeSet::<String>::new();
    for name_or_code in requested {
        let found = if let Ok(code) = name_or_code.parse::<i32>() {
            all_nets.iter().find(|net| net.code == code)
        } else {
            all_nets.iter().find(|net| net.name == *name_or_code)
        };
        let Some(net) = found else {
            bail!("net `{name_or_code}` was not found");
        };
        if seen.insert(net.name.clone()) {
            resolved.push(net.clone());
        }
    }
    Ok(resolved)
}

fn selected_bounding_boxes(
    client: &KiCadClientBlocking,
    items: &[PcbItem],
) -> anyhow::Result<Vec<kicad_ipc_rs::ItemBoundingBox>> {
    let ids = items
        .iter()
        .filter_map(item_id)
        .map(str::to_string)
        .collect::<Vec<_>>();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    client
        .get_item_bounding_boxes(ids, true)
        .context("failed to read selection bounding boxes")
}

fn decoded_count_rows(groups: &[(kicad_ipc_rs::PcbObjectTypeCode, Vec<PcbItem>)]) -> Vec<CountRow> {
    let mut counts = BTreeMap::<String, usize>::new();
    for (_, items) in groups {
        for item in items {
            *counts.entry(item_kind(item).to_string()).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .map(|(name, count)| CountRow { name, count })
        .collect()
}

fn shape_geometry_name(geometry: &TextShapeGeometry) -> &'static str {
    match geometry {
        TextShapeGeometry::Segment { .. } => "segment",
        TextShapeGeometry::Rectangle { .. } => "rectangle",
        TextShapeGeometry::Arc { .. } => "arc",
        TextShapeGeometry::Circle { .. } => "circle",
        TextShapeGeometry::Polygon { .. } => "polygon",
        TextShapeGeometry::Bezier { .. } => "bezier",
        TextShapeGeometry::Unknown => "unknown",
    }
}

fn document_types() -> [DocumentType; 6] {
    [
        DocumentType::Schematic,
        DocumentType::Symbol,
        DocumentType::Pcb,
        DocumentType::Footprint,
        DocumentType::DrawingSheet,
        DocumentType::Project,
    ]
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

#[derive(Debug, Serialize)]
struct DoctorReport {
    ok: bool,
    socket_uri: String,
    version: VersionSummary,
    board_open: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_path: Option<String>,
    open_documents: Vec<DocumentSummary>,
}

#[derive(Debug, Serialize)]
struct VersionSummary {
    major: u32,
    minor: u32,
    patch: u32,
    full_version: String,
}

#[derive(Debug, Serialize)]
struct DocumentSummary {
    document_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    board_filename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    project_path: Option<String>,
}

impl From<&DocumentSpecifier> for DocumentSummary {
    fn from(value: &DocumentSpecifier) -> Self {
        Self {
            document_type: value.document_type.to_string(),
            board_filename: value.board_filename.clone(),
            project_name: value.project.name.clone(),
            project_path: value
                .project
                .path
                .as_ref()
                .map(|path| path.display().to_string()),
        }
    }
}

#[derive(Debug, Serialize)]
struct BoardSummaryReport {
    copper_layer_count: u32,
    enabled_layers: Vec<LayerSummary>,
    visible_layers: Vec<LayerSummary>,
    active_layer: LayerSummary,
    grid_origin: PointSummary,
    drill_origin: PointSummary,
    stackup: StackupSummary,
    net_count: usize,
    item_counts_by_api_type: Vec<CountRow>,
    decoded_item_counts: Vec<CountRow>,
}

#[derive(Debug, Serialize)]
struct StackupSummary {
    finish_type_name: String,
    impedance_controlled: bool,
    edge_has_connector: bool,
    edge_has_castellated_pads: bool,
    edge_has_edge_plating: bool,
    layer_count: usize,
    copper_layer_count: usize,
    dielectric_layer_count: usize,
    layers: Vec<StackupLayerSummary>,
}

#[derive(Debug, Serialize)]
struct StackupLayerSummary {
    id: i32,
    name: String,
    user_name: String,
    material_name: String,
    enabled: bool,
    layer_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    thickness_nm: Option<i64>,
}

#[derive(Debug, Serialize)]
struct InventoryReport {
    counts: InventoryCounts,
    nets: Vec<NetSummary>,
    footprints: Vec<ItemSummary>,
    pads: Vec<PadSummary>,
    item_counts_by_api_type: Vec<CountRow>,
}

#[derive(Debug, Serialize)]
struct InventoryCounts {
    nets: usize,
    pads: usize,
    tracks: usize,
    arcs: usize,
    vias: usize,
    zones: usize,
    groups: usize,
    total_decoded_items: usize,
}

#[derive(Debug, Serialize)]
struct SelectionReport {
    total_items: usize,
    type_counts: Vec<TypeCountSummary>,
    items: Vec<ItemSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    details: Vec<SelectionDetailSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    bounding_boxes: Vec<BoundingBoxSummary>,
}

#[derive(Debug, Serialize)]
struct TypeCountSummary {
    type_url: String,
    count: usize,
}

#[derive(Debug, Serialize)]
struct SelectionDetailSummary {
    type_url: String,
    detail: String,
    raw_len: usize,
}

impl From<&SelectionItemDetail> for SelectionDetailSummary {
    fn from(value: &SelectionItemDetail) -> Self {
        Self {
            type_url: value.type_url.clone(),
            detail: value.detail.clone(),
            raw_len: value.raw_len,
        }
    }
}

#[derive(Debug, Serialize)]
struct NetReport {
    net_count: usize,
    net_classes: Vec<NetClassSummary>,
    nets: Vec<NetOverview>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    selected_nets: Vec<NetDetail>,
}

#[derive(Debug, Serialize)]
struct NetOverview {
    net: NetSummary,
    pad_count: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    net_class: Option<String>,
}

#[derive(Debug, Serialize)]
struct NetDetail {
    net: NetSummary,
    #[serde(skip_serializing_if = "Option::is_none")]
    net_class: Option<NetClassForNetSummary>,
    pad_count: usize,
    pads: Vec<PadSummary>,
    item_count: usize,
    items: Vec<ItemSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    connected_item_count: Option<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    connected_items: Vec<ItemSummary>,
}

#[derive(Debug, Serialize)]
struct NetClassForNetSummary {
    net_name: String,
    net_class: NetClassSummary,
}

impl From<&NetClassForNetEntry> for NetClassForNetSummary {
    fn from(value: &NetClassForNetEntry) -> Self {
        Self {
            net_name: value.net_name.clone(),
            net_class: NetClassSummary::from(&value.net_class),
        }
    }
}

#[derive(Debug, Serialize)]
struct SnapshotReport {
    scope: String,
    output: String,
    bytes: usize,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    selection_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct TextShapesReport {
    text: String,
    extents: TextExtentsSummary,
    entry_count: usize,
    shape_count: usize,
    shape_counts: Vec<CountRow>,
}

#[derive(Debug, Serialize)]
struct TextExtentsSummary {
    x_nm: i64,
    y_nm: i64,
    width_nm: i64,
    height_nm: i64,
}
