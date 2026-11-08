# ADR 0001 : Rust, windows-rs et egui

## Décision

L'application est écrite en Rust, avec `windows-rs` pour les API Windows, `egui`/`eframe` pour la fenêtre de paramètres, `tiny-skia` et `ab_glyph` pour dessiner l'affichage. Elle est livrée en un seul exécutable natif, sans runtime à installer, compilable depuis Linux (`cargo-xwin`) comme depuis Windows.

## Raisons

- Le logiciel tourne en permanence et doit démarrer de façon fiable à l'ouverture de session. Un exécutable natif démarre instantanément, ne dépend d'aucun runtime (CRT lié en statique, voir `.cargo/config.toml`) et n'extrait rien dans `%TEMP%`.
- Les parties délicates (hook souris, `RegisterHotKey`, Core Audio, `IPolicyConfig`) sont des appels Win32 et COM directs, aussi simples en Rust qu'ailleurs.
- La logique et le rendu de l'affichage, séparés dans `crates/core`, se testent sur n'importe quel OS.

## Conséquences

- La fenêtre de paramètres n'a pas l'aspect natif de Windows (egui).
- L'exécutable pèse environ 14 Mo, surtout à cause du moteur de rendu d'egui (wgpu). Le processus d'arrière-plan ne l'utilise pas.
- L'affichage est dessiné à la main plutôt que décrit dans un langage d'interface.
- La fenêtre de paramètres tourne dans un processus séparé, car egui (via winit) veut posséder la boucle de messages du thread principal, déjà prise par le processus d'arrière-plan.
