# 2A12B — Saturation du renderer, sécurité P0

## Identité et reproduction

Worktree basé sur `2f007d3fab954a00c54028be662eb98a7c285599`, dev, avec corrections UI et 2A12A préservées. La reproduction pré-patch utilise le binaire UI SHA256 `e3d2cdb916c18c4c2a0bf0fcc9b84c292a80c76b2bbbc8e8d5cb209d79f75dad`. Sur Xvfb privé/Vulkan logiciel : 10 000 images terminent normalement, 10 001 quittent avec `viewport exceeds current 10000-object renderer limit` (exit 1). Le reçu 2A12A `unsaved-overlap-receipt.json` confirme la perte de l’édition non sauvegardée à duplication.

Binaire validé : `benchmark-results/phase2a12b/tack`, SHA256 `75fbb781edb8c79dc11d70d582027d196ec9db8387255ff2b4e86cddf9582c90`. Rebuild final après réduction de la surface du test GPU : `tack-final`, SHA256 `e119d9183ee32c0dbbb97aebd7845765a013e91ac27525588789c42ee16716fa`. Aucun binaire du répertoire utilisateur bin remplacé.

## Route auditée et choix

Visibilité en ordre document → previews de transform/filter → admission d’affichage → besoins LOD et supply → tiles → scene annotations/images → buffer de sommets → submission. La limite GPU reste 10 000 **quads finaux**, désormais publiée comme `MAX_IMAGE_QUADS` et utilisée par l’application et le planner de tiles. Le modèle accepte toujours 100 000 objets ; aucune autorité du document n’est réduite.

Solution retenue : budget déterministe de présentation. Jusqu’à la limite, même Vec réutilisé ; à saturation uniquement, heap borné à 10 000. Priorité aux images sélectionnées visibles, puis aux images au premier plan. Si plus de 10 000 images visibles sont sélectionnées, les dernières dans l’ordre document gagnent et le nombre de sélectionnées omises est explicite. Le sous-ensemble retenu est trié en ordre initial : pas de tri par texture ni de modification de z-order. Les objets hors viewport ne prennent pas de place ; les previews interactives sont incluses avant admission.

Le statut non modal indique le nombre affiché/visible/omis et les sélectionnées omises, avec « Board intact ». Il reste visible même lorsque la status bar était masquée ; aucune échéance/timer ajouté. À retour sur une vue moins dense, les omissions et le message disparaissent. Les overlays de sélection restent sous leurs bornes existantes ; les annotations gardent leur position entre les images survivantes, les Frames restent dans le chemin overlay existant. Les limites propres des annotations/glyphes sont inchangées.

Alternatives écartées : augmenter arbitrairement le buffer ou retirer sa limite ne fournit pas de borne sûre ; chunking nécessite plus de gestion de sommet/submission/interleaves et ne borne pas le travail par frame. Le sous-ensemble explicite est le plus petit correctif local. Ce choix limite volontairement la présentation saturée ; il ne prétend pas afficher tout le board.

Le planner de tiles contenait déjà une réserve `10000 - logical_draws`, puis 64 tiles maximum : la deuxième route de dépassement n’a donc pas été reproduite avec le planner normal. Audit et défense ajoutée : le display compte la capacité finale, réserve un fallback par image restante et substitue chaque groupe atomiquement seulement s’il tient. Sinon, overview complet et notification de détail limité ; pas de polling pour un groupe refusé par capacité. Test forcé : 9 999 images + groupe de 3 ferait 10 001, donc overview ; groupe de 2 tient exactement à 10 000, rendu GPU valide. JPEG et PNG partagent cette défense.

## Intégrité et tests natifs

43 contrôles passent dans `native-complete/` à 800×600, Xvfb/Xfwm4 privés, software Vulkan, profils propres : N=9 999/10 000/10 001/20 000, duplication locale, sauvegarde avec exact N+1, Undo N, Redo N+1 et géométrie identique, menu F10, cap de quads, compte visible complet, omissions, fermeture normale/réouverture. Save As de 20 001 objets conserve le document exact. Aucun hard exit de saturation. Les quatre intervalles stabilisés de 2 secondes contiennent 0 redraw.

Le test de récupération modifie le board saturé sans sauvegarder, attend le snapshot existant de recovery, puis tue **uniquement son processus privé**. La restauration explicite sauvegarde les 10 002 objets attendus. Ce n’est pas une promesse de recovery instantané avant le debounce existant de 5 secondes ; la différence P0 est que l’application ne quitte plus à la saturation.

Deux contrôles caméra supplémentaires passent : pan vers viewport vide efface les omissions, rappel d’un raccourci de vue revient à 20 000 visibles/10 000 omis sans restart. Captures sparse/saturated/restored conservées. Les tests d’admission couvrent aussi 100 000 candidats, tous sélectionnés, ordre, opacity/crop/rotation/flips/grayscale et annotation intercalée, sans mutation du document.

