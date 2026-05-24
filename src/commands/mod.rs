mod groups;
mod inspect;
mod mutate;

use anyhow::bail;

use crate::cli::{Cli, Command, ComponentGroupsCommand, SelectCommand, ViewCommand, ZonesCommand};

pub fn dispatch(cli: Cli) -> anyhow::Result<()> {
    let client = crate::client::connect(&cli)?;

    match &cli.command {
        Command::Doctor => inspect::doctor(&client, cli.format),
        Command::BoardSummary => inspect::board_summary(&client, cli.format),
        Command::Inventory => inspect::inventory(&client, cli.format),
        Command::Selection(args) => inspect::selection(&client, cli.format, args),
        Command::NetReport(args) => inspect::net_report(&client, cli.format, args),
        Command::ComponentGroups(args) => match &args.command {
            ComponentGroupsCommand::Suggest(args) => groups::suggest(&client, cli.format, args),
            ComponentGroupsCommand::Apply(args) => {
                require_yes(cli.yes, "component-groups apply")?;
                groups::apply(&client, cli.format, args)
            }
        },
        Command::Select(args) => match &args.command {
            SelectCommand::Add(args) => mutate::select_add(&client, cli.format, &args.item_ids),
            SelectCommand::Remove(args) => {
                mutate::select_remove(&client, cli.format, &args.item_ids)
            }
            SelectCommand::Clear => mutate::select_clear(&client, cli.format),
            SelectCommand::ByRef(args) => mutate::select_by_ref(&client, cli.format, args),
            SelectCommand::ByNet(args) => mutate::select_by_net(&client, cli.format, args),
        },
        Command::View(args) => match &args.command {
            ViewCommand::ActiveLayer { layer_id } => {
                mutate::view_active_layer(&client, cli.format, *layer_id)
            }
            ViewCommand::VisibleLayers { layer_ids } => {
                mutate::view_visible_layers(&client, cli.format, layer_ids)
            }
            ViewCommand::Preset(args) => mutate::view_preset(&client, cli.format, args),
        },
        Command::DrcMarker(args) => {
            require_yes(cli.yes, "drc-marker")?;
            mutate::drc_marker(&client, cli.format, args)
        }
        Command::Zones(args) => match &args.command {
            ZonesCommand::Refill(args) => {
                require_yes(cli.yes, "zones refill")?;
                mutate::zones_refill(&client, cli.format, args)
            }
        },
        Command::Snapshot(args) => inspect::snapshot(&client, cli.format, args),
        Command::TextShapes(args) => inspect::text_shapes(&client, cli.format, args),
    }
}

fn require_yes(yes: bool, command: &str) -> anyhow::Result<()> {
    if yes {
        Ok(())
    } else {
        bail!("`{command}` modifies the board; rerun with --yes to confirm")
    }
}

fn all_type_codes() -> Vec<i32> {
    kicad_ipc_rs::KiCadClient::pcb_object_type_codes()
        .iter()
        .map(|entry| entry.code)
        .collect()
}
