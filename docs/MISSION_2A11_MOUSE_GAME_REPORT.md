# Phase 2A11 — Mulot Mouse Game

Implémentation : **768b921a427a1f2082c67750a6bc5a92dbceccf2**, après le socle
`6d496ef` et le correctif de placement/empilement `897aba4`. Les binaires Linux
et Windows identifient ce commit, version `0.1.0-dev.1`, protocole LAN 4 et
schéma document inchangés. Le README public et les autres fichiers utilisateur
préexistants restent hors de ces commits ; leur présence explique le marqueur
`worktree_dirty` du build.

## Résultat et précisions utilisateur

Mulot reste un extra local, absent de la toolbar et des raccourcis par défaut.
Son libellé est **Mulot**, avec son icône existante, dans Keymap et en fin de
catalogue Edit Toolbar, marqué discrètement `EXTRAS`.

Les précisions données pendant cette phase prennent priorité sur les timings
et positions du brief initial :

- **Toutes les animations à 12 images/s** : 83 333 333 ns par frame, sans
  interpolation. La simulation de poursuite utilise des pas séparés de 33 ms,
  plafonnés à 50 ms après retard. Les échéances d'animation/fumée peuvent
  provoquer un pas intermédiaire ; aucun rattrapage non borné.
- Stationnement exclusivement **latéral**, à gauche ou à droite du fromage.
  Les deux canevas 16×16 sont affichés en ×2, soit 32×32 pixels physiques.
  Le hotspot fromage est (14,14). Pour un pointeur P, le centre du fromage est
  P+(2,2), et l'ancrage bas-centre du Mulot est P+(-32,18) à gauche ou
  P+(36,18) à droite. Les canevas ont **2 pixels physiques d'écart**, un pixel
  source en ×2. Leur centre vertical est identique.
- À l'entrée en frame 11 d'Attack, le Mulot rejoint **horizontalement le centre
  du fromage figé**, une seule fois. Depuis ce stationnement, le déplacement
  est de 34 pixels physiques ; il remplace l'ancienne approximation de 12 px
  du brief. **Aucun changement de hauteur**. Les frames de réception, Munch et
  Explosion gardent cet ancrage final ; déplacer le pointeur ensuite ne le
  redirige pas.
- Empreintes **noires**, espacées de 28 pixels logiques au lieu de 18, durée
  800 ms. La silhouette PNG est conservée ; la teinte est appliquée en mémoire.
- Empilement du fond au premier plan : **empreintes → fumées → Mulot**.
- Completion et annulation restaurent **le curseur pixel Pointer de Tack**.
  La visibilité reste limitée à la fenêtre ; aucune suppression globale,
  capture ou déformation du pointeur OS.

## Assets et rendu

Les exports livrés sont **44 PNG individuels numérotés**, pas des spritesheets.
Ils ont été inspectés et copiés à l'identique depuis
`bin/gfx/icons/work_icons/mulot_sprite` dans `gfx/mouse-game` :

| Fichiers | Frames | Dimensions source |
| --- | ---: | --- |
| `animated_mulot_run1..3.png` | 3 | 16×16 |
| `animated_mulot_idle1..3.png` | 3 | 16×16 |
| `animated_mulot_attack1..15.png` | 15 | 16×16 |
| `animated_mulot_munch1..2.png` | 2 | 16×16 |
| `explosion1..12.png` | 12 | 16×32 |
| `smoky_a1..4.png` | 4 | 8×8 |
| `smoky_b1..4.png` | 4 | 8×8 |
| `cheese_cursor.png` | 1 | 16×16 |

Seuls les noms source `smokyb1..4.png` ont été normalisés en `smoky_b1..4.png`.
Ordre numérique explicite, padding intégral, alpha partiel et frames finales
transparentes préservés. Le Mulot est orienté à gauche et retourné lorsqu'il
court à droite ; au stationnement il regarde le fromage. La fumée est authored
vers la droite et retournée pour un trajet vers la gauche. Chaque puff garde
sa position et son orientation d'émission ; A/B alternent par puff.

Rendu ×2 dans les primitives bitmap existantes : masques couleur précalculés,
aucune nouvelle texture GPU, aucun shader/codec/dépendance ajouté. Ancrage
bas-centre commun ; l'explosion 16×32 s'étend vers le haut. Contrat complet dans
[gfx/mouse-game/README.md](../gfx/mouse-game/README.md).