Des premiers essais de harness ont cliqué avant chargement ou au mauvais point après changement d’extent ; ils n’avaient pas sélectionné l’image et sont conservés comme invalides. `native-complete` calcule le point à partir de la caméra dense et vérifie « 1 selected ». Le premier test GPU forçait un overdraw plein écran inutile : passé mais 52 s ; sa cible réduite 8×8 vérifie la même borne en 0,23 s.

## Export et mémoire

Sauvegardes, Save As, history, source references et exports de sources lisent le document complet, indépendamment de l’admission d’affichage. Le snapshot a déjà un refus explicite au-delà de 4 096 objets/assets/sources, et refuse les omissions d’annotations ou dépassements de textures/dimensions ; il n’utilise pas le sous-ensemble P0. Aucun export complet limité n’est annoncé réussi.

Heap et Vec temporaire sont chacun bornés à 10 000 ; pas de buffer GPU agrandi, worker ajouté, source originale clonée dans redraw ni allocation proportionnelle à 100 000 au sein du nouveau budget. Le memo de visibilité existant garde sa borne document. Les quads, textures et travailleurs restent sous leurs limites précédentes.

Grand JPEG 50 000² : 4 tiles ready, CPU/GPU payload maximum 2 179 136 octets, RSS environ 121 MiB, zéro tick/redraw stabilisé ; pas d’allocation RGBA de 10 Go. PNG 50 000×4 096 opt-in : tiles convergées, quads sous cap et zéro redraw au repos. Voir reçus pour RSS/ticks exacts. La première tentative JPEG omettait le helper à côté du preview ; symlink vers le helper existant rétabli, aucun changement codec. Les bytes GPU rapportés sont des texels, pas une mesure VRAM globale.

## A/B release

NVIDIA RTX 2060/Vulkan sur Xvfb privé, 800×600, immediate present, deux répétitions indépendantes de board-tour de 3 secondes, même source minuscule répétée. Statistiques sur frames après 1 s et contenu reconnaissable complet. Moyenne des quantiles de chaque répétition ci-dessous ; données p99/max individuelles conservées, pas une acceptance du bureau physique.

| Images | Build | Query p50 | Query p95 | Supply p50 | Supply p95 | Encode p50 | Encode p95 | CPU p50 | CPU p95 |
|---|---|---|---|---|---|---|---|---|---|
| 100 | baseline | 0.0208 | 0.0353 | 0.1215 | 0.1997 | 0.1743 | 0.2974 | 0.3916 | 0.6015 |
| 100 | tack | 0.0189 | 0.0245 | 0.1171 | 0.1627 | 0.1482 | 0.1846 | 0.3514 | 0.4347 |
| 1000 | baseline | 0.1488 | 0.1860 | 0.8470 | 0.9000 | 0.3569 | 0.4154 | 1.4431 | 1.5900 |
| 1000 | tack | 0.1486 | 0.1928 | 0.8346 | 0.9928 | 0.3497 | 0.4020 | 1.4162 | 1.6442 |
| 10000 | baseline | 1.4433 | 1.5537 | 7.9106 | 8.0403 | 2.6585 | 2.8950 | 12.7669 | 13.2978 |
| 10000 | tack | 1.4657 | 1.5685 | 8.0405 | 8.3352 | 2.7465 | 2.9556 | 13.0575 | 13.3842 |

Toutes les valeurs sont en millisecondes. À 100/1k : pas de régression matérielle ; à 10k les faibles variations ne démontrent ni gain ni régression de cet ajout de sécurité. Acquisition/present/callback/submission sont aussi enregistrés séparément ; Xvfb n’est pas un substitut au bureau physique. Les sources distinctes seront baselinées avant supply, pas revendiquées dans cet A/B P0. La saturation n’a été acceptée nativement qu’en source répétée ; la politique est testée indépendamment des textures, et le test GPU vérifie le cap final.

## Vérification et portée

Workspace all-features : **489 réussis, 0 échec, 21 ignorés**. Clippy -D warnings et fmt/diff check passent. Test tile GPU ignoré exécuté explicitement et réussi sur software Vulkan ; LOD convergence (2) et long churn (1) passent. JPEG et PNG natifs passent. Windows cross-check demandé, acceptance physique Windows/Linux non effectuée. Aucun pilotage du bureau utilisateur ni contact de la session LAN active.

Modules P0 : nouveau `display_budget`, export de limite GPU, admission/redraw et notification chrome, référence de capacité dans launch, défense tiles, tests. Aucun changement de service/protocole/server LAN, stockage, schéma, dépendance, source persistence ni UI unrelated. L’UI modifiée à cette phase se limite à la notification indispensable. `bin/tack`, README utilisateur et artwork existant sont conservés. Réserve disque environ 18 GiB.

P0 clôturé. L’utilisateur a explicitement autorisé la suite P1a puis P1b avant ces travaux ; elles seront mesurées séparément, sans modification LAN.
