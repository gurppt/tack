# 2A12C2 — Regroupement conservateur des draws, P1b

## Décision et identité

**Accepté.** À 10k images consécutives compatibles, l’encodage release NVIDIA baisse de ~2,53 à ~0,46 ms en médiane, et de ~2,56 à ~0,56 ms en p95 : environ **82 % / 78 %**, au-delà du seuil proposé de 50 %. Aucun tri d’objets, atlas, bindless, instancing framework, nouvelle feature GPU ou dépendance. P0 et P1a sont clôturés avant cette phase, autorisée par l’utilisateur.

Worktree dev basé sur `2f007d3fab954a00c54028be662eb98a7c285599`. Baseline GUI : copie du P1a final dans `benchmark-results/phase2a12c2/baseline`. Final helper avec clé/sampler constants hors boucle : `tack-key-cache`, SHA256 `9cfb293204907cf4c4f5ef228f1feeec4b13317966e210ae16660ac6a62d85ff`. Les binaries GUI précédents sont conservés avec leurs reçus ; build-info représente le HEAD de départ modifié. Une release propre, versionnée et compilée après commit suit dans une mission de packaging explicitement demandée.

## Implémentation

Une petite fonction privée `image_run_end` parcourt le flux de commandes existant. Un run nécessite même TextureKey, même sampler Nearest/Smooth, et indices de sommets contigus. Une annotation, un autre binding/sampler ou un index non contigu termine le run. Le pass bind et draw une fois sur la plage contiguë `first*6..end*6` ; les annotations gardent leur position et leur pipeline, le pipeline images est restauré à chaque run. Les overlays, sélection, UI, Frames et buffers sont inchangés.

Crops, rotation, flips, alpha et grayscale sont encodés dans les sommets individuels et n’exigent pas de splitter un run. Les tiles avec keys différentes ne fusionnent pas ; des tiles/instances compatibles peuvent fusionner sans déplacer leur ordre. La borne finale P0 de 10 000 quads et ses indices restent identiques.

Conservatisme : deux keys différentes manquantes qui résolvent toutes deux au placeholder **ne fusionnent pas**. Les keys None identiques peuvent fusionner. On garantit la compatibilité, sans chercher toutes les équivalences possibles du fallback ni ajouter un lookup de résidence par candidat. LRU get se fait une fois par run : l’ordre relatif de dernière utilisation des textures est conservé, aucun upload/éviction ne se produit pendant cette boucle. Valeurs de l’horloge interne différentes mais mêmes priorités relatives.

Aucune allocation de runs, cache permanent, thread, timer ou instrumentation permanente. La clé et le sampler du premier élément sont lus une fois par run. Le test privé couvre aussi A/B/A, samplers, annotation-barrier, index non contigu et None. Le test emploie le détecteur exact que le pass utilise.

## Draws/binds images

Une paire bind/draw par run, vérifiable directement dans la boucle de production ; annotations et overlay conservent leurs draws séparés. Pas de compteur permanent ajouté.

| Cas | Avant | Après |
|---|---:|---:|
| 10k même texture/sampler consécutifs | 10 000 | 1 |
| 10k textures distinctes | 10 000 | 10 000 |
| Alternance A/B/A | 10 000 | 10 000 |
| A/A/B/B/A/A… | 10 000 | 5 000 |
| Même texture, sampler alterné | 10 000 | 10 000 |
| Keys distinctes absentes | 10 000 | 10 000 |
| 64 images effets, 10 interleaves annotations | 64 | 23 |

## GPU A/B principal — NVIDIA, ms encode

GPU offscreen 800×600, RTX 2060/Vulkan, release optimisée, textures pré-uploadées, 5 warmups puis 30 samples, deux répétitions indépendantes. Deux séries finales sans compilation simultanée sont dans before/after-nvidia-quiet. Même shader, buffers, géométrie et budgets. Les quantiles ci-dessous sont la moyenne de chaque répétition ; p99/max, submit, completion incluant wait GPU et GPU timestamp pass sont enregistrés séparément.

