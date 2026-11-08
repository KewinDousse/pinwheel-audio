# Tests manuels sous Windows

Ce que les tests automatiques ne couvrent pas. Le journal (`%LOCALAPPDATA%\SoundDock\logs\sounddock.log`) aide à comprendre un échec.

## Démarrage

- [ ] Premier lancement : l'icône apparaît, la fenêtre de paramètres s'ouvre.
- [ ] Relancer l'exécutable : un seul SoundDock tourne, les paramètres s'ouvrent.
- [ ] Démarrage automatique coché, puis fermeture et réouverture de session : SoundDock tourne.
- [ ] Désactiver SoundDock dans Paramètres Windows > Applications > Démarrage : la fenêtre de paramètres l'indique.
- [ ] Redémarrer Explorer (Gestionnaire des tâches) : l'icône revient.

## Win + molette

- [ ] Dans l'Explorateur et dans un navigateur : le volume change par pas de 10 %, la page ne défile pas.
- [ ] Relâcher Win après avoir tourné la molette : le menu Démarrer ne s'ouvre pas.
- [ ] Win seul, sans molette : le menu Démarrer s'ouvre normalement.
- [ ] Ctrl + Win + molette : SoundDock ne réagit pas.
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
