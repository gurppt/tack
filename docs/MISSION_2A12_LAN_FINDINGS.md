# 2A12 — Examen LAN complémentaire

L’utilisateur a demandé un examen des problèmes LAN possibles avec les missions Turbo. Cet examen est **statique**, complété par les tests isolés existants du workspace ; il n’implémente pas d’optimisation LAN. La session de test utilisateur active (serveur bin/tack-server PID 17206 au contrôle) reste en place : aucun input, requête de test ni redémarrage sur son endpoint.

## Risques de coût confirmés dans le code

`service::control` prend le Mutex de Hub avant de dispatcher Edit/Undo/Redo. `Authority::edit` clone le document puis l’état/historique ; `commit` reconstruit le record. `persistence::publish` sérialise l’état complet, le reparcourt/domain-valide, écrit/checksum le fichier, fsync, rename puis fsync du répertoire Unix. Ces opérations se font sous le verrou Hub : un gros board ou disque lent peut allonger la réponse d’un client et bloquer les contrôles d’autres boards pendant cette opération. La contention et la latence réelle multi-machine n’ont pas été mesurées ici ; pas de chiffre inventé à partir des microbenchmarks locaux.

Le client reconstruit aussi des structures/scopes de commandes et une liste de sources visibles pour le partage. P0 ne transmet au supply visible que le sous-ensemble affiché, tout en gardant le document partagé complet. P1a n’a pas modifié `request_visible`, le protocole, les scopes ou les leases. Le gain local d’une vue homogène ne supprime donc pas le coût d’une mutation partagée acceptée.

Les limites de batch du DTO partagé (4 096 commandes à plat, et quotas propres d’inverse/scopes) sont distinctes de la limite locale document de 100 000 et du budget de présentation de 10 000 quads. Une sélection locale énorme n’implique pas que son opération partagée soit admissible. Le rejet doit conserver l’édition/board et rester explicite ; il serait incorrect d’augmenter ces limites à la faveur de P0. Les tests de scopes, conflits, dedupe, join/restart et snapshots restent présents et passent dans le workspace, sur leurs propres services/ports.

## Problème de release identifié

Le protocole partagé courant est **major 4** dans `tack-shared`, alors que l’identité de manifest/updater et le packager alpha étaient encore figés sur **3**. Une publication canonique ne pouvait pas valider les identités app/server/updater ensemble. Ce décalage est corrigé dans le travail de release autorisé par l’utilisateur : l’identité update rejoint le major courant, le packager vérifie les deux constantes et les trois executables. Il ne s’agit pas d’un nouveau protocole LAN ni d’une compatibilité avec des peers major 3.

## Suite proposée, non implémentée

D’abord mesurer la latence sous mutex (clone, conversion, validation, serialization, publication, broadcast) sur deux processus privés puis deux machines consentantes, tailles/commandes représentatives et disque contrôlé. Ensuite seulement comparer des changements ciblés : isoler le verrou par board ou raccourcir sa section sans rompre l’ordre des revisions/durabilité/dedupe ; éviter les reconstructions redondantes avec preuves d’invalidation ; comparer un journal à la réécriture complète uniquement dans un brief spécifique. Ne pas déplacer un fsync hors transaction ou rendre un ACK visible avant durabilité pour obtenir un benchmark flatteur.

Aucune modification de `tack-server`, des messages/scopes/leases ou du transport n’est livrée par l’examen. Les tests physiques LAN, contention réelle, latence/réassemblage sur réseau perturbé et la durabilité après coupure restent des compléments d’acceptance ; aucun résultat local ne les remplace.
