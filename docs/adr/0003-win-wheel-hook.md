# ADR 0003 : Win + molette par un hook souris bas niveau

## Décision

Win + molette est capté par un hook `WH_MOUSE_LL`, installé sur un thread dédié qui a sa propre boucle de messages.

- **Déclenchement** : seulement avec Win gauche ou droite enfoncée, sans Ctrl, Alt ni Maj. La molette horizontale est ignorée.
- **Molette** : les deltas sont cumulés, et chaque tranche de 120 (un cran classique) donne un pas. Un changement de sens ou une pause de plus de 400 ms annule la fraction en cours. Les molettes haute résolution et les touchpads envoient des deltas plus petits.
- **Pas** : le niveau atterrit sur un multiple du pas (de 33 %, un pas de 10 mène à 40 ou 30). Monter le volume retire la sourdine.
- **Événement bloqué** : la fenêtre sous le curseur ne défile pas.
- **Menu Démarrer** : à chaque événement capté, la touche virtuelle non attribuée `0xE8` est envoyée (appui puis relâchement), comme le recommande la documentation d'AutoHotkey (`A_MenuMaskKey`). Windows considère alors que Win a servi dans une combinaison et n'ouvre pas Démarrer au relâchement.
- **Rôle du callback** : il ne fait que calculer les crans et poster `WM_VOLUME_NOTCHES` à la fenêtre principale. L'appel audio se fait sur le thread principal.
- **Réinstallation** : le hook est réinstallé sans condition toutes les 5 minutes, à la reprise après veille et au déverrouillage de session.

## Raisons

- Raw Input ne permet pas de bloquer l'événement. `RegisterHotKey` ne gère pas la molette.
- Windows retire sans prévenir un hook bas niveau dont le callback dépasse le délai `LowLevelHooksTimeout` (au plus 1 s depuis Windows 10 1709), et l'application ne peut pas savoir qu'il a été retiré ([documentation](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelmouseproc)). Un thread dédié, qui ne fait rien de lent, et une réinstallation périodique évitent qu'il disparaisse pour de bon.

## Conséquences

- Quand une fenêtre lancée en administrateur est au premier plan, Windows (UIPI) ne transmet pas la molette au hook d'un processus non élevé : Win + molette n'y fonctionne pas.
- Si le scroll à deux doigts d'un touchpad n'arrive pas en `WM_MOUSEWHEEL` dans le hook, il n'est pas pris en compte : à vérifier sur le matériel.
