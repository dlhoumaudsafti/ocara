# CI Ocara

Pipeline d'intégration continue pour le projet Ocara.

## Commande

```bash
make ci
```

## Pipeline

Le pipeline CI traite **chaque fichier individuellement** avec 4 étapes séquentielles :

### Pour chaque fichier :

#### Étape 1 : Linter (ocaracs)
- Vérifie le style et la qualité du code
- Détecte les problèmes de formatage, nommage, etc.
- Warnings n'empêchent pas la suite

#### Étape 2 : Vérification sémantique (--check)
- Vérifie la validité sémantique (types, symboles)
- Détecte les erreurs avant compilation
- Cas spécial : `21_errors.oc` doit échouer

#### Étape 3 : Dump IR (--dump)
- Génère le dump de l'IR (représentation intermédiaire)
- Sauvegarde dans `ci/dumps/<fichier>.dump`
- Permet l'analyse du code généré

#### Étape 4 : Tests unitaires + Couverture (ocaraunit --coverage)
- Lance les tests unitaires associés au fichier
- Vérifie la couverture de code
- Affiche les fonctions non couvertes

## Ordre d'exécution

```
Pour chaque fichier .oc :
  ┌─────────────────────────────────────┐
  │ 1. Linter (ocaracs)                 │
  │    ↓                                 │
  │ 2. Check sémantique (--check)       │
  │    ↓                                 │
  │ 3. Dump IR (--dump)                 │
  │    ↓                                 │
  │ 4. Tests + Couverture (ocaraunit)   │
  └─────────────────────────────────────┘
```

## Rapport

À la fin de l'exécution, un rapport complet est généré dans `ci/ci-report.txt` avec :
- Résultats détaillés de chaque fichier
- Statut de chaque étape (Lint → Check → Dump → Coverage)
- Statistiques globales
- Liste des échecs

## Sorties

- **Dumps IR** : `ci/dumps/*.dump`
- **Rapport** : `ci/ci-report.txt`
- **Exit code** : 
  - `0` = succès (tous les fichiers OK)
  - `1` = échec (au moins un fichier échoué)

## Exemple de sortie

```
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  Fichier : 01_variables
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  [1/4] Linter (ocaracs)...
        ✓ Style OK
  [2/4] Vérification sémantique (--check)...
        ✓ Sémantique OK
  [3/4] Dump IR (--dump)...
        ✓ Dump sauvegardé → ci/dumps/01_variables.dump
  [4/4] Tests unitaires + couverture (ocaraunit --coverage)...
        ✓ Tests passés
        Coverage: 100% (12/12 functions covered)
  ✓ SUCCÈS
```

## Avantages de cette approche

### 1. **Ordre logique**
Le pipeline suit l'ordre naturel du développement :
- Style → Sémantique → Code généré → Tests

### 2. **Détection précoce**
Les erreurs sont détectées tôt (lint et check) avant de générer le dump ou lancer les tests.

### 3. **Rapport par fichier**
Chaque fichier a son propre rapport complet, facilitant le debugging.

### 4. **Couverture obligatoire**
La couverture de code est vérifiée pour chaque fichier, garantissant la qualité.

### 5. **Dumps systématiques**
Chaque fichier valide génère son dump IR, utile pour analyser le code généré.

## Usage en développement

Lors de l'ajout d'une nouvelle fonctionnalité :

1. **Créer un exemple** : `examples/NN_feature.oc`
2. **Créer les tests** : `examples/tests/FeatureTest.oc` ou `examples/project/tests/FeatureTest.oc`
3. **Lancer le CI** : `make ci`
4. **Vérifier le rapport** :
   ```bash
   cat ci/ci-report.txt
   # Ou voir le dump IR :
   cat ci/dumps/NN_feature.dump
   ```

Le CI garantit que :
- Le style est propre (ocaracs)
- Le code compile sans erreur sémantique (--check)
- L'IR est généré correctement (--dump)
- La fonctionnalité est testée avec couverture complète (ocaraunit --coverage)
