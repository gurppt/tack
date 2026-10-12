# Phase 2A12A — Diagnostic Turbo, robustesse et cohérence UI

Date : 12 octobre 2026, Europe/Paris. Rapport non committé. Les données sont dans `benchmark-results/phase2a12/`, ignoré par Git.

## Décision proposée

**P0 : supprimer la sortie fatale au dépassement du budget de rendu.** Un board accepté par le modèle peut faire quitter Tack au-delà de 10 000 images visibles. Sur un board jetable, une duplication a provoqué cette sortie, sans sauvegarde ni snapshot de récupération : l'édition est perdue. Ce correctif de fiabilité doit précéder les optimisations.

**P1 : réduire le travail de préparation des demandes d'images, puis les commandes de rendu répétées.** À 10 000 images visibles utilisant une même petite source, une frame native consomme médianement 12,658 ms de préparation/encodage, dont 7,735 ms de supply et 2,707 ms d'encodage GPU. Un index spatial ne cible pas ce coût dominant. Avec 10 000 textures distinctes, l'encodage seul monte à 8,724 ms : ce deuxième scénario réclame un traitement et des objectifs séparés.

**Mulot : conserver la conception actuelle.** Sur le bureau réel, la poursuite consomme environ 3 % d'un cœur, contre 80–88 % sous Xvfb/NVIDIA. Sous Xvfb, le temps se concentre dans `present`, même avec le compositeur activé, sans instrumentation et en mode immédiat. L'hypothèse d'une simulation intrinsèquement disproportionnée est réfutée sur ce poste. Le mécanisme précis du pilote reste à profiler avant d'affirmer un busy-wait identifié.

**P2 : allocations de scratch, invalidations plus fines et petit coût fixe des buffers.** Mesurer les économies et les conditions d'invalidation avant modification. Conserver le cache de visibilité et le pipeline de streaming existants.

**Serveur : DEFER UNTIL LAN ACCEPTANCE COMPLETE.** Amplification et verrou global confirmés par lecture ; aucune mesure de charge ni modification du serveur actif.

