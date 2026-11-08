# Tests manuels sous Windows

Ce que les tests automatiques ne couvrent pas. Le journal (`%LOCALAPPDATA%\Pinwheel\logs\pinwheel.log`) aide à comprendre un échec.

## Démarrage

- [ ] Premier lancement : l'icône apparaît, la fenêtre de paramètres s'ouvre.
- [ ] Relancer l'exécutable : un seul Pinwheel tourne, les paramètres s'ouvrent.
- [ ] Démarrage automatique coché, puis fermeture et réouverture de session : Pinwheel tourne.
- [ ] Désactiver Pinwheel dans Paramètres Windows > Applications > Démarrage : la fenêtre de paramètres l'indique.
- [ ] Redémarrer Explorer (Gestionnaire des tâches) : l'icône revient.

## Win + molette

- [ ] Dans l'Explorateur et dans un navigateur : le volume change par pas de 10 %, la page ne défile pas.
- [ ] Relâcher Win après avoir tourné la molette : le menu Démarrer ne s'ouvre pas.
- [ ] Win seul, sans molette : le menu Démarrer s'ouvre normalement.
- [ ] Ctrl + Win + molette : Pinwheel ne réagit pas.
- [ ] Volume à 33 % (réglé dans Windows) puis un cran vers le haut : 40 %.
- [ ] Sourdine active puis un cran vers le haut : la sourdine est retirée.
- [ ] Touchpad (scroll à deux doigts) et molette libre, si disponibles.
- [ ] Gestionnaire des tâches (fenêtre admin) au premier plan : noter le résultat.
- [ ] Après une mise en veille et un réveil, puis après un verrouillage et un déverrouillage : ça fonctionne encore.
- [ ] Après `Restart-Service audiosrv -Force` (PowerShell admin) : ça fonctionne encore.

## Affichage

- [ ] Chaque style et chaque position, avec le bouton « Aperçu ».
- [ ] L'affichage reste 1 s puis disparaît en fondu, et un nouveau cran relance le délai.
- [ ] Il ne prend pas le focus : on peut continuer à taper dans la fenêtre active.
- [ ] Deux écrans avec des mises à l'échelle différentes : il s'affiche net et à la bonne taille sur l'écran du curseur.

## Raccourcis des sorties

- [ ] Ajouter deux sorties avec Ctrl+Alt+1 et Ctrl+Alt+2 : chaque raccourci change la sortie et l'affiche.
- [ ] Débrancher ou éteindre une sortie (casque Bluetooth) : elle reste dans la liste, marquée « déconnectée », et son raccourci affiche « déconnectée ».
- [ ] La rebrancher : le raccourci refonctionne.
- [ ] Même raccourci sur deux sorties : la fenêtre le signale.
- [ ] Raccourci déjà pris par une autre application : la fenêtre le signale dans la seconde.
- [ ] « Aussi pour les appels » : la sortie de communication change dans Paramètres Windows > Son.
- [ ] Redémarrer le PC : tous les raccourcis sont toujours là.

## Capture d'un raccourci

- [ ] Choisir Win+E : l'Explorateur ne s'ouvre pas, la fenêtre affiche « Win+E » et l'avertissement sur Win.
- [ ] Choisir une combinaison déjà attribuée à une autre sortie : elle ne change pas de sortie pendant la capture.
- [ ] Choisir Ctrl droit + Maj gauche + une lettre, en relâchant les modificateurs dans un ordre quelconque : la combinaison est complète.
- [ ] Choisir une touche seule (une lettre, puis F13) : l'avertissement apparaît pour la lettre, pas pour F13.
- [ ] Une touche multimédia (lecture/pause) et une touche envoyée par un logiciel de macros : toutes deux acceptées.
- [ ] Échap seul : la capture est annulée, l'ancien raccourci reste.
- [ ] Ne rien taper pendant 15 s : la capture s'arrête.
- [ ] Pendant la capture, cliquer dans une autre fenêtre puis taper : la capture s'arrête et les touches arrivent dans cette fenêtre.
- [ ] Fermer et rouvrir les paramètres : le raccourci capturé est enregistré et fonctionne.