Le worker local existant charge l'art uniquement à l'activation, dans son unique
slot d'opération. Résultats associés à un ticket ; les résultats annulés ou
anciens sont rejetés. Limites : 64 KiB par PNG, dimensions exactes, 64 masques
par frame. Résidence effective des masques et du fromage : **26 896 octets**,
libérés à la sortie. Pas de watcher ni de worker permanent pour le jeu.

Les builders Linux/Windows installent seulement les PNG manquants, et les
packages/updater incluent `gfx/mouse-game` en conservant les remplacements
existants. **214 PNG éditables préexistants** vérifiés inchangés par SHA-256.
Les 44 exports, copies source et deux dossiers runtime ont des SHA identiques.

## États, mouvement et échéances

`SpawnDelay → Run ↔ Idle → Attack → Munch → Explosion → PostExplosionWait → Pointer`.

Délai de spawn : 3 s, puis une entrée juste hors d'un bord pseudo-aléatoire,
aléa de session uniquement. Un seul Mulot ; aucune réapparition autonome.
La poursuite vise l'ancrage latéral, conserve son côté et bascule si le bord
rend ce côté inaccessible. Accélération : erreur plafonnée à ±160 px par axe,
coefficient 9 ; amortissement exponentiel 4,5/s ; vitesse maximale 360 px/s.
L'orientation ne suit la vitesse horizontale qu'au-delà de 8 px/s.

Idle entre à moins de 3 px de l'ancrage et sous 24 px/s, après au moins 250 ms
de Run ; le placement final est exact. Sortie si déplacement du fromage de
plus de 3,5 px depuis le **repère ancré** ou erreur de position supérieure à
24 px. Un jitter horizontal ±3 px garde le dwell tout en déplaçant le dock.
Capture après 4 s qualifiées ; un fromage immobile mais distant ne suffit pas.

Attack affiche ses 15 frames une fois : fromage visible en frames 1–10 ; à
l'entrée en 11, curseur invisible et translation horizontale unique. Munch
alterne ses deux frames pendant environ 3 s. Explosion affiche ses 12 frames
une fois. Puis **une seule échéance à +1 s**, sans boucle de redraw pour cette
attente, restaure le curseur Tack et active réellement Pointer. Le garde de
présentation exige que chaque frame Attack/Munch/Explosion ait été présentée
avant d'avancer ; un retard étire le cycle au lieu de sauter une frame.

Fumée : au-dessus de 80 px/s, émission selon une distance de 20 px, pool 24,
quatre frames par puff (~333 ms). Empreintes : pool 64. Aucun effet ne devient
un objet document. Aucun message LAN ajouté.

Escape, changement explicite d'outil, perte de focus, occultation, fermeture
et erreurs fatales passent par la libération du jeu et la restauration du
curseur. Les outils temporaires sont aussi libérés pour que Pointer ne soit
pas masqué par un ancien hold de Mulot. Les contrôles UI continuent à fonctionner.

L'Easter egg `poo_easter.png` reste un import/image ordinaire, au plus une fois
après 600 s continues, avec `Puzzo puzzo !`. Une partie terminée quitte Mulot
bien avant ce délai ; il faut prolonger une poursuite sans capture pour
atteindre dix minutes. Aucune prolongation artificielle après Explosion.

## Validation

- `bash tools/check.sh` sur le code final : **475 tests Rust**, **31 tests Python**,
  formatage, check, Clippy, documentation et cargo-deny 0.20.2 passent.
- **15 tests Game**, plus 4 tests historiques Mouse : chronologie complète,
  frames 10/11, cadence 12 FPS, landing horizontal des deux côtés, stationnement
  depuis les quatre bords, marges latérales, jitter cumulé, vitesse, fumées,
  empreintes/empilement, annulation de chaque état, hold temporaire, Easter egg.
- GPU smoke explicite sur Vulkan logiciel : passe.
- **28 contrôles production X11** sur **NVIDIA RTX 2060/Vulkan**, fenêtres privées
  Xvfb/xfwm4 en **800×600 et 1024×768**, profils/corpus dédiés. Lecture native du
  curseur via XFixes : fromage visible en frame 10 ; alpha entièrement nul dès
  frame 11 et pendant Munch, Explosion et l'attente ; retour exact du PNG Pointer
  de Tack. Escape pendant le délai et pendant l'invisibilité, perte de focus
  pendant l'invisibilité : restauration sans reprise automatique.
