# ADR 0002 : changer la sortie par défaut avec IPolicyConfig

## Décision

La sortie par défaut est changée avec l'interface COM non documentée `IPolicyConfig` (IID `f8679f50-850a-41cf-9c72-430f290290c8`, classe `CPolicyConfigClient`, CLSID `870af99c-171d-4f9e-af0d-e63df40c2bc9`), méthode `SetDefaultEndpoint(id, rôle)`, onzième méthode après celles de `IUnknown`.

Un raccourci règle les rôles `eConsole` et `eMultimedia`, comme les paramètres Son de Windows. Le rôle `eCommunications` (applications d'appel) n'est réglé que si la sortie a l'option « Aussi pour les appels », désactivée par défaut.

Le volume réglé par la molette est celui de la sortie par défaut `eMultimedia`.

Après un changement, la sortie par défaut est relue : un écart est écrit dans le journal et signalé à l'écran.

## Raisons

- Windows n'offre aucune API publique pour changer la sortie par défaut. Les outils du même genre (AudioSwitcher, SoundSwitch, EarTrumpet) utilisent cette interface, stable depuis Windows 7.
- Régler les appels par défaut peut faire basculer un casque Bluetooth sur son profil « mains libres », mono et de moindre qualité.

## Conséquences

- Une mise à jour de Windows pourrait casser l'interface sans erreur visible : la relecture après changement le détecte.
- L'interface est isolée dans `crates/app/src/audio.rs` (module `policy`). Seule la position de `SetDefaultEndpoint` dans la vtable compte : les méthodes qui la précèdent sont déclarées sans paramètres et jamais appelées.
