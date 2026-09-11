# Support Windows non établi pour la compilation du compilateur

Aucune preuve que le compilateur `ocara` lui-même ait jamais été construit ou testé sur Windows : pas de script `.bat`/PowerShell, le `Makefile` utilise des symlinks Unix (`ln -sf`), `runtime/src/mutex.rs` s'appuie sur `pthread_mutex_t` (libc) sans fallback Windows (voir aussi [memoire-fiabilite-runtime-bas-niveau](memoire-fiabilite-runtime-bas-niveau.md)). `WebView2` n'est cité dans le `README.md` que comme runtime pour exécuter un programme Tauri déjà compilé sur Linux/macOS — pas comme cible de build du compilateur.

## Ampleur

Gros chantier si le support Windows natif devient un objectif : remplacement des primitives POSIX (`Mutex`), script de build dédié, validation de toute la chaîne SDL/Tauri sur cette plateforme.

## Contrainte

Nous somme sur un environnement Linux. La seul solution dans l'absolut serais de compiler sous wine