- Les 15 frames Attack et 12 Explosion ont toutes un accusé de présentation.
  Gap latéral 2 px et landing sur le fromage vérifiés dans les traces.
  Munch : **3000,043 / 3000,048 ms** ; attente finale :
  **1000,040 / 1000,029 ms**, respectivement 800×600 / 1024×768.
- Windows x64 GNU : compilé avec JPEG SIMD statique SSE2/AVX2. Exécution CLI
  Wine : création/réouverture d'un board JPEG, sorties scalar/SSE2/automatique
  identiques. Cela ne constitue pas une acceptation desktop Windows physique.

[Receipts et mesures](../benchmarks/phase2a11/receipt.json),
[chronologie des frames](../benchmarks/phase2a11/frame-timeline.json),
[captures 800×600](../benchmarks/phase2a11/game-800x600-timeline.png),
[captures 1024×768](../benchmarks/phase2a11/game-1024x768-timeline.png).
Les captures de fenêtre excluent le curseur OS ; les lectures XFixes séparées
prouvent sa présence/invisibilité. Captures intégrales/logs conservés dans
`benchmark-results/phase2a11/native-corrected` (ignoré par Git).

## Performance, limites et disque

Mesures `/proc` sur le même corpus minuscule, diagnostic activé, écran privé.
CPU exprimé en pourcentage **d'un seul cœur**, pas du processeur complet :

| Taille | Inactif | Poursuite | Idle animé | Après sortie |
| --- | ---: | ---: | ---: | ---: |
| 800×600 | 0 % | 83,6 % | 36,5 % | ~1 % total pilote, 0 tick thread principal |
| 1024×768 | 0 % | 100 % | 45,6 % | ~0,5 % total pilote, 0 tick thread principal |

Ce coût actif sur NVIDIA/Xvfb est **élevé**. La médiane CPU de préparation/rendu
Tack est de 0,208 / 0,254 ms, tandis que `present` prend 23,33 / 33,33 ms avec
activité CPU du pilote. Ces observations ne permettent pas de promettre un
faible coût sur un desktop physique : l'acceptation de fluidité/coût sur ce
desktop reste nécessaire. Les mesures incluent la télémétrie active.

Après sortie : game absent, zéro masque résident, aucune échéance, zéro redraw
dans l'intervalle calme observé, zéro tick du thread principal, aucun I/O mesuré.
Des threads NVIDIA peuvent encore consommer 1–2 ticks pendant les 2 s observées ;
ne pas présenter cela comme zéro CPU de tout le processus. Les sockets observés
sont Unix X11/pilote, aucun nouveau socket réseau du jeu. RSS ~300–306 MiB :
mesure totale application/pilote, pas une attribution mémoire/VRAM du jeu.

L'appréciation humaine du mouvement et la vérification Windows physique,
Wayland/DPI et interruptions fatales injectées restent des points d'acceptation.
Le routage fermeture/erreur est couvert par le code et les tests de teardown ;
ne pas le présenter comme un test physique de panne GPU. Une revue indépendante
avant publication alpha reste à faire. Aucun release public n'a été publié.

Artifacts compacts sous 512 MiB. Le seul écart temporaire est le préfixe Wine
isolé (~1,6 GiB, plafond explicite 2 GiB), supprimé après chaque test. La réserve
10 GiB est restée préservée ; ~19 GiB libres après les builds. Aucun nettoyage
large de caches/utilisateur, aucun `cargo clean`, aucun changement d'icône source.

## Binaires

- Linux : `bin/tack`, SHA-256
  `5eaa5713475511b2674d5544c8f2db93b216e6c5cc23453ac5d38bbe66d19fb3`.
- Windows : `bin/tack-windows-x86_64.zip`, SHA-256
  `c31a3dfc79a7af58e8ae4ca50fcf09b40f4adfab4730e3bd5300860eee3dabbe`.

La phase s'arrête ici : pas de scoring, audio, collision, respawn, multijoueur,
réglage de jeu ou fonctionnalité desktop supplémentaire.
