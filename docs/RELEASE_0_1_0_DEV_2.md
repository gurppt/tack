# Tack 0.1.0-dev.2 — portable pre-alpha

Cette prérelease GitHub est explicitement autorisée par l’utilisateur, avec commit/push et priorité au binaire Windows. Elle réunit le nettoyage UI, le correctif P0 de saturation, P1a supply et P1b draw batching ; les mesures et limites figurent dans les rapports de mission séparés. La présentation README existante est conservée, avec liens de téléchargement et numéro de version actualisés.

## Contenu et identité

Deux archives fraîches : `tack-linux-x86_64.zip` et `tack-windows-x86_64.zip`. Garder le dossier complet : Tack, serveur LAN optionnel, updater séparé, helper libjpeg-turbo 3.2.0, About/logo, icons/cursors/mouse-game et notices. Les PNG authored correspondent aux exemplaires bin/gfx au contrôle ; aucun original existant n’est régénéré ou remplacé. Les quatre glyphes d’alignement sont nouveaux. Les boards/profils/caches et briefs privés ne font pas partie des archives.

Le build-info des trois executables doit correspondre au commit de publication, au canal dev, à la version 0.1.0-dev.2 et au major courant 4. Le packager refuse un worktree tracked modifié, une identité incorrecte, une provenance SIMD manquante, une archive >128 MiB et les candidats existants ; le manifest fusionné vérifie les deux archives et leurs SHA256. Le checksum du helper est vérifié contre sa provenance native. Le packager lit les constantes shared/update et refuse leur divergence, plutôt que garder l’ancien major 3 codé en dur.

L’ajustement du major dans tack-update est une correction d’identité de release ; aucun format de message ni service LAN n’est modifié. Les versions de crates internes et Cargo.lock changent ensemble ; aucune dépendance externe mise à jour. Anciennes connexions major 3 incompatibles avec le major 4 actuel.

## Validation de source

490 tests Rust réussis, 0 échec, 21 ignorés spécialisés ; Clippy all-targets/all-features -D warnings, check local-minimal all-targets, rustfmt et diff check réussis. 31 tests Python réussis, dont contrôles du manifest/packaging. Les tests GPU et natifs P0/P1a/P1b sont documentés dans leurs rapports : pixels exacts, streaming/LOD/revisions, sauvegarde/recovery à saturation et repos sans redraw.

Les builds finaux Linux/Windows sont effectués **après commit**, puis vérifiés en packaging. Le Windows GNU doit inclure le JPEG SIMD SSE2/AVX2 statique, sans dépendance aux DLL de runtime GCC/turbojpeg ; l’exécution CLI et les sorties scalar/SSE2/automatic du decoder sont contrôlées sous un Wine prefix privé. Wine et Xvfb ne constituent pas une acceptance du bureau Windows physique. Les reçus d’archives, imports, Wine, manifest et upload restent dans `benchmark-results/release-dev2/` et les checksums de publication dans les assets GitHub.

La release est une prérelease de test, pas une alpha de production. Tests Windows physiques, ressenti utilisateur et LAN multi-machine restent en cours/pending ; les tests locaux n’en tiennent pas lieu. Le serveur et la fenêtre LAN existants ne sont pas redémarrés et bin/tack utilisateur n’est pas remplacé pendant cette publication.

## Ressources

Pour garder la réserve de disque, seuls 105 anciens exécutables de tests (7,64 GiB) identifiés par les logs de cette session sont retirés avant les builds finaux. Les 68 executables de tests actuels sont conservés, ainsi que caches partagés, fichiers utilisateur et fixtures utiles. Aucun cargo clean global. Les grands JPEG/PNG sont réutilisés, sans copie dans les packages. Wine utilise une scratch privée, bornée et supprimée après ses contrôles ; aucun prefix Wine utilisateur n’est utilisé.

Publication prévue : [Tack v0.1.0-dev.2](https://github.com/gurppt/tack/releases/tag/v0.1.0-dev.2). La publication n’est considérée achevée qu’après présence des deux archives, manifest et checksums vérifiés sur GitHub.
