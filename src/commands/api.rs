use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use anyhow::{bail, Context};
use kicad_ipc_rs::{
    BoardEditorAppearanceSettings, BoardLayerInfo, BoardStackup, BoardStackupDielectricProperties,
    BoardStackupLayer, BoardStackupLayerType, BoardTextSpec, ColorRgba, CommitSession,
    ItemLockState, KiCadClientBlocking, KiCadError, NetClassBoardSettings, NetClassInfo,
    NetClassType, PcbBoardText, PcbItem, TextAttributesSpec, TextHorizontalAlignment,
    TextObjectSpec, TextSpec, TextVerticalAlignment, TitleBlockInfo, Vector2Nm,
};
use prost_types::Any;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::cli::*;
use crate::model::{
    is_known_layer_name, BoundingBoxSummary, CountRow, ItemSummary, LayerSummary, NetClassSummary,
    NetSummary, PointSummary,
};
use crate::output;
use crate::units::{parse_distance_nm, parse_point_nm};

use super::all_type_codes;
use super::inspect::{ensure_board_open, resolve_nets};

const EXPOSED_BINDINGS: &[&str] = &[
    "send_raw_command",
    "ping",
    "refresh_editor",
    "run_action",
    "get_version",
    "get_kicad_binary_path",
    "get_plugin_settings_path",
    "get_open_documents",
    "get_net_classes",
    "set_net_classes",
    "get_text_variables",
    "set_text_variables",
    "expand_text_variables",
    "get_text_extents",
    "get_text_as_shapes",
    "get_current_project_path",
    "has_open_board",
    "get_nets",
    "get_board_enabled_layers",
    "set_board_enabled_layers",
    "get_active_layer",
    "set_active_layer",
    "get_visible_layers",
    "set_visible_layers",
    "get_board_layer_name",
    "get_board_origin",
    "set_board_origin",
    "get_board_stackup",
    "update_board_stackup",
    "get_graphics_defaults",
    "get_board_editor_appearance_settings",
    "set_board_editor_appearance_settings",
    "interactive_move_items",
    "get_items_by_net",
    "get_items_by_net_class",
    "get_connected_items",
    "get_netclass_for_nets",
    "refill_zones",
    "refill_all_zones",
    "get_pad_shape_as_polygon",
    "check_padstack_presence_on_layers",
    "inject_drc_error",
    "get_selection_summary",
    "get_selection",
    "get_selection_details",
    "add_to_selection",
    "remove_from_selection",
    "clear_selection",
    "begin_commit",
    "end_commit",
    "create_items",
    "create_board_text",
    "create_board_texts",
    "create_board_text_in_container",
    "create_board_texts_in_container",
    "update_items",
    "delete_items",
    "parse_and_create_items_from_string",
    "get_items_by_id",
    "get_editable_items_by_id",
    "get_item_bounding_boxes",
    "hit_test_item",
    "get_title_block_info",
    "set_title_block_info",
    "save_document",
    "save_copy_of_document",
    "revert_document",
    "get_board_as_string",
    "get_selection_as_string",
];

pub fn list(format: OutputFormat) -> anyhow::Result<()> {
    let value = json!({
        "bindings": EXPOSED_BINDINGS,
        "type_filter": "use --type track|trace|footprint|pad|text|silkscreen-text or --type-code <number>",
        "layer_filter": "use layer names like F.SilkS, proto names like BL_F_SilkS, or numeric ids",
        "raw_item_schema": {"type_url": "type.googleapis.com/...", "value_hex": "protobuf bytes as lowercase hex"},
        "raw_command_schema": {"type_url": "type.googleapis.com/kiapi.common.commands.Ping", "value_hex": ""},
        "board_text_schema": [{"text": "hello", "at": "10mm,20mm", "layer": "F.SilkS", "height": "1mm", "stroke_width": "0.15mm"}],
        "net_classes_schema": [{"name": "Default", "class_type": "explicit", "constituents": [], "board": {"track_width_nm": 250000}}],
        "stackup_schema": "same field names as `api board stackup` output"
    });
    print_value(format, value, "api bindings listed")
}

pub fn ping(client: &KiCadClientBlocking, format: OutputFormat) -> anyhow::Result<()> {
    client.ping().context("KiCad ping failed")?;
    print_value(format, json!({"ok": true}), "ok")
}

pub fn version(client: &KiCadClientBlocking, format: OutputFormat) -> anyhow::Result<()> {
    let v = client
        .get_version()
        .context("failed to read KiCad version")?;
    print_value(
        format,
        json!({"major": v.major, "minor": v.minor, "patch": v.patch, "full_version": v.full_version}),
        "version read",
    )
}

pub fn project_path(client: &KiCadClientBlocking, format: OutputFormat) -> anyhow::Result<()> {
    let path = client
        .get_current_project_path()
        .context("failed to read current project path")?;
    print_value(format, json!({"project_path": path}), "project path read")
}

pub fn has_open_board(client: &KiCadClientBlocking, format: OutputFormat) -> anyhow::Result<()> {
    let open = client
        .has_open_board()
        .context("failed to check open board")?;
    print_value(format, json!({"board_open": open}), "board state read")
}

pub fn open_documents(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &DocumentTypeArgs,
) -> anyhow::Result<()> {
    let docs = client
        .get_open_documents(args.document_type.into())
        .context("failed to get open documents")?;
    let docs = docs
        .iter()
        .map(|doc| {
            json!({
                "document_type": doc.document_type.to_string(),
                "board_filename": doc.board_filename,
                "project_name": doc.project.name,
                "project_path": doc.project.path,
            })
        })
        .collect::<Vec<_>>();
    print_value(format, json!({"documents": docs}), "open documents read")
}

pub fn kicad_binary_path(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &BinaryPathArgs,
) -> anyhow::Result<()> {
    let path = client
        .get_kicad_binary_path(args.binary_name.clone())
        .context("failed to resolve KiCad binary path")?;
    print_value(
        format,
        json!({"binary_name": args.binary_name, "path": path}),
        "binary path read",
    )
}

pub fn plugin_settings_path(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &PluginSettingsArgs,
) -> anyhow::Result<()> {
    let path = client
        .get_plugin_settings_path(args.identifier.clone())
        .context("failed to resolve plugin settings path")?;
    print_value(
        format,
        json!({"identifier": args.identifier, "path": path}),
        "plugin settings path read",
    )
}

pub fn refresh(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &RefreshArgs,
) -> anyhow::Result<()> {
    client
        .refresh_editor(args.frame.into())
        .context("failed to refresh editor")?;
    print_value(
        format,
        json!({"ok": true, "frame": format!("{:?}", args.frame)}),
        "editor refreshed",
    )
}

