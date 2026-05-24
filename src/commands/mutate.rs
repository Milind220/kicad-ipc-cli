use std::collections::BTreeSet;

use anyhow::{bail, Context};
use kicad_ipc_rs::{
    BoardEditorAppearanceSettings, BoardFlipMode, InactiveLayerDisplayMode, KiCadClientBlocking,
    NetColorDisplayMode, PcbItem, RatsnestDisplayMode, SelectionMutationResult, Vector2Nm,
};
use serde::Serialize;

use crate::cli::{
    DrcMarkerArgs, OutputFormat, SelectByIdArgs, SelectByNetArgs, SelectByRefArgs, SelectionMode,
    ViewPreset, ViewPresetArgs, ZonesRefillArgs,
};
use crate::model::{
    is_known_layer_name, item_id, CountRow, ItemSummary, LayerSummary, PointSummary,
};
use crate::output;

use super::all_type_codes;
use super::inspect::{ensure_board_open, read_all_decoded_pcb_items, resolve_nets};

pub fn select_add(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    item_ids: &[String],
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let result = client
        .add_to_selection(item_ids.to_vec())
        .context("failed to add items to selection")?;
    print_selection_mutation(format, "select add", item_ids.to_vec(), result)
}

pub fn select_remove(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    item_ids: &[String],
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let result = client
        .remove_from_selection(item_ids.to_vec())
        .context("failed to remove items from selection")?;
    print_selection_mutation(format, "select remove", item_ids.to_vec(), result)
}

pub fn select_clear(client: &KiCadClientBlocking, format: OutputFormat) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let result = client
        .clear_selection()
        .context("failed to clear selection")?;
    print_selection_mutation(format, "select clear", Vec::new(), result)
}

pub fn select_by_id(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &SelectByIdArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    apply_selection_mode(client, format, args.mode, args.item_ids.clone())
}