Les petits correctifs UI ont été explicitement autorisés par le message utilisateur postérieur au brief (« petits correctifs d'interface, mais surtout, gros diagnostic »). Ils sont réalisés et testés, sans installation dans `bin/`. Les optimisations, la limite de rendu, les fonctionnalités desktop et le serveur restent inchangés. Aucun commit/push.

## 1. Identité et limites de l'environnement

- HEAD local : `2f007d3fab954a00c54028be662eb98a7c285599`, branche `dev`.
- Baseline exécutée : `bin/tack`, SHA-256 `5eaa5713475511b2674d5544c8f2db93b216e6c5cc23453ac5d38bbe66d19fb3`, build source `768b921a427a1f2082c67750a6bc5a92dbceccf2`, version `0.1.0-dev.1`, LAN 4, network/updater activés. Le HEAD plus récent ajoute la documentation. Ce binaire n'a pas été remplacé.
- Les changements préexistants de `README.md`, les documents non suivis, `director_handoff/`, `gfx/` et `test_file/` sont préservés. Le snapshot `environment.json` a été pris alors que les trois corrections UI étaient déjà en cours : ce n'est pas une preuve de worktree initialement propre.
- Changements de cette mission : `context_menu.rs`, `image_input.rs`, `image_metadata_actions.rs`, nouveau `tests/multi_image_2a12.rs`, ce rapport. Aucun changement render/assets/storage/server/protocole/schéma.
- Ryzen 7 2700X, 8 cœurs / 16 threads ; RAM 31,25 GiB, environ 23,4 GiB disponible au début. NVIDIA RTX 2060 6 GiB, pilote `580.173.02`, Vulkan. Linux `7.1.5-76070105-generic`, Rust/Cargo `1.95.0`.
- X11/Xorg, Xfwm4 `4.18.0`, compositeur réel **activé**. HDMI 1920×1080 à 60 Hz utilisé pour le test ; autre 1920×1080 à 60 Hz et écran tourné 1200×1920 à 59,95 Hz. DPI physique non mesuré ; viewport réel 800×600, échelle de test 1.
- Au départ, GPU global déjà chargé à environ 30 %, 797 MiB de VRAM utilisés ; load average 1,08/0,73/0,76. Pas de poste exclusivement réservé ni attribution fiable de VRAM/utilisation par processus. Le serveur et client utilisateur en cours sont restés actifs et n'ont reçu aucun input/connexion de test.
- Environ 19 GiB libres au début et à la fin : réserve de 10 GiB respectée. Réutilisation des gros fichiers, pas de `cargo clean`, de purge du cache OS ou de suppression des données utilisateur. Seuls les exécutables de sonde créés ici ont été stripped.
- Baseline release optimisée. Sonde Rust : `rustc --edition=2024 -O -C debuginfo=0`, liée aux rlibs release existantes ; commandes et hashes dans `probe-compile.json` / `probe-links.json`. Le contrôle GPU distinct ne fait appel qu'au renderer inchangé ; sonde/commande séparées. Preview UI : `CARGO_BUILD_JOBS=2 cargo build --release --locked -p tack-app`, profil release `debug=1`, source locale avec diff UI ; son SHA figure dans `native-ui-pass/receipt.json`. **Son `--build-info` indique HEAD, pas l'ensemble du diff : c'est un build de worktree modifié.**

## 2. Mulot : bureau réel versus affichage synthétique

L'accord utilisateur pour 60 secondes a été reçu et annoncé avant déplacement du pointeur. Fenêtre Tack dédiée avec profil isolé, propriétaire PID/focus vérifiés avant input, watchdog SIGALRM/SIGINT/SIGTERM, restauration finale du focus/pointeur. Durée mesurée 49,47 s (49,51 s jusqu'à l'annonce de restauration), `restored=true`. Aucune autre prise de contrôle du bureau. L'acceptation perceptive vient du retour utilisateur précédent ; l'automatisation ne prétend pas valider humainement la fluidité ou l'absence de tearing.

Même chemin de curseur : mouvements droits et diagonaux, changements brusques de direction, pas de 150 ms pendant une poursuite de 3 s. Spawn 3,3 s ; FIFO instrumenté couvre Run → Idle qualifié → Attack → Munch → Explosion → attente finale. Les autres variantes sont arrêtées par Escape. Mesure quiet avant/après : 2 s ; contrôle non-jeu : hover alternant chrome/canvas à environ 12 Hz pendant 2 s. Ce contrôle fait des redraws UI ; ce n'est pas une animation strictement identique pixel par pixel à Mulot.

Trois séries uniques : bureau réel ; Xvfb + Xfwm4 sans compositeur ; Xvfb + Xfwm4 avec compositeur vérifié par possession de `_NET_WM_CM_S0`. Chaque série compare FIFO instrumenté, immédiat instrumenté et FIFO sans `--output`. Toutes à 800×600, même GPU. Les répétitions de frames ne sont pas des répétitions indépendantes de session. Les variantes n'ont pas été randomisées.

### CPU du processus — pourcentage d'un cœur

| Affichage | Variante | Baseline % | Poursuite % | Après % | Ticks thread principal / ~3 s | RSS poursuite MiB |
| --- | --- | --- | --- | --- | --- | --- |
| Réel, composité | fifo-instrumented | 0.000 | 3.304 | 0.497 | 8 | 323.535 |
| Réel, composité | immediate-instrumented | 0.000 | 3.489 | 0.994 | 10 | 297.914 |
| Réel, composité | fifo-uninstrumented | 0.000 | 2.855 | 0.994 | 7 | 288.613 |
| Xvfb, sans compositeur | fifo-instrumented | 0.994 | 87.644 | 0.497 | 270 | 303.637 |
| Xvfb, sans compositeur | immediate-instrumented | 0.993 | 81.084 | 1.491 | 250 | 303.445 |
| Xvfb, sans compositeur | fifo-uninstrumented | 0.994 | 81.940 | 0.000 | 252 | 294.641 |
| Xvfb, composité | fifo-instrumented | 1.988 | 87.745 | 0.497 | 272 | 303.691 |
| Xvfb, composité | immediate-instrumented | 0.994 | 82.486 | 0.497 | 255 | 303.684 |
| Xvfb, composité | fifo-uninstrumented | 0.994 | 79.936 | 0.498 | 246 | 294.352 |

Sur le réel FIFO, 8 des 10 ticks actifs sont dans le thread principal ; sous Xvfb sans compositeur, 270 ticks principal pendant la poursuite, environ 88 % d'un cœur. Après le jeu, le principal revient à 0 tick dans les trois variantes réelles. Le total NVIDIA peut garder 0,5–1 % et des context switches de threads pilote : **ne pas annoncer zéro activité de tous les threads**. Aucune lecture/écriture disque physique dans ces intervalles ; sockets uniquement Unix. Le RSS réel passe d'environ 320,5 à 325 MiB dans la série FIFO instrumentée ; sans instrumentation, 286,7 → 288,6 MiB. Ces écarts incluent pilote, buffers et journal de traces, pas seulement les 26 896 octets d'artwork du jeu.

### Frames Run — p50 / p95 / p99 / max, millisecondes

Frames classées par état de la trace (tout Run, comprenant approche/docking, pas uniquement les 3 s de trajectoire). `cpu_ms` et `callback_ms` sont des **durées murales instrumentées**, pas des samples de stack CPU. Les distributions détaillées de tous les états, acquire et submit sont dans `mulot-frame-summary-v2.json`.

| Affichage | Mode | Frames | Préparation CPU ms | Present ms | Callback ms |
| --- | --- | --- | --- | --- | --- |
| physical | fifo-instrumented | 180 | 0.223 / 0.294 / 0.321 / 0.417 | 0.075 / 1.416 / 2.944 / 4.114 | 0.368 / 1.757 / 3.210 / 4.403 |
| physical | immediate-instrumented | 157 | 0.232 / 0.278 / 0.314 / 0.414 | 0.073 / 1.726 / 5.558 / 6.480 | 0.369 / 2.038 / 5.821 / 6.803 |
| private-nvidia-validated | fifo-instrumented | 180 | 0.150 / 0.249 / 0.281 / 0.306 | 20.940 / 23.638 / 28.147 / 31.331 | 21.184 / 23.871 / 28.345 / 31.487 |
| private-nvidia-validated | immediate-instrumented | 152 | 0.173 / 0.252 / 0.268 / 0.275 | 20.987 / 26.079 / 30.722 / 32.573 | 21.228 / 26.292 / 30.933 / 33.015 |
| private-composited | fifo-instrumented | 181 | 0.196 / 0.280 / 0.325 / 0.350 | 21.451 / 23.723 / 27.247 / 28.233 | 21.705 / 23.992 / 27.497 / 28.437 |
| private-composited | immediate-instrumented | 156 | 0.203 / 0.294 / 0.328 / 0.346 | 21.012 / 23.471 / 24.741 / 28.888 | 21.255 / 23.764 / 25.023 / 29.271 |

Le `present` médian est ~0,075 ms sur le réel, ~21 ms sous Xvfb. Le contrôle hover immédiat coûte environ 0,99 % d'un cœur sur le réel, 25,60 % sous Xvfb sans compositeur, 26,10 % avec compositeur. L'immédiat conserve le coût synthétique et n'apporte pas de gain réel convaincant ; aucun changement de défaut recommandé. Le coût sans instrumentation reste 2,86 % sur le réel et 79,94–81,94 % sous Xvfb : la télémétrie n'explique pas le problème.

Au terme du lifecycle, les receipts confirment `tool=Pointer`, `game_active=false`, `deadline_active=false`, `cursor_hidden=false`, `artwork_bytes=0`. Les transitions et la simulation à cadence dédiée fonctionnent sans timer permanent après le jeu.

**Conclusion : CONFIRMED dépendance forte à Xvfb/presentation ; REFUTED coût disproportionné de Mulot sur ce bureau ; UNMEASURED stack exacte du pilote.** CPU système/utilisateur séparés, run queue par thread, wakeups noyau, PSS de la session physique, pertes d'acquire et tearing ne sont pas mesurés. Context switches volontaires disponibles, distincts des wakeups applicatifs. Pas de `perf` installé ; pas de profiling intrusif ou réglage système. Xvfb utilise un écran virtuel 1400×1000 ; sa fréquence effective n’est pas mesurée et ne doit pas être assimilée au 60 Hz physique. 1024×768 physique et autre GPU/Windows restent non testés.

## 3. R1 — coût du renderer

`crates/tack-render/src/gpu.rs:769` produit un draw et un bind par image. Sonde offscreen NVIDIA, cible 800×600, 16×16 RGBA par texture, grille de quads, 5 warmups puis 30 samples/cas, attente de complétion GPU. Aucun surface/present, chrome ou annotation dans cette sonde. Temps total inclut la complétion GPU, donc ne se confond pas avec encode/submit ou pass GPU. Le GPU pass utilise les timestamps existants ; diagnostics explicitement activés. La première sonde non instrumentée donne seulement un temps total valide : ses champs encode/submit à zéro ne sont **pas** des mesures de coût nul.

| Famille | N visible | Textures | Encode ms p50/p95/p99/max | Total + complétion ms p50/p95/p99/max | Pass GPU p50 ms |
| --- | --- | --- | --- | --- | --- |
| Référence répétée | 100 | 1 | 0.067 / 0.085 / 0.085 / 0.322 | 0.290 / 0.374 / 0.457 / 0.467 | 0.013 |
| Référence répétée | 1000 | 1 | 0.288 / 0.304 / 0.305 / 0.307 | 0.505 / 0.521 / 0.566 / 0.907 | 0.025 |
| Référence répétée | 5000 | 1 | 1.326 / 1.558 / 1.605 / 1.664 | 1.839 / 2.274 / 2.302 / 2.956 | 0.098 |
| Référence répétée | 10000 | 1 | 2.706 / 3.140 / 3.188 / 3.420 | 3.700 / 4.320 / 4.458 / 5.108 | 0.199 |
| Textures distinctes | 100 | 100 | 0.094 / 0.104 / 0.113 / 0.362 | 0.300 / 0.388 / 0.515 / 0.701 | 0.011 |
| Textures distinctes | 1000 | 1000 | 0.581 / 0.755 / 1.211 / 1.365 | 0.833 / 1.781 / 1.883 / 2.228 | 0.020 |
| Textures distinctes | 5000 | 5000 | 4.001 / 5.087 / 5.143 / 5.222 | 4.829 / 6.212 / 6.259 / 6.323 | 0.125 |
| Textures distinctes | 10000 | 10000 | 8.724 / 9.469 / 9.764 / 10.053 | 10.302 / 11.065 / 11.305 / 11.757 | 0.256 |

Les 10 000 textures distinctes restent dans 10 240 000 octets de texels logiques ; RSS de la sonde distincte au maximum échantillonné 402 MiB (garde 768 MiB), sortie 0. Le coût d'objets et allocations pilote est distinct du poids des texels. Ces sources minuscules ne représentent pas 10 000 originaux HD simultanément décodés ou affichés en pleine résolution.

**CONFIRMED** : encodage linéaire, et textures distinctes nettement plus coûteuses. **CONDITIONAL** : le bénéfice du batching dépend des répétitions consécutives de texture/sampler. Solution minimale à essayer : regrouper seulement les ranges consécutifs compatibles, conserver ordre/transparence/crop/grayscale/opacity et retomber sur le chemin actuel pour les textures distinctes. Cette solution ne résout pas le pire cas 10 000 textures uniques. Pas d'atlas ou bindless imposé : hardware compatibility, eviction, source variety et coût mémoire doivent être comparés dans une phase dédiée.

## 4. R2 — limite fatale, reproduction et édition perdue

Limite du modèle `DocumentLimits` : 100 000 objets. Budget renderer : 10 000 quads d'image. Contrôle `product_window.rs:294`, puis second contrôle `gpu.rs:677`. Erreur de redraw propagée à `handle_window_event` → `e.exit()`, puis `save.finish` attend seulement un travail déjà lancé. Aucun snapshot de dernière minute garanti.

1. Fixtures privées générées par la sonde, asset/source uniques 16×16, N=100/1 000/5 000/10 000/10 001. Ouverture avec `--dense-view`, compte **visible vérifié**, 800×600 : jusqu'à 10 000 sortie 0 ; 10 001 sortie 1 et `viewport exceeds current 10000-object renderer limit`. Aucun frame validé à 10 001 : le champ visible=0 du receipt indique absence de frame, pas culling réussi.
2. Fixture privée de 10 000 images superposées, première image assez grande pour que la vue initiale contienne toutes les images et la duplication décalée. Sélection d'une image, invocation locale `DuplicateSelection` : sortie 1, état final dirty, génération 2, une image sélectionnée, 2 entrées Undo (dont une préparation de source), aucune save/recovery complétée. Board sur disque inchangé, 10 000 objets ; profil ne contient aucun snapshot. La duplication ne peut être restaurée. Voir `unsaved-overlap-receipt.json`.

**CONFIRMED P0**, pas seulement risque théorique. La reproduction a utilisé uniquement des boards jetables. Un premier essai dense sans `--dense-view` ne voyait que 10/80 images et ne prouvait rien ; un essai d'édition avec mauvais binding/aucune sélection a expiré et a été arrêté. Ils sont conservés comme erreurs de méthode, exclus des résultats.

Autre chemin **CONDITIONAL statique** : `huge::Tiles::display` remplace une image par plusieurs régions. Le nombre final de quads peut dépasser le budget GPU même si le nombre d'images logiques est inférieur à 10 001. À inclure dans les tests du correctif ; non reproduit ici.

Proposition à valider : garder le document complet et vivant, rendre un budget déterministe annoncé, ou découper les soumissions avec un budget de vertices explicite. Ne pas simplement supprimer la garde : le buffer est fixe. Aucun silent truncate/sauvegarde tronquée. La policy précise de priorisation (ordre, sélection, qualité, omission temporaire) doit être décidée ; préserver l'Undo et permettre de dézoomer/enregistrer après saturation.

## 5. R3 — visibilité, préparation de frame, architecture

`visibility.rs:11` met en cache les bounds au-delà de 4 096 objets, invalidation par document/génération. Scan O(N) conservant l'ordre. Sonde release, 1 source/asset, 5 visibles dans une scène espacée, 200 queries warm, inputs et résultats passés à `black_box` ; 30 clones pour estimer un snapshot, pas simuler une édition serveur.

| N total | N visible | Scan direct p50 ms | Memo p50/p95/p99/max ms | Refresh initial ms | Capacité memo octets | Clone p50 ms |
| --- | --- | --- | --- | --- | --- | --- |
| 100 | 5 | 0.002 | 0.002 / 0.002 / 0.003 / 0.004 | 0.000 | 0 | 0.005 |
| 1000 | 5 | 0.038 | 0.037 / 0.042 / 0.046 / 0.054 | 0.000 | 0 | 0.209 |
| 5000 | 5 | 0.212 | 0.004 / 0.009 / 0.009 / 0.013 | 0.358 | 393216 | 1.092 |
| 10000 | 5 | 0.443 | 0.008 / 0.008 / 0.012 / 0.026 | 0.507 | 786432 | 2.566 |
| 20000 | 5 | 1.030 | 0.016 / 0.018 / 0.021 / 0.058 | 1.785 | 1572864 | 5.869 |
| 50000 | 5 | 3.462 | 0.039 / 0.042 / 0.044 / 0.125 | 5.744 | 3145728 | 15.522 |

Le memo à 50 000 objets coûte médianement 0,039 ms de scan dans cette scène et ~3 MiB de capacité, contre 3,462 ms pour la query directe. Rebuild initial 5,744 ms, clone 15,522 ms. Il faut distinguer total et visible : le rendu réel ajoute accès objets, preview/selection, sources, LOD et tri. Ces données ne prouvent pas la supériorité du scan sur un Pentium III ou pour toute distribution ; elles ne justifient pas un index supplémentaire sur le poste mesuré.

### Frames natives : déplacement de caméra, chrome normal

Deux runs `--board-tour` de 3 s par taille, Xvfb/NVIDIA sans compositeur, mode immédiat, 800×600, 13 entrées toolbar + status bar, aucune annotation. Grille dense de petites images réutilisant un asset/source. Vue très dézoomée, toutes visibles ; retenir uniquement frames après 1 s et complètement résolues. C'est un contrôle de préparation/native chrome, pas un benchmark photographique représentatif. Source/image bytes ne sont pas multipliés par N. Latences détaillées : `native-pan-latency.json`.

| N total = visible | Frames / 2 runs | Préparation p50/p95/p99/max ms | Supply p50/p95/p99/max ms | Encode p50/p95/p99/max ms | Query p50 ms |
| --- | --- | --- | --- | --- | --- |
| 100 | 199 | 0.286 / 0.380 / 0.411 / 0.430 | 0.100 / 0.119 / 0.135 / 0.147 | 0.120 / 0.177 / 0.199 / 0.209 | 0.012 |
| 1000 | 186 | 1.435 / 1.612 / 1.710 / 1.734 | 0.819 / 0.903 / 0.938 / 0.973 | 0.354 / 0.420 / 0.505 / 0.547 | 0.171 |
| 5000 | 146 | 6.386 / 7.620 / 7.889 / 8.006 | 3.925 / 4.918 / 5.205 / 5.387 | 1.388 / 1.830 / 1.930 / 2.111 | 0.730 |
| 10000 | 119 | 12.658 / 13.182 / 13.639 / 13.937 | 7.735 / 7.992 / 8.300 / 8.787 | 2.707 / 2.932 / 3.018 / 3.257 | 1.455 |

**CONFIRMED** : sur 10 000 références répétées, le bloc supply domine (~61 % du temps médian), avant encodage (~21 %). Il comprend reconstruction des Needs/Demands, admission, lookups CPU/GPU, pinning et choix final de qualité, même lorsque les pixels sont déjà résidents. Aucune stack/allocation count ne permet ici d'imputer précisément les 7,735 ms à une fonction interne. La native query est plus chère que le memo seul.

Contreproposition P1 : séparer et profiler les sous-étapes, réutiliser les scratch vectors, dédupliquer les demandes équivalentes et éviter de recalculer un plan invariant lors d'un redraw d'overlay. L'identité doit inclure géométrie/crop/zoom/sampling/révision/budget/résidences et changements de workers ; un cache indexé uniquement par génération serait erroné pendant chargement/éviction. Le post-upload quality accounting reste obligatoire. Instrumentation temporaire, aucune nouvelle dépendance runtime. **Ne pas ajouter d'index spatial en première intention.**

Cohésion : `product_window.rs` (~1 500 lignes) mélange orchestration rendu/demande/chrome et état, mais des modules existent déjà (`huge`, `local`, `shared`, `menus`, `mouse`). `gpu.rs` (~840 lignes), `image_input.rs` (~1 000), `context_menu.rs` (~1 300) ont des responsabilités identifiables. Extraire d'abord un plan de frame pur et ses critères d'invalidation, pas réécrire dans un framework. Le modèle/history local utilise des mutations réversibles et rollback batch ; **il ne clone pas tout le Document pour chaque commande locale**. Les clones mesurés concernent snapshots/forks et le serveur, pas chaque déplacement preview. La documentation architecture décrivant cinq crates est historique (workspace actuel : neuf) et mérite une mise à jour ultérieure.

## 6. R4 — mémoire fixe et budgets

**CONFIRMED capacité anticipée**, `gpu.rs:320/346` : 10 000 × 6 × 24 octets = 1 440 000 octets (~1,37 MiB) de vertices GPU, et autant de capacité Vec CPU. Le ctor initialise un staging Vec puis le buffer ; capacité CPU réservée ne veut pas dire pages résidentes. Les statistiques de texture ne comptabilisent pas ces ressources ni allocations pilote.

**REFUTED timestamps gardés actifs sans diagnostic** : `configure_diagnostics(false)` met `timings=None`. Le ctor peut créer transitoirement les ressources si supportées (environ 112,5 KiB de capacité de samples, 816 octets de buffers et query sets), puis les libère. Gain startup/résidence exact non mesuré. Lazy vertices ou création tardive de timings : P2, petit gain théorique, aucun résultat permettant de promettre l'élimination des ~300 MiB RSS NVIDIA.

Budgets conservés : supply standard CPU 64 MiB / disque 512 MiB / 2 workers / 16 requêtes ; potato CPU 8 MiB / disque 8 MiB / 1 worker / 4 requêtes / Medium. Upload GPU limité à 16 MiB et 8 uploads/frame ; 3 submissions in flight. Les ressources pilotes, scratch codec, fichiers compressés et cache OS sont hors statistiques de payload. Le rêve 128 MiB motive les budgets ; ce rapport ne démontre pas compatibilité Pentium III ou un budget RSS 128 MiB garanti.

## 7. R5 et R6 — idle, streaming et invalidation

**CONFIRMED statique R5** : `about_to_wait`, `product_window.rs:1213`, poll de 16 ms pour pending/completing/save ou actif ; sinon prochaine vraie deadline / Wait. `dirty=supply_pending` peut répéter la préparation de frames en attente de pixels inchangés. Event notifier existe côté workers. **UNMEASURED** : coût d'un disque artificiellement ralenti / apport exact d'un scheduling uniquement événementiel. Ne pas enlever indistinctement les deadlines Mulot/caret/recovery/lease, ni les polls nécessaires aux complétions GPU.

Mesures idle : après Mulot, principal à 0 tick, jeu/deadline/artwork relâchés ; threads NVIDIA peuvent conserver leurs propres réveils. Deux fenêtres propres simultanées sur logiciel Vulkan, 800×600 standard et 1024×768 potato : chacune 0 redraw applicatif pendant 3 s et 0 tick du principal. Standard 0 tick total et I/O nuls ; potato 8 ticks répartis entre les deux threads llvmpipe et trafic Unix de WSI, sans bytes disque physiques. Cela ne constitue pas un succès « zéro activité de processus » pour ce deuxième intervalle.

### Huge JPEG et PNG — réemploi des fixtures

- **JPEG 50 000×50 000** (2,5 Gpx, 10 000 000 000 octets RGBA théoriques), fichier existant `benchmark-results/phase2a2/fixtures/50000-square.jpg` (~33,4 MB), harness existant `tools/run_phase2a9_huge.py`. Software Vulkan, potato, 800×600, Actual Size, 4 tiles demandées/prêtes : payload CPU/GPU logiques max 2 179 136 octets, RSS idle 121 049 088 octets (~115,4 MiB). Intervalle propre 3 s : tous les ticks/context switches et I/O à zéro. Aucune allocation d'une surface RGBA complète observée via les statistiques ; le pic natif codec/RSS total de cette ouverture n'est pas échantillonné. Receipt `huge-square/receipt.json`.
- **PNG 50 000×4 096**, fixture existante `test_file/phase1l_generated/huge_sweep/50000x4096.png` (~28,1 MB). Deux ouvertures, cache privé neuf puis réutilisé, OS cache non purgé : reconnaissable en 89,6 / 94,2 ms, payload max 91 136 octets ; RSS échantillonné ~114,5 / 111,1 MiB, PSS ~69,9 / 66,6 MiB. Le comportement par défaut utilise les représentations bornées, **pas les tiles PNG**. Ce « warm » ne prouve pas un gain : supply a encore lu la source ; original déjà warm et cache du container réutilisé. Les intervals dirty à 6–9 s contenaient recovery/I/O et ne sont pas des mesures settled-idle.
- Même PNG avec **`--huge-tiles --potato`**, cache privé dédié, save explicite puis 10 s de settle : 4 tiles prêtes/demandées, max payload CPU/GPU 1 139 712 octets, reconnaissable 89,1 ms, RSS échantillonné 117,0 MiB/PSS 72,3 MiB, max callback 37,4 ms (inclut startup/present logiciel). Idle 3 s : 0 redraw, 0 tick de tous les threads, I/O nuls. PNG tiles restent opt-in dans le produit actuel. Receipt `png-tiles-receipt.json`.

Les peaks RSS/PSS PNG sont échantillonnés toutes les ~100 ms **après création de la session et avant idle** ; ils ne couvrent pas nécessairement chaque pic de startup/create/codec. Les bytes disque ne se confondent pas avec `rchar/wchar` (incluent IPC). Pas de cold OS cache, aucun file-cache drop autorisé/inventé.

Régression supply : la suite rerun couvre source révisée, completion stale, annulation et re-demand, thrash de viewport, admission priorisée, bounded caches, save/reopen de huge PNG linked/embedded, fingerprint/current generator. Les tests opt-in GPU `lod_convergence` (2) et `lod_long_churn` (1) passent sur logiciel Vulkan ; thousands uploads/evictions avec pixel readback et re-entry/révisions. `source_transition` passe pour linked→embedded→linked→refresh et monotonie après Undo. **REFUTED dans cette couverture** : ancien LOD qui pourrait être republier avec la mauvaise révision par ces chemins testés. Ne pas extrapoler à toute modification externe sans refresh/notification, à une course LAN réelle ou à tous les codecs. R6 global = **CONDITIONAL**, protéger la mécanique existante ; pas de réécriture du streaming.

## 8. Serveur — observation seulement

`authority.rs:223/228` clone document et state avant mutation/publish. `persistence.rs:32` sérialise l'état complet, relit/domain-valide, écrit pending exclusif/checksum, fsync fichier, rename, fsync répertoire Unix. `service.rs:293` acquiert `Mutex<Hub>` pour la durée du contrôle, incluant certaines publications synchrones. **CONFIRMED** amplification avec taille d'état et sérialisation de contrôles ; **UNMEASURED** débit, p95 ack, contention entre boards et coût SSD/HDD. Les clones de la sonde locale ne sont pas des mesures serveur.

C'est une transaction volontaire : clock/conflits, source bindings, idempotence, inverse/Undo, révision monotone, publication atomic et ack après durability. Une erreur de sync après rename empoisonne l'autorité plutôt que réutiliser une révision incertaine. Aucun benchmark serveur pendant l'acceptation LAN. Les tests automatisés d'autorité préexistants utilisent leurs propres états temporaires ; aucune connexion au serveur utilisateur. Phase ultérieure séparée : petites/grandes authorities, multi-board, breakdown clone/encode/validate/write/fsync/lock, tout en préservant exactement les garanties. Pas d'in-place mutation ou suppression de fsync pour gagner un score.

## 9. Matrice UI — baseline et correctifs autorisés

Au baseline, Actual Size/Grayscale/Reset Crop étaient déjà batchés et dispatchables pour all-image multi-selection, mais absents du menu Selection. Le guard public refusait toute mutation dès qu'une image sélectionnée était verrouillée : le calcul grayscale incluant ces objets était un problème **latent/CONDITIONAL**, pas une preuve de toggle public incorrect malgré le guard.

Correction : sous-menu **Image**, un seul niveau, dans Selection uniquement all-image. Actual Size, Grayscale, Reset Crop ignorent explicitement les images verrouillées ; le status annonce leur nombre inchangé. Grayscale dérive sa cible des seules images éditables, check lorsque toutes celles-ci sont grises. Zéro éditable : commandes grisées, pas d'history vide via keymap. Aucun pixel re-encode.

| Action | Menu / contexte | Dispatch / résultat et locks | Undo / données partagées | Validation |
| --- | --- | --- | --- | --- |
| Actual Size / 1:1 | Single existant ; multi Image ajouté | Par image : pixels source × fractions crop ; conserve centre/rotation/flips/crop ; skip locked annoncé | Un Batch/Undo ; SetTransform existant | 1/2/25 hétérogènes ; native 25 + Undo exact |
| Grayscale | Single/multi ; check des éditables | Tous gray → couleur ; color/mix → gray ; locked ignorés pour cible et écriture | Batch SetGrayscale ; métadonnées seulement | All-color/all-gray/mix ; locked couleur + gray éditable ; zéro éditable |
| Reset Crop | Single/multi ajouté | Crop FULL, aucun déplacement ni rebinding ; skip locked annoncé | Batch SetCrop | 1/2/25 et locked ; Undo exact |
| Flip H/V | Selection existant + Image multi | Toggle individuel ; tous objets flippables exigés dans menu ; refuse lot entier si any locked | Batch transforms / un Undo | 1/2/25 + refusal locked |
| Opacity presets | Image multi ajouté | Normalise images au preset ; refuse lot entier si any locked | Batch SetOpacity / un Undo | 1/2/25 à 50 % + refusal locked |
| Opacity gesture | Keymap hold existant | Preview puis commit ; contexte Canvas plus large que menu ; pas modifié | Gesture history existante | Régressions existantes ; nouveau test natif mixed/gesture non réalisé |
| Sampling presets / cycle | Presets dans Image ; cycle keymap | Preset partagé ; cycle individuel Default→Smooth→Nearest→Default ; all-image guard, locked refusent le lot | Metadata Batch / un Undo ; ne re-encode pas les pixels | 1/2/25 Smooth/cycle + locked ; matrix mixed valeurs partiellement statique |
| Toggle Lock Selection | Menu/keymap existants | Si au moins un unlocked : lock tous ; si tous locked : unlock tous ; action spéciale multi-types | Batch SetLocked | Régression metadata existante + autorité durable |
| Image + Frame/text/annotation | Pas de sous-menu Image pour mix | 1:1/gray/crop/filtering refusés par ImageSelection ; flips menu tous flippables | Aucune history pour gray mix Frame | Nouveau test Image+Frame ; autres types par guard/code/tests existants |
| Groups et source actions | Group/Ungroup existants ; sources single gates conservés | Actions sur ids sélectionnés ; ne changent pas memberships ; sélection/prune existants | Batch inchangé | Suite groupe/selection existante ; pas nouveau test GUI groupé |
| Application menu / keymap | F10 reste application ; Image au context Selection | Routing menus.rs → même ImageInput dispatcher ; aucun troisième niveau | Même boundary historique/shared | Context/menu tests + native Right click/25 ; keymap F5/Undo |

Incohérences restantes identifiées, sans élargir les corrections : presets opacity via keymap Canvas filtrent silencieusement les images d'une sélection mixte ; flips via keymap filtrent les objets flippables alors que le menu refuse un mix non flippable. Pas une corruption ou exception reproduite ; policy de partial action doit être explicitée dans un correctif distinct. Flip/opacity/filtering restent refusés si un verrou est présent ; les trois actions corrigées signalent le skip. Les presets sampling/opacity ne montrent pas de check uniforme pour multi-selection (baseline single-only), à conserver comme limite UI plutôt que prétendre une tri-state généralisée.

LAN : les commands transform/crop/gray/filtering/opacity/lock restent des DTO existants, un Batch unique, aucune nouvelle commande ou schéma. Le test préexistant `grayscale_and_object_lock_are_atomic_shared_edits_with_durable_undo` et tests batch/conflits passent. **Pas d'acceptation UI sur LAN réel ni nouvelle preuve spécifique 25 images partagées.** Limite DTO flat Batch de 4 096 edits versus limite locale 200 000 : les 25 passent la représentation ; les énormes lots partagés ne sont pas promis.

Validation de cette mission : fmt ; Clippy workspace/all-targets/all-features `-D warnings` ; tests workspace/all-features : **480 passed, 0 failed, 20 ignored**, 74 suites (y compris doc-tests vides) ; check app/no-default-features/all-targets ; tests GPU LOD supplémentaires **3 passed**. Cinq nouveaux tests UI (scénarios 1/2/25 et locks), et régressions context-menu/2A9 : 22 tests ciblés réussis. Build release preview réussi, laissé dans target, pas installé. Native Xvfb/logiciel Vulkan : menu Image réel, grayscale sur 25, keymap 1:1 sur 25, centres préservés, Undo exact, history finale 0 et dirty=false. Receipt `native-ui-pass/receipt.json`, capture `ui-25-image-submenu.png`.

Les premiers essais GUI étaient invalides : profil historique avec bouton droit lié à PanView, puis sélection du submenu File à la place d'Image. Profil isolé corrigé et item Image identifié avant essai final. Ces erreurs de harness ne sont pas des bugs produit et ne sont pas comptées comme réussites. Les tests menu/data ne remplacent pas une acceptation physique Windows.

Backlog desktop séparé : right-drag sur canvas vide sous X11/Xfwm4 et Always on Top par cible restent des fonctions à diagnostiquer/implémenter dans leur propre mission. Pas d'Alt+right-drag resize (demande retirée), pas de changement de WM, pas de lien établi avec Turbo.

## 10. Contreproposition et plan à approuver

| Ordre | Mission distincte | Motivation / objectifs mesurables | Risque / effort | Acceptation et rollback |
| --- | --- | --- | --- | --- |
| P0 | Budget de rendu non fatal | Éliminer le cas reproduit 10 001 et l'édition perdue ; compter aussi les quads tiles | Moyen ; policy UX explicite, patch ciblé | 9 999/10 000/10 001/20 000, source répétée/distincte, tile expansion ; edit/save/Undo continuent ; ordre/alpha et doc complet conservés ; feature switch ou revert isolé |
| P1a | Préparation de demandes et réemploi des plans | Décomposer les 7,735 ms ; viser au moins −30 % du bloc supply médian/p95 à 10k répété, sans régression à 100/1k ou source distincte | Moyen ; invalidation et post-eviction sont critiques | Bench A/B au même SHA, 2+ runs, pan/zoom/metadata/crop/refresh/churn ; LOD pixel readback, huge fixtures et zéro idle ; conserver chemin simple comme fallback |
| P1b | Batching consécutif conservant l'ordre | À 10k répété viser −50 % d'encode ou rejeter le patch ; unique doit garder p95/mémoire proches du baseline | Faible à moyen ; limité au renderer | Alternance A/B/A, alpha/crop/flips/grayscale, annotation interleave, nearest/smooth, budgets, logiciel et vrai GPU ; revert indépendant |
| P1c conditionnel | Textures uniques | 10k unique : 8,724 ms encode / 10,302 ms total offscreen ; profiler binds/lookups/backend avant choisir technique | Investigation, puis éventuellement élevé | Vrais assets différents, budget VRAM borné, lower-end adapter et Windows ; rejeter toute dépendance hardware non prévue ; aucun gain promis par batching simple |
| P2 | Scheduling pending / scratch / resources lazy | Gain réel de wakeups, allocations, RSS/startup à mesurer ; petit buffer fixe ~1,37 MiB | Faible pour scratch, moyen pour deadlines | Slow-worker isolé, multiwindow, no-diagnostics ; aucun timer permanent ou starvation ; baseline et polling fallback conservés |
| Différé | Durability/lock serveur | Seulement après fin d'acceptation LAN et autorisation séparée | Élevé si architecture transactionnelle change | Ack durable, kill/restart, directory-sync failures, idempotence/conflicts/leases/Undo ; jamais fsync retiré pour vitesse |
| Déjà autorisé | UI corrective | Compact Image submenu et cohérence gray/locks/1:1 | Faible, réalisée sans livraison | Tests et preview validés ; diff de trois fichiers + cinq tests, revue/revert indépendants |

Objectifs de gain ci-dessus = **critères proposés**, pas résultats acquis. Mesurer aussi le coût à 100 images ; une optimisation de stress ne doit pas pénaliser le quotidien. L'index spatial, l'atlas global, un moteur de jeu, une refonte du framework et la suppression des garanties serveur sont rejetés pour cette première passe. L'immédiat global et la réduction arbitraire de cadence Mulot ne sont pas justifiés par le réel.

### Tableau des préoccupations

| Point | Verdict | Preuve et échelle | Alternative minimale |
| --- | --- | --- | --- |
| Mulot / Xvfb | CONFIRMED effet de présentation ; REFUTED simulation disproportionnée sur le réel ; UNMEASURED stack exacte | A/B physique et Xvfb, composité et non, instrumenté/non, FIFO/immediate, contrôle hover | Garder le jeu ; profiler le backend de test avant changement produit |
| R1 draws | CONFIRMED scaling ; CONDITIONAL gain du batching | gpu.rs:769 ; 100→10k quads, répétées et distinctes | Batcher seulement les voisins compatibles ; investigation séparée pour unique |
| R2 cap | CONFIRMED P0 | product_window.rs:294 / gpu.rs:677 ; ouverture 10 001 et duplication perdue | Budget explicite non fatal, préserver le document ; tester expansion tiles |
| R3 visibilité/frame | CONFIRMED scan O(N), supply dominant ; UNMEASURED avantage d'un index | visibility.rs:11 ; 50k total/5 visibles et 10k visibles/119 frames natives retenues | Garder le memo ; breakdown/reuse/dedup supply avant index |
| R4 baseline mémoire | CONFIRMED réserve vertices ; REFUTED timings durablement conservés sans diagnostic ; UNMEASURED gain RSS lazy | gpu.rs:320/346/450 ; capacité calculée, RSS contrôlé sans attribution fine | Petites allocations lazy seulement si gain mesuré |
| R5 attente/idle | CONFIRMED polling pending et retour à Wait ; CONDITIONAL coût évitable | product_window.rs:1213 ; true-idle JPEG/PNG, retour Mulot ; activité pilote multiwindow séparée | Notification workers avec fallback de complétion ; préserver vraies deadlines |
| R6 image pipeline | CONFIRMED streaming borné dans les fixtures ; CONDITIONAL couverture globale | 50k² JPEG et 50k×4096 PNG, source transitions et tests pixel LOD | Garder codecs, budgets, révisions et rejection stale ; combler les gaps ciblés |
| Serveur | CONFIRMED amplification/verrou par lecture ; UNMEASURED coût | authority.rs:223/228, persistence.rs:32, service.rs:293 | DEFER UNTIL LAN ACCEPTANCE COMPLETE ; benchmark indépendant ultérieur |

### Couverture encore manquante

Allocation counts et stack CPU interne ; source-distinct native pan/selection/annotations à 5k/10k ; corpus HD photographique complet ; CPU/GPU réellement faibles ; perf physique 1024×768 et autres drivers/Windows ; sustained pending/slow-I/O ; cold OS cache ; pics RSS exacts startup/codec ; attribution VRAM par processus ; acceptation LAN multi-selection et toutes gestures mixtes. Ces cellules sont **UNMEASURED**, pas des succès ni des défauts confirmés. Les receipts mesurent des workloads explicitement définis et ne prouvent pas « toutes images gigantesques sans broncher ».

### Reproduction et conservation

Commandes de sonde et scripts temporaires sont dans le répertoire ignoré de cette mission. Ils créent leur propre display/profil/boards ; les scripts non idempotents refusent certains dossiers existants. Ne pas lancer directement `physical.py` : une nouvelle prise de contrôle physique exige un nouvel accord et une fenêtre annoncée. Échec initial/contrôles invalides conservés dans les logs. Reports/fixtures bounded et originaux lourds réutilisés, aucune copie de dataset.

Inventaire/hashes et état final sont dans `final-receipt.json` / `evidence-manifest.json`. Les scripts temporaires ne sont pas ajoutés à `tools/` ni aux dépendances. Aucun client/serveur actif tué, aucune connexion à la session LAN d'acceptation ; baseline `bin/tack` inchangée, assets authored préservés, pas de commit/push.

**Décision demandée : je recommande P0 puis P1a et P1b, et une investigation séparée P1c ; P2 reste soumis à preuve, serveur différé. Approuver ou ajuster ce périmètre avant toute optimisation. La phase diagnostic s'arrête ici.**