| Images | Cas | Avant p50 | Avant p95 | Après p50 | Après p95 |
|---|---|---|---|---|---|
| 100 | repeated | 0.0561 | 0.0612 | 0.0388 | 0.0484 |
| 100 | unique | 0.0799 | 0.0857 | 0.0818 | 0.0930 |
| 100 | alternate | 0.0751 | 0.0826 | 0.0754 | 0.0808 |
| 100 | runs | 0.0675 | 0.0743 | 0.0634 | 0.0663 |
| 100 | sampler | 0.0791 | 0.0836 | 0.0756 | 0.0822 |
| 1000 | repeated | 0.2787 | 0.2904 | 0.0732 | 0.0902 |
| 1000 | unique | 0.5392 | 0.6235 | 0.5430 | 0.6373 |
| 1000 | alternate | 0.4645 | 0.4753 | 0.4669 | 0.4817 |
| 1000 | runs | 0.3951 | 0.4390 | 0.2720 | 0.2966 |
| 1000 | sampler | 0.4656 | 0.4782 | 0.4705 | 0.6092 |
| 5000 | repeated | 1.2699 | 1.4157 | 0.2357 | 0.3491 |
| 5000 | unique | 4.3183 | 4.5952 | 3.8450 | 4.4749 |
| 5000 | alternate | 2.1846 | 2.2673 | 2.2048 | 2.4202 |
| 5000 | runs | 1.8346 | 1.9534 | 1.2110 | 1.2712 |
| 5000 | sampler | 2.1988 | 2.3321 | 2.2235 | 2.3876 |
| 10000 | repeated | 2.5284 | 2.5606 | 0.4574 | 0.5625 |
| 10000 | unique | 9.2150 | 9.8298 | 8.2303 | 8.5026 |
| 10000 | alternate | 4.4313 | 4.5413 | 4.5487 | 5.6260 |
| 10000 | runs | 3.7291 | 3.8751 | 2.4433 | 2.5817 |
| 10000 | sampler | 4.4307 | 4.5130 | 4.5284 | 4.7141 |

10k unique : p50/p95 final améliorés, aucune réduction du nombre de draws revendiquée. L’écart ne prouve pas une optimisation générale des textures distinctes ; il comprend les variations de contextes/driver/hash maps. À 100 unique l’écart p95 est quelques microsecondes ; pas de régression matérielle démontrée. Un run sampler 1k a une queue p95 plus haute (~0,61 ms moyenne des runs vs ~0,48 avant) malgré une médiane proche de 0,47 ms ; cette dispersion est conservée, pas effacée par un choix de samples. Ce cas ne gagne aucun draw et reste à surveiller sur le bureau physique.

Une première variante vérifiait la clé avant le sampler ; une seconde inversait ces vérifications. Le final lit les invariants du run une fois et teste le sampler avant la clé. Les séries candidates sont conservées ; la décision utilise la série finale, pas le meilleur sample d’une variante.

Software Vulkan llvmpipe, LP_NUM_THREADS=2 : matrices répétées, uniques, A/B/A, runs, samplers, fallback et effets exécutées et captures comparées. Les timings logiciels sont complémentaires et dépendent du CPU ; ils ne constituent pas une acceptance de GPU faible physique. Les timestamps sont ceux fournis par le backend, pas des valeurs simulées. Les bytes GPU rapportés sont les texels, hors mémoire du driver. Le nouveau helper utilise seulement un petit état de pile ; aucun buffer ou budget mémoire agrandi. Peak RSS n’a pas été échantillonné systématiquement pour toutes les séries GPU ; ne pas déduire la VRAM système des texels.

## A/B natif séparé

P1a baseline versus P1b (variante avant la dernière lecture locale des invariants, sémantiquement identique), Xvfb privé/NVIDIA, deux board-tour 3 s, 800×600, immediate. Le build final est en plus testé nativement sur les boards à effets. Aucune entrée sur bureau physique.

