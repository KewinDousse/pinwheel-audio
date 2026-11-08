# Pinwheel

Utilitaire Windows 10/11 qui tourne dans la zone de notification :

- **Win + molette** règle le volume de la sortie audio par défaut, par pas de 10 % (réglable).
- **Un raccourci clavier par sortie audio** en fait la sortie par défaut.
- **Un affichage à l'écran** montre le volume ou la sortie choisie pendant 1 s, puis disparaît en fondu. Trois styles : barre fine, pilule, grand chiffre.

## Installation

1. Copier `pinwheel.exe` dans un dossier local, par exemple `%LOCALAPPDATA%\Programs\Pinwheel\`. Un exécutable lancé depuis un partage réseau est souvent bloqué par Windows.
2. Le lancer. Au premier lancement, la fenêtre de paramètres s'ouvre.
3. Cocher « Lancer Pinwheel à l'ouverture de session » pour le démarrage automatique.

Quitter Volume2 et AudioSwitcher, et les retirer du démarrage : avec Volume2 actif, Win + molette changerait le volume deux fois.

## Utilisation

- Clic sur l'icône : paramètres. Clic droit : menu (Paramètres, Quitter).
- Relancer `pinwheel.exe` alors qu'il tourne ouvre aussi les paramètres.
- Les changements sont enregistrés et appliqués tout de suite.

## Fichiers

| Fichier | Contenu |
|:--|:--|
| `%APPDATA%\Pinwheel\settings.json` | Paramètres |
| `%LOCALAPPDATA%\Pinwheel\logs\pinwheel.log` | Journal (bouton « Ouvrir le dossier des journaux ») |
| `%LOCALAPPDATA%\Pinwheel\status.json` | Raccourcis refusés par Windows, lu par la fenêtre de paramètres |

Un fichier de paramètres illisible est renommé en `settings.json.<horodatage>.bak` au lieu d'être écrasé.

## Développement

Rust (édition 2024). Le dépôt contient :

- `crates/core` : logique indépendante de Windows (paramètres, raccourcis, pas du volume, molette, rapprochement des sorties, rendu de l'affichage). Se teste sur n'importe quel OS.
- `crates/app` : l'application Windows.
- `tools/make-icon` : génère `crates/app/res/pinwheel.ico`.

Voir [`docs/architecture.md`](docs/architecture.md) et les décisions dans [`docs/adr/`](docs/adr/).

### Tests

```sh
cargo test                     # crates/core
cargo run -p pinwheel-core --example osd-preview -- apercu.png <police.ttf> [police-grasse.ttf]
```

L'exemple `osd-preview` dessine tous les styles d'affichage dans une image. Les essais sur Windows suivent [`docs/tests-manuels.md`](docs/tests-manuels.md).

### Compiler l'exécutable Windows

Sous Windows : `cargo build -p pinwheel --release`.

Depuis Linux, avec [`cargo-xwin`](https://github.com/rust-cross/cargo-xwin) (qui télécharge le CRT MSVC et le SDK Windows, et accepte leur licence) et LLVM (`clang-cl`, `lld-link`, `llvm-rc`) :

```sh
rustup target add x86_64-pc-windows-msvc
cargo install cargo-xwin --locked
scripts/build-windows.sh       # → target/x86_64-pc-windows-msvc/release/pinwheel.exe
```

L'icône, les informations de version et le manifeste (DPI par écran, sans élévation) sont intégrés à l'exécutable par `crates/app/build.rs`.
