# 2A12C1 — Coût du supply/LOD, décision P1a

## Décision et baseline

**Accepté : spécialisation des vues homogènes en asset.** Aucun cache persistant ni changement du renderer. P0 est terminé, reproduit et vérifié dans `MISSION_2A12B_RENDERER_SAFETY_REPORT.md` ; l’utilisateur a autorisé P1a et P1b à l’avance. Worktree dev basé sur `2f007d3fab954a00c54028be662eb98a7c285599`, fichiers UI/2A12A conservés.

Baseline P0 release `baseline` : SHA256 `e119d9183ee32c0dbbb97aebd7845765a013e91ac27525588789c42ee16716fa`. Final P1a `benchmark-results/phase2a12c1/tack` : SHA256 `f60e00a7f8744b0668fbd83f5df053b3008deaed3fcc654f3182cdc148ca2fcb`. Binaries stripped pour limiter le disque ; aucun remplacement de bin/tack. Les métriques natives utilisent deux runs distincts, NVIDIA RTX 2060/Vulkan, Xvfb privé, 800×600, immediate present et board-tour 3 s, même méthodologie que 2A12A ; frames après 1 s, visibles/reconnaissables/qualité résolue complets, workers pending/queued=0.

## Profil et candidats

Le baseline instrumenté 10k répété mesure supply p50 **7,875 ms**, avec médianes par étape : Need **0,411**, plan **0,296**, demandes + tiles + tri + pin **2,277**, replace_view/poll/schedule **1,678**, représentation/upload **1,497**, sélection post-upload + display tiles **1,719 ms**. Les médianes par étape ne sont pas additives strictement. L’enregistrement détaillé est temporaire, absent du code final.

Premier candidat : grouper toutes les instances consécutives d’un même asset dans demandes/upload/qualité. Gain élevé en répété mais coût distinct p50/p95 défavorable dans les premières mesures (9–11 % à 10k) ; **écarté** pour éviter une optimisation générale mal étayée.

Candidat retenu : déterminer à chaque frame si tous les dessins admis partagent le même AssetId. Si oui, un groupe pour demandes, pins, tentative d’upload et résultat de résidence post-upload ; sinon groupes singleton. Pour un board distinct, le contrôle d’homogénéité s’arrête dès le deuxième asset différent. Pas de hash map supplémentaire ni de nouvelle allocation. Les besoins `Need` restent individuels : tailles, crops, zoom et sampling hétérogènes contribuent encore intégralement au plan. La priorité preview du groupe est le minimum des priorités individuelles. La Vec des demandes garde sa borne précédente, même si la vue homogène n’en remplit plus que 1 ou 2.

Scratch/caching de plan non implémentés : le profil ne les justifie pas après cette réduction, et un cache durable imposerait de nombreux invalidateurs. L’agrégation générale par source/asset et un index spatial sont différés ; pas de complexité introduite pour les seuls chiffres répétés.

## A/B native (ms)

Les valeurs ci-dessous sont la moyenne des quantiles de deux répétitions, avec p99/max individuels, acquire/present/submit et RSS dans les reçus.

| Images | Build | Supply p50 | Supply p95 | CPU p50 | CPU p95 | Encode p50 | Encode p95 | Callback p50 | Callback p95 |
|---|---|---|---|---|---|---|---|---|---|
| 100 | avant | 0.1074 | 0.1372 | 0.3076 | 0.4001 | 0.1286 | 0.1881 | 20.3051 | 21.0697 |
| 100 | après | 0.0365 | 0.0491 | 0.2348 | 0.2645 | 0.1292 | 0.1515 | 19.9313 | 20.4031 |
| 1000 | avant | 0.8322 | 0.9205 | 1.4215 | 1.6064 | 0.3533 | 0.4033 | 21.7535 | 22.9261 |
| 1000 | après | 0.1261 | 0.1483 | 0.6717 | 0.7371 | 0.3397 | 0.3782 | 20.3264 | 20.8201 |
| 5000 | avant | 4.1158 | 4.3130 | 6.6976 | 7.4981 | 1.4245 | 1.5521 | 27.0979 | 29.4378 |
| 5000 | après | 0.5509 | 0.5829 | 2.8903 | 2.9764 | 1.3973 | 1.4317 | 23.0199 | 23.4408 |
| 10000 | avant | 7.8782 | 8.0434 | 12.6779 | 12.9932 | 2.6682 | 2.7157 | 33.3273 | 33.9935 |
| 10000 | après | 0.8817 | 0.9013 | 5.5191 | 5.8372 | 2.6861 | 2.9455 | 26.0877 | 26.7697 |

Le seuil ≥30 % en p50 et p95 supply à 10k répété est dépassé largement ; le CPU total baisse aussi. Encode reste dans son chemin initial, sans regroupement GPU à cette phase. Xvfb acquire/present conserve son coût connu et ne représente pas le bureau physique.

## Sources distinctes : protocole contrôlé et limite native

Un run natif cold de 70 s à 10k sources distinctes n’atteint que 2 712 textures reconnaissables, avec 14 pending/12 queued à la fin. Les deux workers et le cycle GUI/présentation Xvfb limitent la convergence ; **ce run ne constitue pas un benchmark warm valide et n’est pas utilisé pour revendiquer un gain**. Le premier filtre du harness comparait aussi quality_resolved à desired_detail, alors que la métrique compte toutes les instances adéquates, y compris Thumbnail : corrigé pour vérifier quality_resolved=N. Les tentatives invalides sont conservées.