pub fn run_action(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &RunActionArgs,
) -> anyhow::Result<()> {
    let status = client
        .run_action(args.action.clone())
        .context("failed to run KiCad action")?;
    print_value(
        format,
        json!({"action": args.action, "status": format!("{status:?}")}),
        "action dispatched",
    )
}

pub fn net_classes(client: &KiCadClientBlocking, format: OutputFormat) -> anyhow::Result<()> {
    let classes = client
        .get_net_classes()
        .context("failed to get net classes")?;
    print_value(
        format,
        json!({"net_classes": classes.iter().map(NetClassSummary::from).collect::<Vec<_>>()}),
        "net classes read",
    )
}

pub fn set_net_classes(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &NetClassesSetArgs,
) -> anyhow::Result<()> {
    let classes: NetClassesJson = read_json_arg(args.json.as_deref(), args.file.as_ref())?;
    let classes = classes
        .into_vec()
        .into_iter()
        .map(NetClassInfo::try_from)
        .collect::<anyhow::Result<Vec<_>>>()?;
    let updated = client
        .set_net_classes(classes, args.mode.into())
        .context("failed to set net classes")?;
    print_value(
        format,
        json!({"net_classes": updated.iter().map(NetClassSummary::from).collect::<Vec<_>>()}),
        "net classes set",
    )
}

pub fn text_variables_get(
    client: &KiCadClientBlocking,
    format: OutputFormat,
) -> anyhow::Result<()> {
    let variables = client
        .get_text_variables()
        .context("failed to get text variables")?;
    print_value(
        format,
        json!({"variables": variables}),
        "text variables read",
    )
}

pub fn text_variables_set(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &TextVariablesSetArgs,
) -> anyhow::Result<()> {
    let variables = args.vars.iter().cloned().collect::<BTreeMap<_, _>>();
    let result = client
        .set_text_variables(variables, args.mode.into())
        .context("failed to set text variables")?;
    print_value(
        format,
        json!({"mode": format!("{:?}", args.mode).to_ascii_lowercase(), "variables": result}),
        "text variables set",
    )
}

pub fn expand_text_variables(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &ExpandTextVariablesArgs,
) -> anyhow::Result<()> {
    let expanded = client
        .expand_text_variables(args.text.clone())
        .context("failed to expand text variables")?;
    print_value(format, json!({"expanded": expanded}), "text expanded")
}

pub fn text_extents(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &TextValueArgs,
) -> anyhow::Result<()> {
    let extents = client
        .get_text_extents(TextSpec::plain(args.text.clone()))
        .context("failed to get text extents")?;
    print_value(
        format,
        json!({"text": args.text, "extents": extents_value(extents)}),
        "text extents read",
    )
}

pub fn text_as_shapes(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &TextValueArgs,
) -> anyhow::Result<()> {
    let shapes = client
        .get_text_as_shapes(vec![TextObjectSpec::Text(TextSpec::plain(
            args.text.clone(),
        ))])
        .context("failed to convert text as shapes")?;
    print_value(
        format,
        json!({"text": args.text, "entries": debug_values(&shapes)}),
        "text shapes read",
    )
}

pub fn board_nets(client: &KiCadClientBlocking, format: OutputFormat) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let nets = client.get_nets().context("failed to get nets")?;
    print_value(
        format,
        json!({"nets": nets.iter().map(NetSummary::from).collect::<Vec<_>>()}),
        "nets read",
    )
}

pub fn enabled_layers(client: &KiCadClientBlocking, format: OutputFormat) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let layers = client
        .get_board_enabled_layers()
        .context("failed to get enabled layers")?;
    print_value(
        format,
        json!({"copper_layer_count": layers.copper_layer_count, "layers": layers.layers.iter().map(LayerSummary::from).collect::<Vec<_>>()}),
        "enabled layers read",
    )
}

pub fn set_enabled_layers(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &SetEnabledLayersArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let layers = client
        .set_board_enabled_layers(args.copper_layer_count, args.layer_ids.clone())
        .context("failed to set enabled layers")?;
    print_value(
        format,
        json!({"copper_layer_count": layers.copper_layer_count, "layers": layers.layers.iter().map(LayerSummary::from).collect::<Vec<_>>()}),
        "enabled layers set",
    )
}

pub fn active_layer(client: &KiCadClientBlocking, format: OutputFormat) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let layer = client
        .get_active_layer()
        .context("failed to get active layer")?;
    print_value(
        format,
        json!({"active_layer": LayerSummary::from(&layer)}),
        "active layer read",
    )
}

pub fn set_active_layer(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &LayerIdArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    client
        .set_active_layer(args.layer_id)
        .context("failed to set active layer")?;
    active_layer(client, format)
}

pub fn visible_layers(client: &KiCadClientBlocking, format: OutputFormat) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let layers = client
        .get_visible_layers()
        .context("failed to get visible layers")?;
    let enabled_layer_ids = client
        .get_board_enabled_layers()
        .context("failed to get enabled layers")?
        .layers
        .into_iter()
        .map(|layer| layer.id)
        .collect::<BTreeSet<_>>();
    print_value(
        format,
        json!({"visible_layers": layers.iter().filter(|layer| enabled_layer_ids.contains(&layer.id) && is_known_layer_name(&layer.name)).map(LayerSummary::from).collect::<Vec<_>>()}),
        "visible layers read",
    )
}

pub fn set_visible_layers(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &LayerIdsArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    client
        .set_visible_layers(args.layer_ids.clone())
        .context("failed to set visible layers")?;
    visible_layers(client, format)
}

pub fn layer_name(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &LayerIdArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let name = client
        .get_board_layer_name(args.layer_id)
        .context("failed to get layer name")?;
    print_value(
        format,
        json!({"layer_id": args.layer_id, "name": name}),
        "layer name read",
    )
}

pub fn origin(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &BoardOriginArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let point = client
        .get_board_origin(args.kind.into())
        .context("failed to get board origin")?;
    print_value(
        format,
        json!({"kind": format!("{:?}", args.kind).to_ascii_lowercase(), "origin": PointSummary::from(point)}),
        "origin read",
    )
}

pub fn set_origin(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &SetBoardOriginArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    client
        .set_board_origin(
            args.kind.into(),
            Vector2Nm {
                x_nm: args.at.x_nm,
                y_nm: args.at.y_nm,
            },
        )
        .context("failed to set board origin")?;
    print_value(
        format,
        json!({"kind": format!("{:?}", args.kind).to_ascii_lowercase(), "origin": PointSummary::from(Vector2Nm { x_nm: args.at.x_nm, y_nm: args.at.y_nm })}),
        "origin set",
    )
}