| Images | Build | Encode p50 | Encode p95 | CPU p50 | CPU p95 | Callback p50 | Callback p95 | Present p50 | Present p95 |
|---|---|---|---|---|---|---|---|---|---|
| 100 | baseline | 0.1312 | 0.1592 | 0.2411 | 0.2789 | 19.8469 | 20.3564 | 19.5824 | 20.0733 |
| 100 | tack | 0.0887 | 0.1367 | 0.1868 | 0.2722 | 19.8916 | 20.5717 | 19.6761 | 20.2923 |
| 1000 | baseline | 0.3417 | 0.3650 | 0.6735 | 0.7307 | 20.5139 | 20.9925 | 19.7931 | 20.3014 |
| 1000 | tack | 0.1290 | 0.1472 | 0.4558 | 0.4949 | 19.9182 | 20.5896 | 19.4293 | 20.1198 |
| 5000 | baseline | 1.3663 | 1.4730 | 2.8820 | 3.1284 | 23.1391 | 23.8260 | 20.1548 | 20.8999 |
| 5000 | tack | 0.3086 | 0.3354 | 1.8074 | 1.9291 | 21.7504 | 22.2004 | 19.8943 | 20.3138 |
| 10000 | baseline | 2.6548 | 2.7049 | 5.4956 | 5.6889 | 26.0769 | 26.7202 | 20.5671 | 21.0154 |
| 10000 | tack | 0.5327 | 0.5708 | 3.4711 | 3.6298 | 23.8364 | 24.2778 | 20.3134 | 20.7140 |

Supply reste sur le chemin P1a. Le gain CPU d’encodage est réel ; la présentation Xvfb garde son coût connu. Ne pas convertir cette table en promesse de FPS sur le bureau utilisateur.

## Pixels, sécurité et idle

**14 comparaisons pixel-exactes** : répété, unique, A/B/A, runs, sampling mixte, keys absentes et 64 effets avec alpha/grayscale/crop/flips/rotation, annotation intercalée et overlay type Frame, sur NVIDIA et software Vulkan. Les données RGBA et hashes sont conservés. NVIDIA final compare les binaries du détecteur final ; software couvre le même algorithme avant la seule lecture locale d’invariants, dont les getters privés sont purs. Les vertex ranges, ordre et pixels sont identiques.

Les deux boards natifs à 64 images/Frame/annotations passent encore, avec égalité des captures et zéro tick/redraw dans les quatre intervalles stabilisés. Les 43 tests natifs P0 sont répétés : 9 999/10 000/10 001/20 000, duplication, Undo/Redo, sauvegarde/réouverture, Save As exact 20 001, recovery 10 002, menus, cap final, idle zéro. Test camera saturated→empty→saturated réussi ; JPEG 50 000² et PNG optionnel 50 000×4 096 convergent et restent bornés/zéro redraw au repos. Test tile GPU + deux convergence et un long churn réussis après batching. Aucun exit de saturation ni contenu de document supprimé.

Workspace all-features : 490 tests réussis, 0 échec, 21 ignorés. Clippy -D warnings et rustfmt/diff check passent. Le helper final repasse ses tests renderer et Clippy. Windows cross-build/CLI Wine final appartient au packaging qui suit ; **pas de test physique Windows** ni de LAN multi-machine dans cette phase.

## Portée et suite

Seul `tack-render/src/gpu.rs` change pour P1b. P0 admission/cap, P1a supply, schémas, originals, profils, server/service/protocole LAN et artwork restent inchangés. Le dossier bin utilisateur n’est pas remplacé pendant ses tests LAN. Réserve disque >17 GiB ; sorties RGBA limitées aux petites matrices, pas de copie de grands rasters.

Les textures distinctes restent coûteuses (lookup/bind/draw et travail driver par texture). Une investigation P1c éventuelle doit mesurer cela séparément ; aucun atlas/bindless/reorder/renderer rewrite n’est lancé ici. Pas de P2 implémenté. L’examen LAN complémentaire demandé est statique et documenté dans `MISSION_2A12_LAN_FINDINGS.md`, sans toucher sa session active.

Preuves privées : `benchmark-results/phase2a12c2/` matrices GPU et sources/commands de compilation, pixel-comparison-final.json, before/after pixels, perf/summary.json, effects-native et effects-key-cache, native-complete/receipt.json, camera, huge-jpeg, png, logs workspace/Clippy/LOD/tile. Les sources/notes de release et l’upload GitHub seront traités après cette clôture, selon la nouvelle instruction explicite de l’utilisateur.
