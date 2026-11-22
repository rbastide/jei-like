# Enough Items: un JEI pour Horizon

Un mod natif (Rust) pour Horizon 26.3 (build `2b77287`) qui affiche tous les items dans un panneau
à droite des inventaires, et leurs recettes.

## Utilisation

| Action | Effet |
|---|---|
| Barre de recherche (en bas à droite) | filtre par nom ou id; `@mod` filtre par mod, `#tag` par tag; plusieurs mots = tous requis |
| Clic gauche / **R** sur un item | recettes qui fabriquent l'item |
| Clic droit / **U** sur un item | recettes qui utilisent l'item |
| R / U marchent aussi sur les items de ton inventaire ou d'un coffre | |
| Onglets en haut de la page | Établi, Four, Haut fourneau, Fumoir, Feu de camp, Tailleur de pierre, Table de forgeron, Alambic |
| Clic sur un ingrédient dans une recette | navigue vers ses recettes (clic droit : ses usages) |
| Back / Échap | page précédente |
| Bouton « + » d'une recette d'établi | place les ingrédients dans la grille de craft ouverte |

Recettes couvertes : vanilla, celles des mods de contenu (`.hmod`) et des types que le jeu sait lire.
Sur un serveur, ce sont les recettes du client (vanilla + mods), pas celles des datapacks du serveur.

## Fichiers

- `horizon-mod.toml`: manifeste (id `recipes`, pins `[native]`, module et mixins)
- `src/viewer.rs`: toute la logique, injectée dans le jeu comme `crate::game::enough_items`
- `src/mixins/tick.rs`: appelé à chaque tick client (indexe les tables, affiche la liste)
- `src/mixins/panel.rs`: appelé à chaque événement du panneau
- `pack.py`: crée `recipes.hmod` et l'installe dans l'instance Horizon

## Recompiler après une modification

```sh
python pack.py
"%LOCALAPPDATA%\Programs\Horizon Launcher\horizon-launcher.exe" native-build --instance horizon --trust-native-code
```

Le mod est lié à ce build exact de Horizon (il utilise des mixins). Après une mise à jour de Horizon,
change `horizon` et `mixins` dans `[native]` (voir `native/sdk/<commit>/sdk.json` du launcher) et
recompile.
