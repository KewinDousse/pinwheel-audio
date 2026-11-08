# Architecture

## Deux processus, un exécutable

| Mode | Lancement | Rôle |
|:--|:--|:--|
| Arrière-plan | `sounddock.exe` | Icône, hook de la molette, raccourcis, affichage. Une seule instance (mutex `Local\SoundDock.Main`). |
| Paramètres | `sounddock.exe --settings` | Fenêtre egui. Une seule instance (mutex `Local\SoundDock.Settings`). |

La fenêtre de paramètres est un processus à part : la bibliothèque d'interface (egui, wgpu) n'est chargée que quand elle est ouverte, et le processus d'arrière-plan reste léger.

Communication :

- Paramètres → arrière-plan : messages postés à la fenêtre cachée de classe `SoundDock.Main` (`crates/app/src/ipc.rs`). `WM_RELOAD` après chaque enregistrement, `WM_PREVIEW_OSD` pour l'aperçu.
- Arrière-plan → paramètres : `status.json` (raccourcis refusés), et `settings.json` quand une sortie a changé d'identifiant. La fenêtre relit les deux chaque seconde.

## Processus d'arrière-plan

Tout l'état est dans `App` (`crates/app/src/app.rs`), sur le thread principal, qui possède une fenêtre de premier niveau cachée. Cette fenêtre reçoit :

| Message | Origine | Effet |
|:--|:--|:--|
| `WM_VOLUME_NOTCHES` | thread du hook | Change le volume, montre l'affichage |
| `WM_HOTKEY` | `RegisterHotKey` | Change la sortie par défaut |
| `WM_TRAY`, `TaskbarCreated` | Explorer | Menu, paramètres, icône recréée |
| `WM_POWERBROADCAST` (reprise), `WM_WTSSESSION_CHANGE` (déverrouillage) | Windows | Hook réinstallé |
| `WM_TIMER` | minuteurs | Fondu de l'affichage, nouvel essai des raccourcis refusés et de l'icône |

Le hook souris tourne sur son propre thread (`hook.rs`), voir [ADR 0003](adr/0003-win-wheel-hook.md).

Les hooks bas niveau ont besoin d'une boucle de messages sur leur thread. `thread.rs` fournit ce thread (`MessageThread`) : il vit jusqu'à ce que son propriétaire le libère, et ne se termine qu'après `WM_QUIT`, pour qu'aucun message posté plus tard n'arrive à un autre thread qui aurait repris son identifiant.

### Chemin d'un cran de molette

1. Le hook voit `WM_MOUSEWHEEL` avec Win seul enfoncé.
2. Il cumule le delta (`WheelAccumulator`) et envoie la touche `vkE8` pour que Démarrer ne s'ouvre pas.
3. Il bloque l'événement et poste le nombre de crans entiers à la fenêtre principale.
4. `App` demande la sortie multimédia par défaut, calcule le nouveau niveau (`stepped_level`), l'applique, retire la sourdine si le volume monte.
5. L'affichage dessine le niveau (`core::osd::render`) et le montre sur l'écran du curseur.

### Chemin d'un raccourci

1. `WM_HOTKEY` donne l'index de la sortie dans les paramètres.
2. `resolve` retrouve la sortie parmi celles de Windows, voir [ADR 0004](adr/0004-identite-des-sorties.md).
3. `IPolicyConfig::SetDefaultEndpoint` la rend sortie par défaut, voir [ADR 0002](adr/0002-ipolicyconfig.md).
4. La sortie par défaut est relue pour vérifier le changement, puis affichée.

## Capture d'un raccourci

Dans la fenêtre de paramètres, « Choisir… » lance `keys::Capture`, qui pose un hook clavier bas niveau sur un `MessageThread`. Tant que la capture dure, le hook avale toutes les touches : ni le shell ni le processus d'arrière-plan ne les voient, donc Win+E n'ouvre pas l'Explorateur et les raccourcis existants ne se déclenchent pas. N'importe quelle touche, combinaison avec Win comprise, peut ainsi être choisie.

- La combinaison est complète au premier appui sur une touche qui n'est pas un modificateur. Ctrl, Alt, Maj et Win sont suivis côté gauche et côté droit (`HeldModifiers`), pour qu'en relâcher un ne fasse pas oublier l'autre.
- Échap seul annule. La capture s'arrête aussi d'elle-même au bout de 15 s, ou dès qu'une autre application prend le focus : le hook est global, et une touche tapée ailleurs ne doit pas être volée.
- Les touches injectées sont acceptées, car les logiciels de macros (G Hub, Synapse, AutoHotkey) passent par là. Seule `vkE8`, envoyée par le hook de la molette, est ignorée.
- `Hotkey::validate` refuse une combinaison sans touche principale, et F12, réservée par Windows. `Hotkey::warnings` signale celles qui restent possibles mais gênantes : Win, ou une combinaison sans Ctrl, Alt ni Win, dont la touche ne marchera plus dans les autres applications (F13 à F24 exceptées).

## Robustesse

- **Audio** : seul l'énumérateur Core Audio est gardé. Chaque action redemande la sortie par défaut, et une erreur recrée l'énumérateur avant un second essai. Un service audio lent au démarrage ou redémarré ne bloque donc rien.
- **Icône** : réessayée toutes les 2 s si Explorer n'est pas prêt, recréée sur `TaskbarCreated`.
- **Raccourcis** : un raccourci refusé reste dans les paramètres et est réessayé toutes les 30 s.
- **Paramètres** : écriture atomique, fichier illisible gardé de côté, jamais réécrit au démarrage.
- **Journal** : `%LOCALAPPDATA%\SoundDock\logs\sounddock.log`, avec les paniques.

## Affichage

`crates/core/src/osd.rs` dessine dans une image RGBA prémultipliée (tiny-skia pour les formes, ab_glyph pour le texte, polices Segoe UI de Windows). `crates/app/src/osd.rs` la copie dans une fenêtre `WS_EX_LAYERED` posée par `UpdateLayeredWindow`. La fenêtre ne prend jamais le focus (`WS_EX_NOACTIVATE`), laisse passer les clics (`WS_EX_TRANSPARENT`) et n'apparaît pas dans Alt+Tab (`WS_EX_TOOLWINDOW`). Elle ne passe pas au-dessus d'un jeu en plein écran exclusif.
