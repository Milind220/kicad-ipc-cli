mod api;
mod groups;
mod inspect;
mod mutate;

use anyhow::bail;

use crate::cli::{
    ApiBoardCommand, ApiCommand, ApiCommonCommand, ApiDocumentCommand, ApiItemsCommand,
    ApiRawCommand, ApiSelectionCommand, Cli, Command, SelectCommand, ViewCommand, ZonesCommand,
};

pub fn dispatch(cli: Cli) -> anyhow::Result<()> {
    if let Command::Api(args) = &cli.command {
        if matches!(args.command, ApiCommand::List) {
            return api::list(cli.format);
        }
    }

    if let Some(command) = confirmation_requirement(&cli.command) {
        require_yes(cli.yes, command)?;
    }

    let client = crate::client::connect(&cli)?;

    match &cli.command {
        Command::Doctor => inspect::doctor(&client, cli.format),
        Command::BoardSummary => inspect::board_summary(&client, cli.format),
        Command::Inventory(args) => inspect::inventory(&client, cli.format, args),
        Command::Selection(args) => inspect::selection(&client, cli.format, args),
        Command::NetReport(args) => inspect::net_report(&client, cli.format, args),
        Command::ComponentGroups(args) => match &args.command {
            crate::cli::ComponentGroupsCommand::Apply(args) => {
                require_yes(cli.yes, "component-groups apply")?;
                groups::apply(&client, cli.format, args)
            }
        },
        Command::Select(args) => match &args.command {
            SelectCommand::ById(args) => mutate::select_by_id(&client, cli.format, args),
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
        Command::Api(args) => match &args.command {
            ApiCommand::List => unreachable!("api list returns before connecting"),
            ApiCommand::Common(args) => match &args.command {
                ApiCommonCommand::Ping => api::ping(&client, cli.format),
                ApiCommonCommand::Version => api::version(&client, cli.format),
                ApiCommonCommand::ProjectPath => api::project_path(&client, cli.format),
                ApiCommonCommand::HasOpenBoard => api::has_open_board(&client, cli.format),
                ApiCommonCommand::OpenDocuments(args) => {
                    api::open_documents(&client, cli.format, args)
                }
                ApiCommonCommand::KicadBinaryPath(args) => {
                    api::kicad_binary_path(&client, cli.format, args)
                }
                ApiCommonCommand::PluginSettingsPath(args) => {
                    api::plugin_settings_path(&client, cli.format, args)
                }
                ApiCommonCommand::Refresh(args) => api::refresh(&client, cli.format, args),
                ApiCommonCommand::RunAction(args) => api::run_action(&client, cli.format, args),
                ApiCommonCommand::NetClasses => api::net_classes(&client, cli.format),
                ApiCommonCommand::SetNetClasses(args) => {
                    require_yes(cli.yes, "api common set-net-classes")?;
                    api::set_net_classes(&client, cli.format, args)
                }
                ApiCommonCommand::TextVariablesGet => api::text_variables_get(&client, cli.format),
                ApiCommonCommand::TextVariablesSet(args) => {
                    require_yes(cli.yes, "api common text-variables-set")?;
                    api::text_variables_set(&client, cli.format, args)
                }
                ApiCommonCommand::ExpandTextVariables(args) => {
                    api::expand_text_variables(&client, cli.format, args)
                }
                ApiCommonCommand::TextExtents(args) => api::text_extents(&client, cli.format, args),
                ApiCommonCommand::TextAsShapes(args) => {
                    api::text_as_shapes(&client, cli.format, args)
                }
            },
            ApiCommand::Board(args) => match &args.command {
                ApiBoardCommand::Nets => api::board_nets(&client, cli.format),
                ApiBoardCommand::EnabledLayers => api::enabled_layers(&client, cli.format),
                ApiBoardCommand::SetEnabledLayers(args) => {
                    require_yes(cli.yes, "api board set-enabled-layers")?;
                    api::set_enabled_layers(&client, cli.format, args)
                }
                ApiBoardCommand::ActiveLayer => api::active_layer(&client, cli.format),
                ApiBoardCommand::SetActiveLayer(args) => {
                    api::set_active_layer(&client, cli.format, args)
                }
                ApiBoardCommand::VisibleLayers => api::visible_layers(&client, cli.format),
                ApiBoardCommand::SetVisibleLayers(args) => {
                    api::set_visible_layers(&client, cli.format, args)
                }
                ApiBoardCommand::LayerName(args) => api::layer_name(&client, cli.format, args),
                ApiBoardCommand::Origin(args) => api::origin(&client, cli.format, args),
                ApiBoardCommand::SetOrigin(args) => {
                    require_yes(cli.yes, "api board set-origin")?;
                    api::set_origin(&client, cli.format, args)
                }
                ApiBoardCommand::Stackup => api::stackup(&client, cli.format),
                ApiBoardCommand::UpdateStackup(args) => {
                    require_yes(cli.yes, "api board update-stackup")?;
                    api::update_stackup(&client, cli.format, args)
                }
                ApiBoardCommand::GraphicsDefaults => api::graphics_defaults(&client, cli.format),
                ApiBoardCommand::Appearance => api::appearance(&client, cli.format),
                ApiBoardCommand::SetAppearance(args) => {
                    api::set_appearance(&client, cli.format, args)
                }
                ApiBoardCommand::InteractiveMoveItems(args) => {
                    api::interactive_move_items(&client, cli.format, args)
                }
                ApiBoardCommand::Items(args) => api::board_items(&client, cli.format, args),
                ApiBoardCommand::ItemsByNet(args) => api::items_by_net(&client, cli.format, args),
                ApiBoardCommand::ItemsByNetClass(args) => {
                    api::items_by_net_class(&client, cli.format, args)
                }
                ApiBoardCommand::ConnectedItems(args) => {
                    api::connected_items(&client, cli.format, args)
                }
                ApiBoardCommand::NetclassForNets(args) => {
                    api::netclass_for_nets(&client, cli.format, args)
                }
                ApiBoardCommand::PadShapeAsPolygon(args) => {
                    api::pad_shape_as_polygon(&client, cli.format, args)
                }
                ApiBoardCommand::PadstackPresence(args) => {
                    api::padstack_presence(&client, cli.format, args)
                }
                ApiBoardCommand::InjectDrcError(args) => {
                    require_yes(cli.yes, "api board inject-drc-error")?;
                    mutate::drc_marker(&client, cli.format, args)
                }
                ApiBoardCommand::BoundingBoxes(args) => {
                    api::bounding_boxes(&client, cli.format, args)
                }
                ApiBoardCommand::HitTest(args) => api::hit_test(&client, cli.format, args),
                ApiBoardCommand::RefillZones(args) => {
                    require_yes(cli.yes, "api board refill-zones")?;
                    api::refill_zones(&client, cli.format, args)
                }
            },
            ApiCommand::Selection(args) => match &args.command {
                ApiSelectionCommand::Summary(args) => {
                    api::selection_summary(&client, cli.format, args)
                }
                ApiSelectionCommand::Get(args) => api::selection_get(&client, cli.format, args),
                ApiSelectionCommand::Details(args) => {
                    api::selection_details(&client, cli.format, args)
                }
                ApiSelectionCommand::Add(args) => api::selection_add(&client, cli.format, args),
                ApiSelectionCommand::Remove(args) => {
                    api::selection_remove(&client, cli.format, args)
                }
                ApiSelectionCommand::Clear => api::selection_clear(&client, cli.format),
            },
            ApiCommand::Items(args) => match &args.command {
                ApiItemsCommand::BeginCommit => api::begin_commit(&client, cli.format),
                ApiItemsCommand::EndCommit(args) => api::end_commit(&client, cli.format, args),
                ApiItemsCommand::CreateBoardText(args) => {
                    require_yes(cli.yes, "api items create-board-text")?;
                    api::create_board_text(&client, cli.format, args)
                }
                ApiItemsCommand::CreateBoardTexts(args) => {
                    require_yes(cli.yes, "api items create-board-texts")?;
                    api::create_board_texts(&client, cli.format, args)
                }
                ApiItemsCommand::CreateRaw(args) => {
                    require_yes(cli.yes, "api items create-raw")?;
                    api::create_raw(&client, cli.format, args)
                }
                ApiItemsCommand::UpdateRaw(args) => {
                    require_yes(cli.yes, "api items update-raw")?;
                    api::update_raw(&client, cli.format, args)
                }
                ApiItemsCommand::ParseCreate(args) => {
                    require_yes(cli.yes, "api items parse-create")?;
                    api::parse_create(&client, cli.format, args)
                }
                ApiItemsCommand::Delete(args) => {
                    require_yes(cli.yes, "api items delete")?;
                    api::delete_items(&client, cli.format, args)
                }
                ApiItemsCommand::GetById(args) => api::get_items_by_id(&client, cli.format, args),
                ApiItemsCommand::GetEditableById(args) => {
                    api::get_editable_items_by_id(&client, cli.format, args)
                }
            },
            ApiCommand::Raw(args) => match &args.command {
                ApiRawCommand::Send(args) => api::raw_send(&client, cli.format, args),
            },
            ApiCommand::Document(args) => match &args.command {
                ApiDocumentCommand::TitleBlock => api::title_block(&client, cli.format),
                ApiDocumentCommand::SetTitleBlock(args) => {
                    require_yes(cli.yes, "api document set-title-block")?;
                    api::set_title_block(&client, cli.format, args)
                }
                ApiDocumentCommand::Save => {
                    require_yes(cli.yes, "api document save")?;
                    api::save_document(&client, cli.format)
                }
                ApiDocumentCommand::SaveCopy(args) => api::save_copy(&client, cli.format, args),
                ApiDocumentCommand::Revert => {
                    require_yes(cli.yes, "api document revert")?;
                    api::revert_document(&client, cli.format)
                }
                ApiDocumentCommand::BoardString => api::board_string(&client, cli.format),
                ApiDocumentCommand::SelectionString => api::selection_string(&client, cli.format),
            },
        },
    }
}

fn require_yes(yes: bool, command: &str) -> anyhow::Result<()> {
    if yes {
        Ok(())
    } else {
        bail!("`{command}` modifies the board; rerun with --yes to confirm")
    }
}

fn confirmation_requirement(command: &Command) -> Option<&'static str> {
    match command {
        Command::ComponentGroups(args) => match &args.command {
            crate::cli::ComponentGroupsCommand::Apply(_) => Some("component-groups apply"),
        },
        Command::DrcMarker(_) => Some("drc-marker"),
        Command::Zones(args) => match &args.command {
            ZonesCommand::Refill(_) => Some("zones refill"),
        },
        Command::Api(args) => match &args.command {
            ApiCommand::Common(args) => match &args.command {
                ApiCommonCommand::RunAction(_) => Some("api common run-action"),
                ApiCommonCommand::SetNetClasses(_) => Some("api common set-net-classes"),
                ApiCommonCommand::TextVariablesSet(_) => Some("api common text-variables-set"),
                _ => None,
            },
            ApiCommand::Board(args) => match &args.command {
                ApiBoardCommand::SetEnabledLayers(_) => Some("api board set-enabled-layers"),
                ApiBoardCommand::SetOrigin(_) => Some("api board set-origin"),
                ApiBoardCommand::UpdateStackup(_) => Some("api board update-stackup"),
                ApiBoardCommand::InjectDrcError(_) => Some("api board inject-drc-error"),
                ApiBoardCommand::RefillZones(_) => Some("api board refill-zones"),
                _ => None,
            },
            ApiCommand::Items(args) => match &args.command {
                ApiItemsCommand::BeginCommit => Some("api items begin-commit"),
                ApiItemsCommand::EndCommit(_) => Some("api items end-commit"),
                ApiItemsCommand::CreateBoardText(_) => Some("api items create-board-text"),
                ApiItemsCommand::CreateBoardTexts(_) => Some("api items create-board-texts"),
                ApiItemsCommand::CreateRaw(_) => Some("api items create-raw"),
                ApiItemsCommand::UpdateRaw(_) => Some("api items update-raw"),
                ApiItemsCommand::ParseCreate(_) => Some("api items parse-create"),
                ApiItemsCommand::Delete(_) => Some("api items delete"),
                _ => None,
            },
            ApiCommand::Raw(args) => match &args.command {
                ApiRawCommand::Send(_) => Some("api raw send"),
            },
            ApiCommand::Document(args) => match &args.command {
                ApiDocumentCommand::SetTitleBlock(_) => Some("api document set-title-block"),
                ApiDocumentCommand::Save => Some("api document save"),
                ApiDocumentCommand::SaveCopy(_) => Some("api document save-copy"),
                ApiDocumentCommand::Revert => Some("api document revert"),
                _ => None,
            },
            _ => None,
        },
        _ => None,
    }
}

