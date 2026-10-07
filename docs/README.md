# Tack documentation

Start with the [public overview](../README.md) for features and everyday controls.

## Using and building Tack

- [Build, troubleshooting and developer checks](DEVELOPMENT.md)
- [Feature inventory](FEATURE_INVENTORY.md)
- [Current UI/clipboard checklist](HUMAN_TEST_1J.md)
- [About checklist](HUMAN_TEST_1K_ABOUT.md)
- [Contribution guidelines](../CONTRIBUTING.md)
- [Dependency and font notices](THIRD_PARTY_NOTICES.md)

## Architecture and design

- [Architecture](architecture.md)
- [Board format and compatibility](design/tack_document_compatibility.md)
- [Local files, recovery and preferences](design/local_production_hardening.md)
- [Image interaction](design/image_interaction.md)
- [Spatial organization](design/spatial_organization.md)
- [Annotations](design/annotation_objects.md)
- [Board arrangement](design/board_arrangement.md)
- [Keyboard shortcuts](design/keyboard_shortcuts.md)
- [Native UI polish and clipboard policy](design/ui_polish.md)
- [About modal and package](design/about_modal.md)
- [Progressive image supply](design/local_image_supply.md)

## Development reports

Reports describe the code and evidence at their recorded checkpoint. Later
reports supersede earlier feature status, limitations and stop conditions.

| Checkpoint | Focus |
| --- | --- |
| [1K About](MISSION_1K_ABOUT_REPORT.md) | Editable metadata, compact on-demand modal and package |
| [1J](MISSION_1J_REPORT.md) · [independent verification](VERIFICATION_1J.md) | Clipboard, unified menus, UI scale and themes |
| [1I](MISSION_1I_REPORT.md) · [independent verification](VERIFICATION_1I.md) | Layout-aware shortcuts, one-shot tools and arrangement |
| [1H](MISSION_1H_REPORT.md) | Contextual UI and history integration |
| [1G](MISSION_1G_REPORT.md) · [independent verification](VERIFICATION_1G.md) | Performance and progressive image supply |
| [1F](MISSION_1F_REPORT.md) | Local files, recovery and preferences |
| [1E](MISSION_1E_REPORT.md) | Annotations and linked sources |
| [1D](MISSION_1D_REPORT.md) | Spatial organization |
| [1C](MISSION_1C_REPORT.md) | Image manipulation |
| [1B](MISSION_1B_REPORT.md) | Local board persistence |
| [1A](MISSION_1A_REPORT.md) | Document and input foundations |
| [0.7](MISSION_0_7_REPORT.md) | Charged overview preparation |
| [0.6](MISSION_0_6_REPORT.md) | Asset pipeline experiments |
| [0.5](MISSION_0_5_REPORT.md) | Streaming and navigation |
| [First prototype](MISSION_0_REPORT.md) | Initial native renderer |

## Research and measurements

- [Renderer research](research/renderer_prior_art.md)
- [Asset pipeline experiments](research/asset_pipeline_experiments.md)
- [Overview preparation experiment](research/overview_preparation_experiment.md)
- [Native thumbnail decoder review](research/native_thumbnail_decoder.md)
- [Published benchmark receipts](../benchmarks/)
- [X11 startup diagnosis](X11_STARTUP.md)
- [Desktop freeze incident](INCIDENT_2026_10_03_DESKTOP_FREEZE.md)
