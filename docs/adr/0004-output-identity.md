# ADR 0004 : identité des sorties et conservation des paramètres

## Décision

Chaque sortie choisie est enregistrée avec son identifiant d'endpoint Windows et son nom (`PKEY_Device_FriendlyName`, par exemple « Casque (Realtek USB Audio) »). À l'appui sur son raccourci, la sortie est cherchée parmi les sorties actives, débranchées ou désactivées :

1. Même identifiant et active : c'est elle.
2. Sinon, si **une seule** sortie active porte le même nom : c'est elle. Son nouvel identifiant et son nom remplacent les anciens dans les paramètres, et le changement est écrit dans le journal.
3. Sinon (débranchée, introuvable, ou plusieurs sorties du même nom) : rien ne change, l'affichage indique « déconnectée ».

Une sortie absente reste dans les paramètres avec son raccourci jusqu'à ce que l'utilisateur la retire.

Le fichier de paramètres (`%APPDATA%\Pinwheel\settings.json`, JSON avec un numéro de version) :

- n'est écrit qu'après une action de l'utilisateur : un changement dans la fenêtre de paramètres, ou l'appui sur un raccourci qui déclenche le cas 2 ;
- est écrit de façon atomique (fichier temporaire puis renommage) ;
- s'il est illisible, il est renommé en `settings.json.<horodatage>.bak` avant que les valeurs par défaut soient utilisées ;
- tolère les champs absents (valeurs par défaut) et ignore les champs inconnus.

Un raccourci que Windows refuse reste enregistré : le refus est affiché dans les paramètres et l'enregistrement est retenté toutes les 30 s.

## Raisons

- L'identifiant d'endpoint survit aux redémarrages et aux débranchements, mais Microsoft précise qu'il change si le pilote est mis à jour ou le périphérique réinstallé ([documentation](https://learn.microsoft.com/en-us/windows/win32/coreaudio/endpoint-id-strings)). Un nouvel appairage Bluetooth en donne aussi un nouveau. Le nom permet de retrouver la sortie dans ces cas.
- Deux sorties du même nom ne permettent pas de choisir sans risque d'erreur.
- Ne jamais réécrire au démarrage empêche qu'une sortie absente à ce moment disparaisse des paramètres.