pub fn stackup(client: &KiCadClientBlocking, format: OutputFormat) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let stackup = client
        .get_board_stackup()
        .context("failed to get board stackup")?;
    let layers = stackup
        .layers
        .iter()
        .map(|layer| {
            json!({
                "layer": LayerSummary::from(&layer.layer),
                "user_name": layer.user_name,
                "material_name": layer.material_name,
                "enabled": layer.enabled,
                "thickness_nm": layer.thickness_nm,
                "layer_type": format!("{:?}", layer.layer_type),
                "color": layer.color.map(|c| json!({"r": c.r, "g": c.g, "b": c.b, "a": c.a})),
                "dielectric_layers": debug_values(&layer.dielectric_layers),
            })
        })
        .collect::<Vec<_>>();
    print_value(
        format,
        json!({
            "finish_type_name": stackup.finish_type_name,
            "impedance_controlled": stackup.impedance_controlled,
            "edge_has_connector": stackup.edge_has_connector,
            "edge_has_castellated_pads": stackup.edge_has_castellated_pads,
            "edge_has_edge_plating": stackup.edge_has_edge_plating,
            "layers": layers,
        }),
        "stackup read",
    )
}

pub fn update_stackup(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &StackupUpdateArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let stackup: StackupJson = read_json_arg(args.json.as_deref(), args.file.as_ref())?;
    let updated = client
        .update_board_stackup(BoardStackup::try_from(stackup)?)
        .context("failed to update board stackup")?;
    print_stackup_value(format, updated, "stackup updated")
}

pub fn graphics_defaults(client: &KiCadClientBlocking, format: OutputFormat) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let defaults = client
        .get_graphics_defaults()
        .context("failed to get graphics defaults")?;
    print_value(
        format,
        json!({"layers": debug_values(&defaults.layers)}),
        "graphics defaults read",
    )
}

pub fn appearance(client: &KiCadClientBlocking, format: OutputFormat) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let settings = client
        .get_board_editor_appearance_settings()
        .context("failed to get board editor appearance")?;
    print_value(format, appearance_value(&settings), "appearance read")
}

pub fn set_appearance(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &SetAppearanceArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let current = client
        .get_board_editor_appearance_settings()
        .context("failed to get current appearance")?;
    let next = BoardEditorAppearanceSettings {
        inactive_layer_display: args
            .inactive_layer_display
            .map(Into::into)
            .unwrap_or(current.inactive_layer_display),
        net_color_display: args
            .net_color_display
            .map(Into::into)
            .unwrap_or(current.net_color_display),
        board_flip: args
            .board_flip
            .map(Into::into)
            .unwrap_or(current.board_flip),
        ratsnest_display: args
            .ratsnest_display
            .map(Into::into)
            .unwrap_or(current.ratsnest_display),
    };
    let settings = client
        .set_board_editor_appearance_settings(next)
        .context("failed to set board editor appearance")?;
    print_value(format, appearance_value(&settings), "appearance set")
}

pub fn interactive_move_items(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &ItemIdsArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    client
        .interactive_move_items(args.item_ids.clone())
        .context("failed to start interactive move")?;
    print_value(
        format,
        json!({"ok": true, "item_ids": args.item_ids}),
        "interactive move started",
    )
}

pub fn board_items(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &ApiBoardItemsArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    if args.details {
        if args.type_codes.is_empty() {
            let buckets = client
                .get_all_pcb_items_details()
                .context("failed to get all item details")?;
            return print_item_detail_buckets(format, buckets, "all item details read");
        }
        let details = client
            .get_items_details_by_type_codes(args.type_codes.clone())
            .context("failed to get item details")?;
        return print_value(format, item_details_value(details), "item details read");
    }
    if args.type_codes.is_empty() {
        let buckets = client
            .get_all_pcb_items()
            .context("failed to get all PCB items")?;
        return print_item_buckets(format, buckets, "all items read");
    }
    let items = client
        .get_items_by_type_codes(args.type_codes.clone())
        .context("failed to get items")?;
    print_items(format, items, "items read")
}

pub fn items_by_net(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &ItemsByNetArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let nets = client.get_nets().context("failed to get nets")?;
    let nets = resolve_nets(&nets, &args.nets)?;
    let items = client
        .get_items_by_net(default_type_codes(&args.type_codes), nets)
        .context("failed to get items by net")?;
    print_items(format, items, "items by net read")
}

pub fn items_by_net_class(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &ItemsByNetClassArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let items = client
        .get_items_by_net_class(
            default_type_codes(&args.type_codes),
            args.net_classes.clone(),
        )
        .context("failed to get items by net class")?;
    print_items(format, items, "items by net class read")
}

pub fn connected_items(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &ConnectedItemsArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let items = client
        .get_connected_items(args.item_ids.clone(), default_type_codes(&args.type_codes))
        .context("failed to get connected items")?;
    print_items(format, items, "connected items read")
}

pub fn netclass_for_nets(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &NetNamesArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let nets = client.get_nets().context("failed to get nets")?;
    let nets = resolve_nets(&nets, &args.nets)?;
    let classes = client
        .get_netclass_for_nets(nets)
        .context("failed to get net classes for nets")?;
    print_value(
        format,
        json!({"net_classes": debug_values(&classes)}),
        "netclass mapping read",
    )
}

pub fn pad_shape_as_polygon(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &PadShapeAsPolygonArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let polygons = client
        .get_pad_shape_as_polygon(args.pad_ids.clone(), args.layer_id)
        .context("failed to get pad shape polygons")?;
    print_value(
        format,
        json!({"polygons": debug_values(&polygons)}),
        "pad polygons read",
    )
}

pub fn padstack_presence(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &PadstackPresenceArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let rows = client
        .check_padstack_presence_on_layers(args.item_ids.clone(), args.layer_ids.clone())
        .context("failed to check padstack presence")?;
    print_value(
        format,
        json!({"presence": debug_values(&rows)}),
        "padstack presence read",
    )
}

pub fn bounding_boxes(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &BoundingBoxesArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let boxes = client
        .get_item_bounding_boxes(args.item_ids.clone(), args.include_child_text)
        .context("failed to get item bounding boxes")?;
    print_value(
        format,
        json!({"bounding_boxes": boxes.iter().map(BoundingBoxSummary::from).collect::<Vec<_>>()}),
        "bounding boxes read",
    )
}

pub fn hit_test(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &HitTestArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let result = client
        .hit_test_item(
            args.item_id.clone(),
            Vector2Nm {
                x_nm: args.at.x_nm,
                y_nm: args.at.y_nm,
            },
            args.tolerance_nm,
        )
        .context("failed to hit-test item")?;
    print_value(
        format,
        json!({"result": format!("{result:?}")}),
        "hit test complete",
    )
}

pub fn refill_zones(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &ZoneIdsArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    if args.zone_ids.is_empty() {
        client
            .refill_all_zones()
            .context("failed to refill all zones")?;
    } else {
        client
            .refill_zones(args.zone_ids.clone())
            .context("failed to refill zones")?;
    }
    print_value(
        format,
        json!({"ok": true, "zone_ids": args.zone_ids}),
        "zones refilled",
    )
}