pub fn select_by_ref(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &SelectByRefArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let all_items = read_all_decoded_pcb_items(client)
        .context("failed to read PCB items for reference selection")?;
    let requested = args
        .refs
        .iter()
        .map(|reference| crate::groups::normalize_reference(reference))
        .collect::<BTreeSet<_>>();
    let matched_ids = all_items
        .iter()
        .filter_map(|item| match item {
            PcbItem::Footprint(footprint) => {
                let reference = footprint.reference.as_ref()?;
                if requested.contains(&crate::groups::normalize_reference(reference)) {
                    footprint.id.clone()
                } else {
                    None
                }
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    if matched_ids.is_empty() {
        bail!("no footprints matched the requested references");
    }

    apply_selection_mode(client, format, args.mode, matched_ids)
}

pub fn select_by_net(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &SelectByNetArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let nets = client.get_nets().context("failed to read nets")?;
    let selected_nets = resolve_nets(&nets, &args.nets)?;
    let items = client
        .get_items_by_net(all_type_codes(), selected_nets)
        .context("failed to read items by net")?;
    let matched_ids = ids_from_items(&items);

    if matched_ids.is_empty() {
        bail!("no selectable items matched the requested nets");
    }

    apply_selection_mode(client, format, args.mode, matched_ids)
}

pub fn view_active_layer(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    layer_id: Option<i32>,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let action = if let Some(layer_id) = layer_id {
        client
            .set_active_layer(layer_id)
            .with_context(|| format!("failed to set active layer to {layer_id}"))?;
        "set"
    } else {
        "show"
    };
    let active_layer = client
        .get_active_layer()
        .context("failed to read active layer")?;
    let report = ActiveLayerReport {
        action: action.to_string(),
        active_layer: LayerSummary::from(&active_layer),
    };

    output::print(format, &report, || {
        format!(
            "active layer {}\n{} ({})",
            report.action, report.active_layer.name, report.active_layer.id
        )
    })
}

pub fn view_visible_layers(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    layer_ids: &[i32],
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let action = if layer_ids.is_empty() {
        "show"
    } else {
        client
            .set_visible_layers(layer_ids.to_vec())
            .context("failed to set visible layers")?;
        "set"
    };
    let visible_layers = client
        .get_visible_layers()
        .context("failed to read visible layers")?;
    let enabled_layer_ids = client
        .get_board_enabled_layers()
        .context("failed to read enabled layers")?
        .layers
        .into_iter()
        .map(|layer| layer.id)
        .collect::<BTreeSet<_>>();
    let report = VisibleLayersReport {
        action: action.to_string(),
        visible_layers: visible_layers
            .iter()
            .filter(|layer| {
                enabled_layer_ids.contains(&layer.id) && is_known_layer_name(&layer.name)
            })
            .map(LayerSummary::from)
            .collect(),
    };

    output::print(format, &report, || {
        format!(
            "visible layers {}\ncount: {}",
            report.action,
            report.visible_layers.len()
        )
    })
}

pub fn view_preset(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &ViewPresetArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;

    let mut selected_item_ids = Vec::new();
    match args.preset {
        ViewPreset::AllLayers => {
            let enabled_layers = client
                .get_board_enabled_layers()
                .context("failed to read enabled layers")?;
            let layer_ids = enabled_layers
                .layers
                .iter()
                .map(|layer| layer.id)
                .collect::<Vec<_>>();
            client
                .set_visible_layers(layer_ids)
                .context("failed to show all enabled layers")?;
            set_appearance(
                client,
                InactiveLayerDisplayMode::Normal,
                NetColorDisplayMode::All,
                RatsnestDisplayMode::AllLayers,
            )?;
        }
        ViewPreset::FocusNet => {
            let Some(net) = &args.net else {
                bail!("view preset focus-net requires --net <name-or-code>");
            };
            let nets = client.get_nets().context("failed to read nets")?;
            let selected_nets = resolve_nets(&nets, std::slice::from_ref(net))?;
            let items = client
                .get_items_by_net(all_type_codes(), selected_nets)
                .context("failed to read net items for focus-net preset")?;
            selected_item_ids = ids_from_items(&items);
            if selected_item_ids.is_empty() {
                bail!("net `{net}` did not resolve to any selectable PCB items");
            }
            client
                .clear_selection()
                .context("failed to clear selection before focus-net preset")?;
            client
                .add_to_selection(selected_item_ids.clone())
                .context("failed to select focus-net items")?;
            set_appearance(
                client,
                InactiveLayerDisplayMode::Dimmed,
                NetColorDisplayMode::Ratsnest,
                RatsnestDisplayMode::AllLayers,
            )?;
        }
        ViewPreset::Ratsnest => {
            set_appearance(
                client,
                InactiveLayerDisplayMode::Dimmed,
                NetColorDisplayMode::Ratsnest,
                RatsnestDisplayMode::AllLayers,
            )?;
        }
    }

    let appearance = client
        .get_board_editor_appearance_settings()
        .context("failed to read appearance after preset")?;
    let visible_layers = client
        .get_visible_layers()
        .context("failed to read visible layers after preset")?;
    let report = ViewPresetReport {
        preset: format!("{:?}", args.preset).to_ascii_lowercase(),
        visible_layer_count: visible_layers.len(),
        appearance: AppearanceSummary::from(&appearance),
        selected_item_ids,
    };

    output::print(format, &report, || {
        format!(
            "view preset applied\npreset: {}\nvisible layers: {}\nselected items: {}",
            report.preset,
            report.visible_layer_count,
            report.selected_item_ids.len()
        )
    })
}

pub fn drc_marker(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &DrcMarkerArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let mut item_ids = args.item_ids.iter().cloned().collect::<BTreeSet<_>>();
    if args.selected {
        let selected = client
            .get_selection(Vec::new())
            .context("failed to read selection for DRC marker")?;
        item_ids.extend(ids_from_items(&selected));
    }
    let item_ids = item_ids.into_iter().collect::<Vec<_>>();
    let position = args.at.map(|point| Vector2Nm {
        x_nm: point.x_nm,
        y_nm: point.y_nm,
    });

    if position.is_none() && item_ids.is_empty() {
        bail!("drc-marker requires --at, --item-id, or --selected");
    }

    let marker_id = client
        .inject_drc_error(
            args.severity.into(),
            args.message.clone(),
            position,
            item_ids.clone(),
        )
        .context("failed to inject DRC marker")?;
    let report = DrcMarkerReport {
        marker_id,
        severity: format!("{:?}", args.severity).to_ascii_lowercase(),
        message: args.message.clone(),
        position: position.map(PointSummary::from),
        item_ids,
    };

    output::print(format, &report, || {
        format!(
            "drc marker injected\nmarker: {}\nitems: {}",
            report.marker_id.as_deref().unwrap_or("-"),
            report.item_ids.len()
        )
    })
}

pub fn zones_refill(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    args: &ZonesRefillArgs,
) -> anyhow::Result<()> {
    ensure_board_open(client)?;
    let selected_zone_ids = if args.selected {
        let selected = client
            .get_selection(Vec::new())
            .context("failed to read selected zones")?;
        selected
            .iter()
            .filter_map(|item| match item {
                PcbItem::Zone(_) => item_id(item).map(str::to_string),
                _ => None,
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let refill = prepare_zone_refill(&args.zone_ids, &selected_zone_ids, args.selected)?;

    if refill.zone_ids.is_empty() {
        client
            .refill_all_zones()
            .context("failed to refill all zones")?;
    } else {
        client
            .refill_zones(refill.zone_ids.clone())
            .context("failed to refill zones")?;
    }

    let report = ZonesRefillReport {
        target: refill.target,
        zone_ids: refill.zone_ids,
        action: "refill".to_string(),
    };
    output::print(format, &report, || {
        if report.zone_ids.is_empty() {
            "zones refill dispatched\nscope: all zones".to_string()
        } else {
            format!("zones refill dispatched\nzones: {}", report.zone_ids.len())
        }
    })
}

fn prepare_zone_refill(
    explicit_zone_ids: &[String],
    selected_zone_ids: &[String],
    include_selected: bool,
) -> anyhow::Result<ZoneRefillPlan> {
    let mut zone_ids = explicit_zone_ids
        .iter()
        .filter(|id| !id.trim().is_empty())
        .cloned()
        .collect::<BTreeSet<_>>();
    let explicit_count = zone_ids.len();
    let selected = selected_zone_ids
        .iter()
        .filter(|id| !id.trim().is_empty())
        .cloned()
        .collect::<BTreeSet<_>>();
    let selected_count = selected.len();

    if include_selected {
        if explicit_count == 0 && selected_count == 0 {
            bail!("--selected was set but the current selection contains no zones");
        }
        zone_ids.extend(selected);
    }

    let target = if include_selected && explicit_count > 0 && selected_count > 0 {
        "explicit+selected"
    } else if include_selected && selected_count > 0 {
        "selected"
    } else if zone_ids.is_empty() {
        "all"
    } else {
        "explicit"
    };

    Ok(ZoneRefillPlan {
        target: target.to_string(),
        zone_ids: zone_ids.into_iter().collect(),
    })
}

fn apply_selection_mode(
    client: &KiCadClientBlocking,
    format: OutputFormat,
    mode: SelectionMode,
    item_ids: Vec<String>,
) -> anyhow::Result<()> {
    let result = match mode {
        SelectionMode::Replace => {
            client
                .clear_selection()
                .context("failed to clear selection before replacing it")?;
            client
                .add_to_selection(item_ids.clone())
                .context("failed to add replacement selection")?
        }
        SelectionMode::Add => client
            .add_to_selection(item_ids.clone())
            .context("failed to add matching items to selection")?,
        SelectionMode::Remove => client
            .remove_from_selection(item_ids.clone())
            .context("failed to remove matching items from selection")?,
    };

    print_selection_mutation(
        format,
        &format!("select {}", format!("{mode:?}").to_ascii_lowercase()),
        item_ids,
        result,
    )
}

fn print_selection_mutation(
    format: OutputFormat,
    action: &str,
    item_ids: Vec<String>,
    result: SelectionMutationResult,
) -> anyhow::Result<()> {
    let selected_items = result
        .items
        .iter()
        .map(ItemSummary::from)
        .collect::<Vec<_>>();
    let report = SelectionMutationReport {
        action: action.to_string(),
        affected_item_ids: item_ids,
        selected_total: result.summary.total_items,
        type_counts: result
            .summary
            .type_url_counts
            .into_iter()
            .map(|entry| CountRow {
                name: entry.type_url,
                count: entry.count,
            })
            .collect(),
        selected_items,
    };

    output::print(format, &report, || {
        format!(
            "{}\naffected items: {}\nselected items: {}",
            report.action,
            report.affected_item_ids.len(),
            report.selected_total
        )
    })
}

fn ids_from_items(items: &[PcbItem]) -> Vec<String> {
    items
        .iter()
        .filter_map(item_id)
        .map(str::to_string)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn set_appearance(
    client: &KiCadClientBlocking,
    inactive_layer_display: InactiveLayerDisplayMode,
    net_color_display: NetColorDisplayMode,
    ratsnest_display: RatsnestDisplayMode,
) -> anyhow::Result<()> {
    client
        .set_board_editor_appearance_settings(BoardEditorAppearanceSettings {
            inactive_layer_display,
            net_color_display,
            board_flip: BoardFlipMode::Normal,
            ratsnest_display,
        })
        .context("failed to set board editor appearance")?;
    Ok(())
}

#[derive(Debug, Serialize)]
struct SelectionMutationReport {
    action: String,
    affected_item_ids: Vec<String>,
    selected_total: usize,
    type_counts: Vec<CountRow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    selected_items: Vec<ItemSummary>,
}

#[derive(Debug, Serialize)]
struct ActiveLayerReport {
    action: String,
    active_layer: LayerSummary,
}

#[derive(Debug, Serialize)]
struct VisibleLayersReport {
    action: String,
    visible_layers: Vec<LayerSummary>,
}

#[derive(Debug, Serialize)]
struct ViewPresetReport {
    preset: String,
    visible_layer_count: usize,
    appearance: AppearanceSummary,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    selected_item_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct AppearanceSummary {
    inactive_layer_display: String,
    net_color_display: String,
    board_flip: String,
    ratsnest_display: String,
}

impl From<&BoardEditorAppearanceSettings> for AppearanceSummary {
    fn from(value: &BoardEditorAppearanceSettings) -> Self {
        Self {
            inactive_layer_display: format!("{:?}", value.inactive_layer_display),
            net_color_display: format!("{:?}", value.net_color_display),
            board_flip: format!("{:?}", value.board_flip),
            ratsnest_display: format!("{:?}", value.ratsnest_display),
        }
    }
}

#[derive(Debug, Serialize)]
struct DrcMarkerReport {
    #[serde(skip_serializing_if = "Option::is_none")]
    marker_id: Option<String>,
    severity: String,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    position: Option<PointSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    item_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
struct ZonesRefillReport {
    action: String,
    target: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    zone_ids: Vec<String>,
}

#[derive(Debug, Eq, PartialEq)]
struct ZoneRefillPlan {
    target: String,
    zone_ids: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::prepare_zone_refill;

    #[test]
    fn selected_zone_refill_requires_selected_zones() {
        let err = prepare_zone_refill(&[], &[], true).expect_err("empty selection should fail");
        assert!(
            err.to_string().contains("contains no zones"),
            "unexpected error: {err:#}"
        );
    }

    #[test]
    fn empty_zone_refill_targets_all_zones_without_selected_flag() {
        let refill = prepare_zone_refill(&[], &[], false).expect("all zones should be valid");

        assert_eq!(refill.target, "all");
        assert!(refill.zone_ids.is_empty());
    }

    #[test]
    fn selected_zone_refill_uses_only_selected_zone_ids() {
        let refill = prepare_zone_refill(&[], &[zone("zone-b"), zone("zone-a")], true)
            .expect("selected zones should be valid");

        assert_eq!(refill.target, "selected");
        assert_eq!(refill.zone_ids, [zone("zone-a"), zone("zone-b")]);
    }

    #[test]
    fn explicit_and_selected_zone_ids_are_deduplicated() {
        let refill = prepare_zone_refill(
            &[zone("zone-a"), zone("zone-c")],
            &[zone("zone-a"), zone("zone-b")],
            true,
        )
        .expect("explicit plus selected zones should be valid");

        assert_eq!(refill.target, "explicit+selected");
        assert_eq!(
            refill.zone_ids,
            [zone("zone-a"), zone("zone-b"), zone("zone-c")]
        );
    }

    fn zone(value: &str) -> String {
        value.to_string()
    }
}
