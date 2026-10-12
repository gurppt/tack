# Mission UI Cleanup — toolbar et signets

Le nettoyage est terminé et vérifié sur le worktree dérivé de `2f007d3fab954a00c54028be662eb98a7c285599` (branche dev). Les corrections de sélection multiple 2A12A sont conservées. Le README modifié par l’utilisateur et les autres fichiers non suivis sont conservés. Les trois briefs suivants ont été explicitement autorisés par l’utilisateur, dans l’ordre P0, supply, batching ; ce rapport clôt cette mission avant leur démarrage.

## Résultat

- Une seule politique `toolbar::eligible` exclut les 56 actions ci-dessous du catalogue et de la normalisation des profils, sans les retirer du dispatcher ni du keymap. Les 13 boutons par défaut sont inchangés.
- Rotate View, son outil temporaire, ses identifiants et son icône exclusive sont supprimés. La rotation des images reste disponible.
- Note Alignment reste optionnel, hors configuration par défaut : quatre glyphes distincts gauche/centre/droite/mixte, état courant et raccourci dans le tooltip. Le cache dépend du document, de sa génération, de la révision de sélection et de l’édition de note. Aucun timer ajouté ; atlas toujours 128×80.
- Les quatre ajustements de propriété ont maintenant une route dans le menu des annotations/notes ; Snap to grid est accessible aussi pour une sélection simple.
- Camera bookmarks a un bouton visible « Add current view... (Insert) », y compris sur une liste vide. Création, renommage, annulation, suppression, Undo/Redo et persistance utilisent les handlers existants. Les modifications restent désactivées en mode partagé. Les noms dupliqués restent autorisés par le validateur existant ; aucune migration.
- Les captures de vue locales B/0…9 sont distinguées des signets enregistrés dans le document. Leur action est intitulée Assign view shortcut.
- La bande de révélation/maintien de la barre supérieure passe à 24 pixels logiques. Les boutons de la toolbar restent cliquables lorsqu’elle la chevauche ; la bande de resize de 3 pixels conserve sa priorité. La barre est dessinée sous la toolbar.
- Finish Scribble reste Enter, Eraser reste E, Align Text reste Ctrl+Shift+E. Shift+E n’est pas assigné par défaut. Enter termine le Scribble composé une fois ; une note utilise Shift+Enter pour sauter une ligne et Enter pour valider. Enter ne valide plus pendant une composition IME active.

## Matrice de joignabilité

« Route/catalogue » signifie que les tests vérifient la présence de l’action dans le catalogue et son chemin de menu/panneau ; cela ne prétend pas avoir exécuté les 56 opérations externes (exports, mises à jour, lancement d’un éditeur source, etc.). Les conditions de sélection/mode restent applicables. Les opérations Source exigent notamment une image liée. Les six routes spécifiques de panneau sont exercées au niveau du catalogue/contrôleur.