pub fn selection_summary(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &TypeCodesArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let summary = client
        .get_selection_summary(args.type_codes.clone())
        .context("failed to get selection summary")?;
    print_value(
        format,
        json!({
            "total_items": summary.total_items,
            "type_counts": summary.type_url_counts.into_iter().map(|row| CountRow { name: row.type_url, count: row.count }).collect::<Vec<_>>()
        }),
        "selection summary read",
    )
}

pub fn selection_get(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &TypeCodesArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let items = client
        .get_selection(args.type_codes.clone())
        .context("failed to get selection")?;
    print_items(format, items, "selection read")
}

pub fn selection_details(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &TypeCodesArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let items = client
        .get_selection_details(args.type_codes.clone())
        .context("failed to get selection details")?;
    print_value(
        format,
        json!({"items": debug_values(&items)}),
        "selection details read",
    )
}

pub fn selection_add(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &ItemIdsArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let result = client
        .add_to_selection(args.item_ids.clone())
        .context("failed to add to selection")?;
    print_selection_result(format, result, "selection added")
}

pub fn selection_remove(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &ItemIdsArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let result = client
        .remove_from_selection(args.item_ids.clone())
        .context("failed to remove from selection")?;
    print_selection_result(format, result, "selection removed")
}

pub fn selection_clear(client: &KiCadClientBlocking, format: OutputFormat) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let result = client
        .clear_selection()
        .context("failed to clear selection")?;
    print_selection_result(format, result, "selection cleared")
}

pub fn begin_commit(client: &KiCadClientBlocking, format: OutputFormat) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let session = client.begin_commit().context("failed to begin commit")?;
    print_value(format, json!({"session_id": session.id}), "commit begun")
}

pub fn end_commit(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &EndCommitArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    client
        .end_commit(
            CommitSession {
                id: args.session_id.clone(),
            },
            args.action.into(),
            args.message.clone(),
        )
        .context("failed to end commit")?;
    print_value(
        format,
        json!({"ok": true, "session_id": args.session_id}),
        "commit ended",
    )
}

pub fn create_board_text(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &CreateBoardTextArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let spec = board_text_spec_from_args(args);
    let created = if let Some(container_id) = &args.container_id {
        client
            .create_board_text_in_container(spec, container_id.clone())
            .context("failed to create board text in container")?
    } else {
        client
            .create_board_text(spec)
            .context("failed to create board text")?
    };
    print_value(
        format,
        json!({"created_text": board_text_value(&created)}),
        "board text created",
    )
}

pub fn create_board_texts(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &CreateBoardTextsArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let specs: BoardTextSpecsJson = read_json_arg(None, Some(&args.file))?;
    let specs = specs.into_specs()?;
    let created = if let Some(container_id) = &args.container_id {
        client
            .create_board_texts_in_container(specs, container_id.clone())
            .context("failed to create board texts in container")?
    } else {
        client
            .create_board_texts(specs)
            .context("failed to create board texts")?
    };
    print_value(
        format,
        json!({
            "count": created.len(),
            "created_texts": created.iter().map(board_text_value).collect::<Vec<_>>()
        }),
        "board texts created",
    )
}

pub fn create_raw(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &RawItemsArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let items = read_raw_items(args)?;
    let created = client
        .create_items(items, args.container_id.clone())
        .context("failed to create raw items")?;
    print_value(
        format,
        json!({"created_items": any_values(&created)}),
        "items created",
    )
}

pub fn update_raw(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &RawItemsArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let items = read_raw_items(args)?;
    let updated = client
        .update_items(items)
        .context("failed to update raw items")?;
    print_value(
        format,
        json!({"updated_items": any_values(&updated)}),
        "items updated",
    )
}

pub fn parse_create(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &ParseCreateArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let contents = match (&args.text, &args.file) {
        (Some(text), None) => text.clone(),
        (None, Some(path)) => fs::read_to_string(path)
            .with_context(|| format!("failed to read `{}`", path.display()))?,
        (None, None) => bail!("provide --text or --file"),
        (Some(_), Some(_)) => unreachable!("clap conflicts_with enforces this"),
    };
    if contents.contains("(gr_text") {
        bail!(
            "parse-create is unreliable for board text/silkscreen; use `api items create-board-text` instead"
        );
    }
    let items = client
        .parse_and_create_items_from_string(contents)
        .context("failed to parse and create items")?;
    print_value(
        format,
        json!({"created_items": debug_values(&items)}),
        "items created",
    )
}

pub fn delete_items(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &ItemIdsArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let deleted = client
        .delete_items(args.item_ids.clone())
        .context("failed to delete items")?;
    let verification = verify_items_deleted(client, &deleted);
    print_value(
        format,
        json!({
            "requested_item_ids": args.item_ids,
            "accepted_item_ids": deleted.clone(),
            "deleted_item_ids": deleted,
            "verified_deleted": verification.verified_deleted,
            "remaining_items": verification.remaining_items,
            "verification_error": verification.error
        }),
        "delete accepted by KiCad",
    )
}

pub fn raw_send(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &RawCommandArgs,
) -> anyhow::Result<()> {
    let command = read_raw_command(args)?;
    let command_type_url = command.type_url.clone();
    let response = client
        .send_raw_command(command)
        .context("failed to send raw command")?;
    print_value(
        format,
        json!({
            "command_type_url": command_type_url,
            "response": any_value(&response),
        }),
        "raw command sent",
    )
}

pub fn get_items_by_id(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &LookupItemIdsArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    match client.get_items_by_id(args.item_ids.clone()) {
        Ok(items) => print_items(format, items, "items read"),
        Err(err) if args.missing_ok && is_absent_item_error(&err) => print_value(
            format,
            json!({"count": 0, "items": [], "missing": true, "requested_item_ids": args.item_ids}),
            "items missing",
        ),
        Err(err) => Err(err).context("failed to get items by id"),
    }
}

pub fn get_editable_items_by_id(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &LookupItemIdsArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    match client.get_editable_items_by_id(args.item_ids.clone()) {
        Ok(items) => print_value(
            format,
            json!({"count": items.len(), "items": debug_values(&items)}),
            "editable items read",
        ),
        Err(err) if args.missing_ok && is_absent_item_error(&err) => print_value(
            format,
            json!({"count": 0, "items": [], "missing": true, "requested_item_ids": args.item_ids}),
            "editable items missing",
        ),
        Err(err) => Err(err).context("failed to get editable items by id"),
    }
}

pub fn title_block(client: &KiCadClientBlocking, format: OutputFormat) -> anyhow::Result<()> {
    let title = client
        .get_title_block_info()
        .context("failed to get title block")?;
    print_value(format, title_block_value(&title), "title block read")
}