Pour comparer des sources distinctes entièrement résidentes sans confondre décodage et rendu, un probe release exécute les blocs supply extraits mécaniquement des sources avant/après, avec le vrai `ProductAssets`, le vrai planner/tiles et le vrai `Gpu`. Tous les produits courants sont uploadés avant la mesure ; ce workload dispose de textures GPU mais pas de cache CPU de représentations. Il mesure le **chemin de supply warm GPU**, pas l’import cold, le scheduler sous charge, l’encode ni une frame native complète. Aucun mock de planner ou de résultat de résidence.

Deux passages indépendants et 60 samples par cas ; comparaison avant/après alternée dans **le même** document/assets/GPU pour maîtriser dispersion des hash maps et fréquences CPU. Chaque frame est entièrement reconnaissable et sans supply_pending. Les matrices séparées avaient une dispersion plus grande ; elles sont conservées et ne remplacent pas la comparaison appariée finale.

| Images | Sources | Avant p50 | Avant p95 | Après p50 | Après p95 |
|---|---|---|---|---|---|
| 100 | répété | 0.0720 | 0.0769 | 0.0123 | 0.0124 |
| 100 | distinct | 0.1224 | 0.1275 | 0.1245 | 0.1316 |
| 1000 | répété | 0.7129 | 0.7268 | 0.1031 | 0.1128 |
| 1000 | distinct | 1.5073 | 1.5371 | 1.5319 | 1.5575 |
| 5000 | répété | 3.5567 | 3.5762 | 0.4937 | 0.5187 |
| 5000 | distinct | 8.4439 | 9.1373 | 8.5993 | 8.8296 |
| 10000 | répété | 6.9200 | 6.9631 | 0.8045 | 0.8153 |
| 10000 | distinct | 18.8446 | 19.4230 | 19.1462 | 20.0713 |

À 10k distinct, écart apparié p50 environ +1,6 %, p95 +3,3 % ; pas de régression matérielle démontrée. Les contrôles 100/1k distinct restent dans une faible variation absolue. Aucune accélération générale des sources distinctes revendiquée. L’acceptance native warm de 10k sources distinctes reste un complément à effectuer ; les petites sources et textures entièrement préchargées ne démontrent pas le comportement d’un import de 10k photos HD.

## Invalidation et correction

Pas de donnée réutilisée entre frames. Homogénéité, plan, priorités, révisions et qualité sont reconstruits à chaque redraw. Tous les uploads d’overview/detail **et de tiles** précèdent encore la sélection de la texture affichée ; aucune résidence pré-upload n’est considérée valide après éviction. Le groupe n’existe que pour un AssetId identique dans une frame, donc même source/révision/desired, mais les géométries restent individuelles. Mixed assets/aliases utilisent le chemin singleton. Les états failed_rep sont consultés après observe_current/poll et gardent leurs gardes de révision. Workers, annulation, budget, originals et protocoles sont inchangés.

Camera/zoom, crop/transform, sampling, grayscale, source revision, dimensions et budgets participent encore au code existant ; worker completion et GPU eviction sont relus à chaque frame. Pas d’invalidation par génération seule, pas de résultat mémorisé au repos, pas de timer supplémentaire, pas d’IO ajouté au redraw.

## Validation

Workspace all-features : 489 réussis, 0 échec, 21 ignorés ; Clippy -D warnings et fmt/diff check passent. GPU convergence (2) et long churn (1) exécutés explicitement : succès, évictions/réentrée/révisions/crossings/pixels couverts par les tests existants.

Deux boards natifs de 64 images : même asset avec crops/rotation/flips/opacity/grayscale/samplers différents, puis assets aliases/mixed. Rectangle annotation intercalé et Frame. Images pixel-identiques avant/après, et les quatre fenêtres ont zéro tick/redraw dans leurs intervalles stabilisés de 2 s. Le premier harness ne montrait qu’une image (auto-fit du premier objet) et avait sauvegardé le même fichier entre builds, ce qui changeait Undo : invalide. Le final place le Frame en premier pour un cadrage de toutes les 64 images et utilise deux copies fraîches ; `effects-native-valid/receipt.json` vérifie population et pixels.

JPEG 50 000² + PNG optionnel 50 000×4 096 : tiles convergent, quads bornés, zéro redraw après stabilisation, payload budgets inchangés. Les changements UI de taille réelle/crop/gray et source transitions sont aussi couverts par la suite workspace. Les pan/zoom du board-tour restent natifs ; aucune session physique ou LAN active pilotée.

Le patch P1a ne touche que les trois boucles supply de `product_window.rs` (et le test d’homogénéité), sans modification de supply_plan, ProductAssets, renderer, stockage, server/service/protocole LAN, dépendance ou profil. Instrumentation retirée. RSS et bytes GPU des runs sont conservés ; ceux-ci sont des texels et non une mesure VRAM totale. Réserve disque >17 GiB. Windows physique reste non effectué ; un build final commun sera vérifié après P1b.

Preuves : `benchmark-results/phase2a12c1/` : profile-valid, before-repeated, after-homogeneous, paired.json et sources/compilations du probe, effects-native-valid, logs workspace/Clippy/LOD, JPEG/PNG. P1a clôturé, décision d’acceptation fondée sur la réduction native répétée et l’absence de régression matérielle dans l’A/B distinct apparié. La suite P1b est autorisée séparément par l’instruction globale de l’utilisateur.