| Action | Chemin conservé | Keymap éditable | Raccourci par défaut | Vérification |
|---|---|---|---|---|
| SelectAll | Tack > Edit | Oui | Ctrl+A | Route/catalogue |
| DeleteSelection | Tack > Edit | Oui | Del | Route/catalogue |
| Opacity(Full) | Image > Opacity/Sampling; Selection > Image (2A12A) | Oui | — | Route/catalogue |
| Opacity(ThreeQuarters) | Image > Opacity/Sampling; Selection > Image (2A12A) | Oui | — | Route/catalogue |
| Opacity(Half) | Image > Opacity/Sampling; Selection > Image (2A12A) | Oui | — | Route/catalogue |
| Opacity(Quarter) | Image > Opacity/Sampling; Selection > Image (2A12A) | Oui | — | Route/catalogue |
| Filtering(Default) | Image > Opacity/Sampling; Selection > Image (2A12A) | Oui | — | Route/catalogue |
| Filtering(Smooth) | Image > Opacity/Sampling; Selection > Image (2A12A) | Oui | — | Route/catalogue |
| Filtering(Nearest) | Image > Opacity/Sampling; Selection > Image (2A12A) | Oui | — | Route/catalogue |
| ContextCoarseIncrease | MISSING at baseline; proposed Selection > Adjust property | Oui | Shift+pos:Code(NumpadAdd) | Route/catalogue |
| ContextCoarseDecrease | MISSING at baseline; proposed Selection > Adjust property | Oui | Shift+pos:Code(NumpadSubtract) | Route/catalogue |
| ContextIncrease | MISSING at baseline; proposed Selection > Adjust property | Oui | pos:Code(NumpadAdd) | Route/catalogue |
| ContextDecrease | MISSING at baseline; proposed Selection > Adjust property | Oui | pos:Code(NumpadSubtract) | Route/catalogue |
| Layout(SnapToGrid) | Selection > Arrange > Snap to grid; single-object route MISSING | Oui | — | Route/catalogue |
| FinishScribble | Tack > Tools > Finish Scribble | Oui | Enter | Route/catalogue |
| RenameFrame | Frame/Note context > Rename / edit | Oui | F2 | Route/catalogue |
| AnnotationStyle(Fill) | Note/Text or Annotation > Style | Oui | F | Route/catalogue |
| AnnotationStyle(Wider) | Note/Text or Annotation > Style | Oui | — | Route/catalogue |
| AnnotationStyle(Narrower) | Note/Text or Annotation > Style | Oui | — | Route/catalogue |
| AnnotationStyle(LargerText) | Note/Text or Annotation > Style | Oui | Ctrl+Shift+. | Route/catalogue |
| AnnotationStyle(SmallerText) | Note/Text or Annotation > Style | Oui | Ctrl+Shift+, | Route/catalogue |
| AnnotationStyle(OpacityUp) | Note/Text or Annotation > Style | Oui | Shift+] | Route/catalogue |
| AnnotationStyle(OpacityDown) | Note/Text or Annotation > Style | Oui | Shift+[ | Route/catalogue |
| OpenSource | Single image context > Source or Image information | Oui | Ctrl+Shift+O | Route/catalogue |
| RevealSource | Single image context > Source or Image information | Oui | Ctrl+Alt+O | Route/catalogue |
| CopySourcePath | Single image context > Source or Image information | Oui | Ctrl+Shift+C | Route/catalogue |
| About | Tack > About Tack | Oui | — | Route/catalogue |
| CheckForUpdates | Tack > Update... | Oui | — | Route/catalogue |
| NewBoard | Tack > File | Oui | Ctrl+N | Route/catalogue |
| CloseBoard | Tack > File | Oui | — | Route/catalogue |
| ImportImages | Tack > File | Oui | Ctrl+I | Route/catalogue |
| SaveAs | Tack > File | Oui | Ctrl+Shift+S | Route/catalogue |
| SaveToLocal | Tack > File | Oui | — | Route/catalogue |
| Paste | Tack > File | Oui | Ctrl+V | Route/catalogue |
| RelinkSource | Single image context > Relink | Oui | Ctrl+Shift+R | Route/catalogue |
| SaveOriginalAs | Single image context > Source or Image information | Oui | — | Route/catalogue |
| ToggleAlwaysOnTop | Tack > Window | Oui | — | Route/catalogue |
| ToggleNativeTitleBar | Tack > Window | Oui | — | Route/catalogue |
| MinimizeWindow | Tack > Window | Oui | — | Route/catalogue |
| MaximizeWindow | Tack > Window | Oui | — | Route/catalogue |
| ExportSelectedSources | Tack > File | Oui | — | Route/catalogue |
| ExportAllSources | Tack > File | Oui | — | Route/catalogue |
| Preferences | Tack > Preferences | Oui | Ctrl+, | Route/catalogue |
| KeymapEditor | Tack > Edit keymap | Oui | — | Route/catalogue |
| RecentBoards | Tack > File | Oui | — | Route/catalogue |
| ImportKeymap | Tack > Keymap > Import / Export / Save keyset | Oui | — | Route/catalogue |
| ExportKeymap | Tack > Keymap > Import / Export / Save keyset | Oui | — | Route/catalogue |
| SaveKeymap | Tack > Keymap > Import / Export / Save keyset | Oui | — | Route/catalogue |
| ExportPreferences | Tack > Preferences > Export | Oui | — | Route/catalogue |
| ExportProfile | Tack > Edit Toolbar > Export / Import Profile | Oui | — | Route/catalogue |
| ImportProfile | Tack > Edit Toolbar > Export / Import Profile | Oui | — | Route/catalogue |
| AddCameraBookmark | Tack > View | Oui | B | Route/catalogue |
| SourceInfo | Single image context > Source or Image information | Oui | — | Route/catalogue |
| ToggleToolbar | Tack > View | Oui | F9 | Route/catalogue |
| EditToolbar | Tack > Tools > Edit Toolbar | Oui | — | Route/catalogue |
| ToggleStatusBar | Tack > View | Oui | — | Route/catalogue |

## Utilisation des signets

Ouvrir Camera bookmarks, cliquer Add current view ou presser Insert, saisir le nom et valider. Un signet stocke centre et zoom dans le board. Jump revient à cette vue ; Rename/Delete et Undo/Redo restent disponibles. Pour une capture locale rapide, utiliser Assign view shortcut puis B/0…9 : ce profil local est indépendant des signets du board.

## Validation et preuves

- Workspace `--all-features` : **486 tests réussis, 0 échec, 20 ignorés** (GPU/fixtures explicitement spécialisés). Clippy all-targets/all-features avec `-D warnings`, rustfmt et diff check réussis.
- Application release sur Xvfb privé avec Vulkan logiciel : **52 contrôles natifs réussis**, à 800×600 et 1024×768. Signets vides, création, deux vues distinctes, jump, rename/delete, Undo/Redo, sauvegarde/réouverture, vue locale indépendante, note multiligne, alignement à la souris et clavier, disposition XKB française, Scribble composé, Enter/Shift+E/E.
- **Quatre scénarios natifs supplémentaires** : deux tailles × scale 1/2. La limite 23/24 pixels logiques, le déplacement par la barre sans modifier l’image, la sélection du canvas et l’ouverture du vrai catalogue Edit Toolbar passent.
- Deux fenêtres réellement stabilisées : **0 tick CPU et 0 redraw** pendant les intervalles mesurés de 3 secondes après stabilisation. Aucun polling permanent ajouté.
- Les tests IME vérifient le contrôleur et son état de composition ; pas d’acceptance d’un serveur IME physique. Les remaps et layouts sont aussi couverts par les tests d’entrée existants. Aucun symptôme supplémentaire Enter/Shift+E n’a été communiqué ; le profil frais ne reproduit pas de défaut.
- Les profils natifs sont frais, avec uniquement F3 pour ouvrir Camera bookmarks et le bouton optionnel Align Text pour l’essai. Le keymap par défaut re-extrait du build courant est identique au profil utilisé.
- Un premier harness de drag relâchait avant la prise du grab du gestionnaire de fenêtres ; ce résultat invalide est conservé puis remplacé par `extra-valid`, avec attente de prise/relâchement. L’inventaire final avait initialement lié un ancien rlib ; il a été recompilé avec le rlib du build UI, et confirme 149 actions, 56 exclusions et des defaults identiques. Ces erreurs de harness ne sont pas attribuées au produit.

Preuves privées ignorées par Git : `benchmark-results/ui-cleanup/` : `workspace-tests.log`, `clippy.log`, `native/receipt.json`, `native/checks.json`, captures, `extra-valid/receipt.json`, `final-actions.json`, `inventory-compile.json`, `starting-state.json`.

## Binaire et intégrité

Preview release optimisée : `benchmark-results/ui-cleanup/tack`, SHA256 `e3d2cdb916c18c4c2a0bf0fcc9b84c292a80c76b2bbbc8e8d5cb209d79f75dad`. Build avec jobs=2, debuginfo=1 puis copie stripped. Le build-info indique le HEAD de départ avec worktree modifié ; il ne représente pas un commit propre.

`bin/tack` reste inchangé pour préserver les tests LAN en cours. Aucun PNG existant n’a été réécrit ; quatre nouvelles icônes `text_align_*` sont créées et copiées dans bin seulement si absentes. Seul `gfx/icons/rotate_view_tool.png`, exclusif à l’outil retiré, est supprimé. Le hash du README utilisateur est préservé. Réserve disque au terme du contrôle : environ 18,5 GiB ; aucune copie de fixture lourde ni purge générale des caches.

Fichiers concernés : actions/toolbar/icônes, menu contextuel, sélection (révision transitoire), édition annotation, chrome/desktop, local_ui signets et routage, tests ciblés, documentation des fonctionnalités/icônes. Aucun fichier du renderer GPU, supply, stockage, service/protocole/server LAN ni dépendance modifié par cette mission. Les DTO et documents n’acquièrent aucun champ. Aucun test sur le bureau physique ni acceptance Windows dans cette mission.

La mission UI est clôturée ; la suite autorisée commence par la saturation non fatale P0. L’examen LAN restera isolé et ne touchera pas à la session utilisateur active.