pub fn set_title_block(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &SetTitleBlockArgs,
) -> anyhow::Result<()> {
    let current = client
        .get_title_block_info()
        .context("failed to get current title block")?;
    let next = TitleBlockInfo {
        title: args.title.clone().unwrap_or(current.title),
        date: args.date.clone().unwrap_or(current.date),
        revision: args.revision.clone().unwrap_or(current.revision),
        company: args.company.clone().unwrap_or(current.company),
        comments: if args.comments.is_empty() {
            current.comments
        } else {
            args.comments.clone()
        },
    };
    client
        .set_title_block_info(next.clone())
        .context("failed to set title block")?;
    print_value(format, title_block_value(&next), "title block set")
}

pub fn save_document(client: &KiCadClientBlocking, format: OutputFormat) -> anyhow::Result<()> {
    client.save_document().context("failed to save document")?;
    print_value(format, json!({"ok": true}), "document saved")
}

pub fn save_copy(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &SaveCopyArgs,
) -> anyhow::Result<()> {
    client
        .save_copy_of_document(
            args.path.display().to_string(),
            args.overwrite,
            args.include_project,
        )
        .context("failed to save document copy")?;
    print_value(format, json!({"path": args.path}), "document copy saved")
}

pub fn revert_document(client: &KiCadClientBlocking, format: OutputFormat) -> anyhow::Result<()> {
    client
        .revert_document()
        .context("failed to revert document")?;
    print_value(format, json!({"ok": true}), "document reverted")
}

pub fn board_string(client: &KiCadClientBlocking, format: OutputFormat) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let contents = client
        .get_board_as_string()
        .context("failed to get board string")?;
    print_value(format, json!({"contents": contents}), "board string read")
}

pub fn selection_string(client: &KiCadClientBlocking, format: OutputFormat) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let dump = client
        .get_selection_as_string()
        .context("failed to get selection string")?;
    print_value(
        format,
        json!({"ids": dump.ids, "contents": dump.contents}),
        "selection string read",
    )
}

fn print_items(format: OutputFormat, items: Vec<PcbItem>, human: &str) -> anyhow::Result<()> {
    print_value(
        format,
        json!({"count": items.len(), "items": items.iter().map(ItemSummary::from).collect::<Vec<_>>()}),
        human,
    )
}