fn all_type_codes() -> Vec<i32> {
    kicad_ipc_rs::KiCadClient::pcb_object_type_codes()
        .iter()
        .map(|entry| entry.code)
        .collect()
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use crate::cli::Cli;

    use super::confirmation_requirement;

    #[test]
    fn confirmation_gate_covers_raw_send_before_connection() {
        let cli = Cli::parse_from([
            "kicad-ipc-cli",
            "api",
            "raw",
            "send",
            "--json",
            r#"{"type_url":"type.googleapis.com/kiapi.common.commands.Ping"}"#,
        ]);

        assert_eq!(confirmation_requirement(&cli.command), Some("api raw send"));
    }

    #[test]
    fn confirmation_gate_covers_action_save_copy_and_commits() {
        for (argv, expected) in [
            (
                vec!["kicad-ipc-cli", "api", "common", "run-action", "foo"],
                "api common run-action",
            ),
            (
                vec![
                    "kicad-ipc-cli",
                    "api",
                    "document",
                    "save-copy",
                    "copy.kicad_pcb",
                ],
                "api document save-copy",
            ),
            (
                vec!["kicad-ipc-cli", "api", "items", "begin-commit"],
                "api items begin-commit",
            ),
            (
                vec![
                    "kicad-ipc-cli",
                    "api",
                    "items",
                    "end-commit",
                    "--session-id",
                    "s1",
                    "--action",
                    "drop",
                ],
                "api items end-commit",
            ),
        ] {
            let cli = Cli::parse_from(argv);

            assert_eq!(confirmation_requirement(&cli.command), Some(expected));
        }
    }
}