fn print_item_buckets(
    format: OutputFormat,
    buckets: Vec<(kicad_ipc_rs::PcbObjectTypeCode, Vec<PcbItem>)>,
    human: &str,
) -> anyhow::Result<()> {
    let count = buckets.iter().map(|(_, items)| items.len()).sum::<usize>();
    let flat_items = buckets
        .iter()
        .flat_map(|(_, items)| items.iter().map(ItemSummary::from))
        .collect::<Vec<_>>();
    let bucket_values = buckets
        .iter()
        .map(|(object_type, items)| {
            json!({
                "type_code": object_type.code,
                "type_name": object_type.name,
                "count": items.len(),
                "items": items.iter().map(ItemSummary::from).collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();

    print_value(
        format,
        json!({"count": count, "buckets": bucket_values, "items": flat_items}),
        human,
    )
}

fn print_item_detail_buckets(
    format: OutputFormat,
    buckets: Vec<(
        kicad_ipc_rs::PcbObjectTypeCode,
        Vec<kicad_ipc_rs::SelectionItemDetail>,
    )>,
    human: &str,
) -> anyhow::Result<()> {
    let count = buckets
        .iter()
        .map(|(_, details)| details.len())
        .sum::<usize>();
    let bucket_values = buckets
        .into_iter()
        .map(|(object_type, details)| {
            json!({
                "type_code": object_type.code,
                "type_name": object_type.name,
                "count": details.len(),
                "items": item_detail_rows(&details),
            })
        })
        .collect::<Vec<_>>();
    print_value(
        format,
        json!({"count": count, "buckets": bucket_values}),
        human,
    )
}

fn item_details_value(details: Vec<kicad_ipc_rs::SelectionItemDetail>) -> Value {
    json!({"count": details.len(), "items": item_detail_rows(&details)})
}

fn item_detail_rows(details: &[kicad_ipc_rs::SelectionItemDetail]) -> Vec<Value> {
    details
        .iter()
        .map(|detail| {
            json!({
                "type_url": detail.type_url,
                "detail": detail.detail,
                "raw_len": detail.raw_len,
            })
        })
        .collect()
}

fn print_selection_result(
    format: OutputFormat,
    result: kicad_ipc_rs::SelectionMutationResult,
    human: &str,
) -> anyhow::Result<()> {
    print_value(
        format,
        json!({
            "selected_total": result.summary.total_items,
            "type_counts": result.summary.type_url_counts.into_iter().map(|row| CountRow { name: row.type_url, count: row.count }).collect::<Vec<_>>(),
            "items": result.items.iter().map(ItemSummary::from).collect::<Vec<_>>(),
        }),
        human,
    )
}

fn print_value(format: OutputFormat, value: Value, human: &str) -> anyhow::Result<()> {
    output::print(format, &value, || human.to_string())
}

struct DeleteVerification {
    verified_deleted: Value,
    remaining_items: Vec<ItemSummary>,
    error: Value,
}

fn verify_items_deleted(client: &KiCadClientBlocking, item_ids: &[String]) -> DeleteVerification {
    if item_ids.is_empty() {
        return DeleteVerification {
            verified_deleted: Value::Bool(true),
            remaining_items: Vec::new(),
            error: Value::Null,
        };
    }

    match client.get_items_by_id(item_ids.to_vec()) {
        Ok(items) => DeleteVerification {
            verified_deleted: Value::Bool(items.is_empty()),
            remaining_items: items.iter().map(ItemSummary::from).collect(),
            error: Value::Null,
        },
        Err(err) if is_absent_item_error(&err) => DeleteVerification {
            verified_deleted: Value::Bool(true),
            remaining_items: Vec::new(),
            error: Value::Null,
        },
        Err(err) => DeleteVerification {
            verified_deleted: Value::Null,
            remaining_items: Vec::new(),
            error: Value::String(err.to_string()),
        },
    }
}

fn is_absent_item_error(err: &KiCadError) -> bool {
    matches!(err, KiCadError::ApiStatus { code, .. } if code == "AS_BAD_REQUEST")
}

fn print_stackup_value(
    format: OutputFormat,
    stackup: BoardStackup,
    human: &str,
) -> anyhow::Result<()> {
    let value = serde_json::to_value(StackupJson::from(stackup))?;
    print_value(format, value, human)
}

fn read_json_arg<T: for<'de> Deserialize<'de>>(
    json: Option<&str>,
    file: Option<&std::path::PathBuf>,
) -> anyhow::Result<T> {
    let contents = match (json, file) {
        (Some(json), None) => json.to_string(),
        (None, Some(path)) => fs::read_to_string(path)
            .with_context(|| format!("failed to read `{}`", path.display()))?,
        (None, None) => bail!("provide --json or --file"),
        (Some(_), Some(_)) => unreachable!("clap conflicts_with enforces this"),
    };
    serde_json::from_str(&contents).context("failed to parse JSON")
}

fn read_raw_items(args: &RawItemsArgs) -> anyhow::Result<Vec<Any>> {
    let items: Vec<RawAnyJson> = read_json_arg(args.json.as_deref(), args.file.as_ref())?;
    items.into_iter().map(Any::try_from).collect()
}

fn read_raw_command(args: &RawCommandArgs) -> anyhow::Result<Any> {
    let command: RawAnyJson = read_json_arg(args.json.as_deref(), args.file.as_ref())?;
    command.try_into()
}

fn default_type_codes(type_codes: &[i32]) -> Vec<i32> {
    if type_codes.is_empty() {
        all_type_codes()
    } else {
        type_codes.to_vec()
    }
}

fn debug_values<T: std::fmt::Debug>(values: &[T]) -> Vec<String> {
    values.iter().map(|value| format!("{value:?}")).collect()
}

fn any_values(values: &[Any]) -> Vec<Value> {
    values.iter().map(any_value).collect()
}

fn any_value(value: &Any) -> Value {
    json!({
        "type_url": value.type_url,
        "value_hex": hex_encode(&value.value),
        "value_len": value.value.len(),
    })
}

fn board_text_spec_from_args(args: &CreateBoardTextArgs) -> BoardTextSpec {
    let mut spec = BoardTextSpec::new(
        args.text.clone(),
        Vector2Nm {
            x_nm: args.at.x_nm,
            y_nm: args.at.y_nm,
        },
        args.layer,
        Some(text_attributes_from_args(args)),
    );
    spec.text.hyperlink = args.hyperlink.clone();
    spec.knockout = args.knockout;
    spec.locked = if args.locked {
        ItemLockState::Locked
    } else {
        ItemLockState::Unlocked
    };
    spec
}

fn text_attributes_from_args(args: &CreateBoardTextArgs) -> TextAttributesSpec {
    TextAttributesSpec {
        font_name: args.font.clone(),
        horizontal_alignment: TextHorizontalAlignment::Unknown,
        vertical_alignment: TextVerticalAlignment::Unknown,
        angle_degrees: args.angle_degrees,
        line_spacing: args.line_spacing,
        stroke_width_nm: Some(args.stroke_width_nm),
        italic: args.italic,
        bold: args.bold,
        underlined: args.underlined,
        mirrored: args.mirrored,
        multiline: args.multiline,
        keep_upright: args.keep_upright,
        size_nm: Some(Vector2Nm {
            x_nm: args.width_nm.unwrap_or(args.height_nm),
            y_nm: args.height_nm,
        }),
    }
}

fn board_text_value(text: &PcbBoardText) -> Value {
    let item = PcbItem::BoardText(text.clone());
    json!(ItemSummary::from(&item))
}

fn appearance_value(settings: &BoardEditorAppearanceSettings) -> Value {
    json!({
        "inactive_layer_display": format!("{:?}", settings.inactive_layer_display),
        "net_color_display": format!("{:?}", settings.net_color_display),
        "board_flip": format!("{:?}", settings.board_flip),
        "ratsnest_display": format!("{:?}", settings.ratsnest_display),
    })
}

fn title_block_value(title: &TitleBlockInfo) -> Value {
    json!({
        "title": title.title,
        "date": title.date,
        "revision": title.revision,
        "company": title.company,
        "comments": title.comments,
    })
}

fn extents_value(extents: kicad_ipc_rs::TextExtents) -> Value {
    json!({
        "x_nm": extents.x_nm,
        "y_nm": extents.y_nm,
        "width_nm": extents.width_nm,
        "height_nm": extents.height_nm,
    })
}

#[derive(Debug, Deserialize)]
struct RawAnyJson {
    type_url: String,
    #[serde(default)]
    value_hex: String,
}

impl TryFrom<RawAnyJson> for Any {
    type Error = anyhow::Error;

    fn try_from(value: RawAnyJson) -> Result<Self, Self::Error> {
        Ok(Self {
            type_url: value.type_url,
            value: hex_decode(&value.value_hex)?,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BoardTextSpecsJson {
    Wrapped { board_texts: Vec<BoardTextJson> },
    Bare(Vec<BoardTextJson>),
}

impl BoardTextSpecsJson {
    fn into_specs(self) -> anyhow::Result<Vec<BoardTextSpec>> {
        let rows = match self {
            Self::Wrapped { board_texts } => board_texts,
            Self::Bare(board_texts) => board_texts,
        };
        rows.into_iter().map(BoardTextJson::into_spec).collect()
    }
}

#[derive(Debug, Deserialize)]
struct BoardTextJson {
    #[serde(default)]
    id: Option<String>,
    text: String,
    at: String,
    #[serde(default)]
    layer: Option<String>,
    #[serde(default)]
    layer_id: Option<i32>,
    #[serde(default)]
    knockout: bool,
    #[serde(default)]
    locked: bool,
    #[serde(default)]
    hyperlink: Option<String>,
    #[serde(default)]
    font: Option<String>,
    #[serde(default)]
    height: Option<String>,
    #[serde(default)]
    width: Option<String>,
    #[serde(default)]
    stroke_width: Option<String>,
    #[serde(default)]
    angle_degrees: Option<f64>,
    #[serde(default)]
    line_spacing: Option<f64>,
    #[serde(default)]
    bold: bool,
    #[serde(default)]
    italic: bool,
    #[serde(default)]
    underlined: bool,
    #[serde(default)]
    mirrored: bool,
    #[serde(default)]
    multiline: bool,
    #[serde(default)]
    keep_upright: bool,
    #[serde(default)]
    horizontal_alignment: Option<String>,
    #[serde(default)]
    vertical_alignment: Option<String>,
}

impl BoardTextJson {
    fn into_spec(self) -> anyhow::Result<BoardTextSpec> {
        let at = parse_point_nm(&self.at).map_err(anyhow::Error::msg)?;
        let layer_id = match (self.layer_id, self.layer.as_deref()) {
            (Some(layer_id), None) => layer_id,
            (None, Some(layer)) => parse_layer_id(layer).map_err(anyhow::Error::msg)?,
            (None, None) => parse_layer_id("F.SilkS").map_err(anyhow::Error::msg)?,
            (Some(_), Some(_)) => bail!("board text JSON cannot include both layer and layer_id"),
        };
        let height_nm = parse_distance_or_default(self.height.as_deref(), "1mm")?;
        let width_nm = match self.width.as_deref() {
            Some(width) => parse_distance_nm(width).map_err(anyhow::Error::msg)?,
            None => height_nm,
        };
        let stroke_width_nm = parse_distance_or_default(self.stroke_width.as_deref(), "0.15mm")?;
        let mut spec = BoardTextSpec::new(
            self.text,
            Vector2Nm {
                x_nm: at.x_nm,
                y_nm: at.y_nm,
            },
            layer_id,
            Some(TextAttributesSpec {
                font_name: self.font,
                horizontal_alignment: self
                    .horizontal_alignment
                    .as_deref()
                    .map(parse_horizontal_alignment)
                    .transpose()?
                    .unwrap_or(TextHorizontalAlignment::Unknown),
                vertical_alignment: self
                    .vertical_alignment
                    .as_deref()
                    .map(parse_vertical_alignment)
                    .transpose()?
                    .unwrap_or(TextVerticalAlignment::Unknown),
                angle_degrees: self.angle_degrees,
                line_spacing: self.line_spacing,
                stroke_width_nm: Some(stroke_width_nm),
                italic: self.italic,
                bold: self.bold,
                underlined: self.underlined,
                mirrored: self.mirrored,
                multiline: self.multiline,
                keep_upright: self.keep_upright,
                size_nm: Some(Vector2Nm {
                    x_nm: width_nm,
                    y_nm: height_nm,
                }),
            }),
        );
        spec.id = self.id;
        spec.text.hyperlink = self.hyperlink;
        spec.knockout = self.knockout;
        spec.locked = if self.locked {
            ItemLockState::Locked
        } else {
            ItemLockState::Unlocked
        };
        Ok(spec)
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum NetClassesJson {
    Wrapped { net_classes: Vec<NetClassJson> },
    Bare(Vec<NetClassJson>),
}

impl NetClassesJson {
    fn into_vec(self) -> Vec<NetClassJson> {
        match self {
            Self::Wrapped { net_classes } => net_classes,
            Self::Bare(net_classes) => net_classes,
        }
    }
}

#[derive(Debug, Deserialize)]
struct NetClassJson {
    name: String,
    priority: Option<i32>,
    #[serde(default = "default_explicit")]
    class_type: String,
    #[serde(default)]
    constituents: Vec<String>,
    board: Option<NetClassBoardJson>,
}

#[derive(Debug, Deserialize)]
struct NetClassBoardJson {
    #[serde(default, deserialize_with = "deserialize_opt_i64")]
    clearance_nm: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_opt_i64")]
    track_width_nm: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_opt_i64")]
    diff_pair_track_width_nm: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_opt_i64")]
    diff_pair_gap_nm: Option<i64>,
    #[serde(default, deserialize_with = "deserialize_opt_i64")]
    diff_pair_via_gap_nm: Option<i64>,
    color: Option<ColorJson>,
    tuning_profile: Option<String>,
    #[serde(default)]
    has_via_stack: bool,
    #[serde(default)]
    has_microvia_stack: bool,
}

impl TryFrom<NetClassJson> for NetClassInfo {
    type Error = anyhow::Error;

    fn try_from(value: NetClassJson) -> Result<Self, Self::Error> {
        Ok(Self {
            name: value.name,
            priority: value.priority,
            class_type: parse_net_class_type(&value.class_type)?,
            constituents: value.constituents,
            board: value.board.map(Into::into),
        })
    }
}

impl From<NetClassBoardJson> for NetClassBoardSettings {
    fn from(value: NetClassBoardJson) -> Self {
        Self {
            clearance_nm: value.clearance_nm,
            track_width_nm: value.track_width_nm,
            diff_pair_track_width_nm: value.diff_pair_track_width_nm,
            diff_pair_gap_nm: value.diff_pair_gap_nm,
            diff_pair_via_gap_nm: value.diff_pair_via_gap_nm,
            color: value.color.map(Into::into),
            tuning_profile: value.tuning_profile,
            has_via_stack: value.has_via_stack,
            has_microvia_stack: value.has_microvia_stack,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct StackupJson {
    finish_type_name: String,
    impedance_controlled: bool,
    edge_has_connector: bool,
    edge_has_castellated_pads: bool,
    edge_has_edge_plating: bool,
    layers: Vec<StackupLayerJson>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct StackupLayerJson {
    layer: LayerJson,
    user_name: String,
    material_name: String,
    enabled: bool,
    thickness_nm: Option<i64>,
    layer_type: String,
    color: Option<ColorJson>,
    #[serde(default)]
    dielectric_layers: Vec<DielectricJson>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct DielectricJson {
    epsilon_r: f64,
    loss_tangent: f64,
    material_name: String,
    thickness_nm: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct LayerJson {
    id: i32,
    name: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
struct ColorJson {
    r: f64,
    g: f64,
    b: f64,
    a: f64,
}

impl TryFrom<StackupJson> for BoardStackup {
    type Error = anyhow::Error;

    fn try_from(value: StackupJson) -> Result<Self, Self::Error> {
        Ok(Self {
            finish_type_name: value.finish_type_name,
            impedance_controlled: value.impedance_controlled,
            edge_has_connector: value.edge_has_connector,
            edge_has_castellated_pads: value.edge_has_castellated_pads,
            edge_has_edge_plating: value.edge_has_edge_plating,
            layers: value
                .layers
                .into_iter()
                .map(BoardStackupLayer::try_from)
                .collect::<anyhow::Result<Vec<_>>>()?,
        })
    }
}

impl TryFrom<StackupLayerJson> for BoardStackupLayer {
    type Error = anyhow::Error;

    fn try_from(value: StackupLayerJson) -> Result<Self, Self::Error> {
        Ok(Self {
            layer: BoardLayerInfo {
                id: value.layer.id,
                name: value.layer.name,
            },
            user_name: value.user_name,
            material_name: value.material_name,
            enabled: value.enabled,
            thickness_nm: value.thickness_nm,
            layer_type: parse_stackup_layer_type(&value.layer_type)?,
            color: value.color.map(Into::into),
            dielectric_layers: value
                .dielectric_layers
                .into_iter()
                .map(Into::into)
                .collect(),
        })
    }
}

impl From<DielectricJson> for BoardStackupDielectricProperties {
    fn from(value: DielectricJson) -> Self {
        Self {
            epsilon_r: value.epsilon_r,
            loss_tangent: value.loss_tangent,
            material_name: value.material_name,
            thickness_nm: value.thickness_nm,
        }
    }
}

impl From<BoardStackup> for StackupJson {
    fn from(value: BoardStackup) -> Self {
        Self {
            finish_type_name: value.finish_type_name,
            impedance_controlled: value.impedance_controlled,
            edge_has_connector: value.edge_has_connector,
            edge_has_castellated_pads: value.edge_has_castellated_pads,
            edge_has_edge_plating: value.edge_has_edge_plating,
            layers: value.layers.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<BoardStackupLayer> for StackupLayerJson {
    fn from(value: BoardStackupLayer) -> Self {
        Self {
            layer: LayerJson {
                id: value.layer.id,
                name: value.layer.name,
            },
            user_name: value.user_name,
            material_name: value.material_name,
            enabled: value.enabled,
            thickness_nm: value.thickness_nm,
            layer_type: format!("{:?}", value.layer_type),
            color: value.color.map(Into::into),
            dielectric_layers: value
                .dielectric_layers
                .into_iter()
                .map(Into::into)
                .collect(),
        }
    }
}

impl From<BoardStackupDielectricProperties> for DielectricJson {
    fn from(value: BoardStackupDielectricProperties) -> Self {
        Self {
            epsilon_r: value.epsilon_r,
            loss_tangent: value.loss_tangent,
            material_name: value.material_name,
            thickness_nm: value.thickness_nm,
        }
    }
}

impl From<ColorJson> for ColorRgba {
    fn from(value: ColorJson) -> Self {
        Self {
            r: value.r,
            g: value.g,
            b: value.b,
            a: value.a,
        }
    }
}

impl From<ColorRgba> for ColorJson {
    fn from(value: ColorRgba) -> Self {
        Self {
            r: value.r,
            g: value.g,
            b: value.b,
            a: value.a,
        }
    }
}

fn default_explicit() -> String {
    "explicit".to_string()
}

fn deserialize_opt_i64<'de, D>(deserializer: D) -> Result<Option<i64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(number)) => number
            .as_i64()
            .ok_or_else(|| serde::de::Error::custom("expected i64"))
            .map(Some),
        Some(Value::String(text)) if text == "-" || text.is_empty() => Ok(None),
        Some(Value::String(text)) => text
            .parse::<i64>()
            .map(Some)
            .map_err(serde::de::Error::custom),
        Some(other) => Err(serde::de::Error::custom(format!(
            "expected i64, string, null, or missing; got {other}"
        ))),
    }
}

fn parse_net_class_type(value: &str) -> anyhow::Result<NetClassType> {
    match normalized(value).as_str() {
        "explicit" => Ok(NetClassType::Explicit),
        "implicit" => Ok(NetClassType::Implicit),
        other => bail!("unknown net class type `{other}`"),
    }
}

fn parse_distance_or_default(value: Option<&str>, default: &str) -> anyhow::Result<i64> {
    parse_distance_nm(value.unwrap_or(default)).map_err(anyhow::Error::msg)
}

fn parse_horizontal_alignment(value: &str) -> anyhow::Result<TextHorizontalAlignment> {
    match normalized(value).replace(['-', '_'], "").as_str() {
        "unknown" => Ok(TextHorizontalAlignment::Unknown),
        "left" => Ok(TextHorizontalAlignment::Left),
        "center" | "centre" => Ok(TextHorizontalAlignment::Center),
        "right" => Ok(TextHorizontalAlignment::Right),
        "indeterminate" => Ok(TextHorizontalAlignment::Indeterminate),
        _ => bail!("unknown horizontal alignment `{value}`"),
    }
}

fn parse_vertical_alignment(value: &str) -> anyhow::Result<TextVerticalAlignment> {
    match normalized(value).replace(['-', '_'], "").as_str() {
        "unknown" => Ok(TextVerticalAlignment::Unknown),
        "top" => Ok(TextVerticalAlignment::Top),
        "center" | "centre" => Ok(TextVerticalAlignment::Center),
        "bottom" => Ok(TextVerticalAlignment::Bottom),
        "indeterminate" => Ok(TextVerticalAlignment::Indeterminate),
        _ => bail!("unknown vertical alignment `{value}`"),
    }
}

fn parse_stackup_layer_type(value: &str) -> anyhow::Result<BoardStackupLayerType> {
    match normalized(value).as_str() {
        "copper" => Ok(BoardStackupLayerType::Copper),
        "dielectric" => Ok(BoardStackupLayerType::Dielectric),
        "silkscreen" => Ok(BoardStackupLayerType::Silkscreen),
        "soldermask" | "solder_mask" | "solder-mask" => Ok(BoardStackupLayerType::SolderMask),
        "solderpaste" | "solder_paste" | "solder-paste" => Ok(BoardStackupLayerType::SolderPaste),
        "undefined" => Ok(BoardStackupLayerType::Undefined),
        other => bail!("unknown stackup layer type `{other}`"),
    }
}

fn normalized(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace(' ', "")
}

fn hex_decode(value: &str) -> anyhow::Result<Vec<u8>> {
    let clean = value.trim();
    if clean.len() % 2 != 0 {
        bail!("hex value must contain an even number of digits");
    }
    (0..clean.len())
        .step_by(2)
        .map(|idx| {
            u8::from_str_radix(&clean[idx..idx + 2], 16)
                .with_context(|| format!("invalid hex byte at offset {idx}"))
        })
        .collect()
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{BoardTextSpecsJson, RawAnyJson};
    use kicad_ipc_rs::{BoardLayerInfo, ItemLockState};
    use prost_types::Any;

    #[test]
    fn board_text_json_defaults_to_visible_front_silkscreen() {
        let specs: BoardTextSpecsJson = serde_json::from_str(
            r#"[{"text":"hello","at":"10mm,20mm","height":"1.2mm","stroke_width":"0.15mm"}]"#,
        )
        .expect("board text JSON should parse");
        let specs = specs.into_specs().expect("board text specs should build");

        assert_eq!(specs.len(), 1);
        let spec = &specs[0];
        assert_eq!(spec.id, None);
        assert_eq!(spec.layer_id, 40);
        assert_eq!(
            BoardLayerInfo::canonical_name_for_id(spec.layer_id).as_deref(),
            Some("F.SilkS")
        );
        assert_eq!(spec.text.text, "hello");
        assert_eq!(
            spec.text.position_nm.as_ref().map(|point| point.x_nm),
            Some(10_000_000)
        );
        assert_eq!(
            spec.text.position_nm.as_ref().map(|point| point.y_nm),
            Some(20_000_000)
        );
        let attrs = spec
            .text
            .attributes
            .as_ref()
            .expect("attributes should default");
        assert_eq!(
            attrs.size_nm.as_ref().map(|size| size.y_nm),
            Some(1_200_000)
        );
        assert_eq!(attrs.stroke_width_nm, Some(150_000));
        assert_eq!(spec.locked, ItemLockState::Unlocked);
    }

    #[test]
    fn raw_any_json_allows_empty_payload() {
        let raw: RawAnyJson = serde_json::from_str(
            r#"{"type_url":"type.googleapis.com/kiapi.common.commands.Ping"}"#,
        )
        .expect("raw Any JSON should parse");
        let any = Any::try_from(raw).expect("raw Any should build");

        assert_eq!(
            any.type_url,
            "type.googleapis.com/kiapi.common.commands.Ping"
        );
        assert!(any.value.is_empty());
    }
}
