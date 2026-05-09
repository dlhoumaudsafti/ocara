# ocara.DotEnv

Classe builtin pour charger et gérer des variables d'environnement depuis des fichiers `.env`.

## Import

```ocara
import ocara.DotEnv
```

## Chargement de fichiers .env

### `DotEnv::load(env: string) → void`

Charge un fichier `.env` dans l'environnement de l'application.

**Paramètres** :
- `env` : extension du fichier à charger
  - Si vide ou `null` : charge `.env`
  - Si `"prod"` : charge `.env.prod`
  - Si `"dev"` : charge `.env.dev`
  - etc.

```ocara
// Charge .env
DotEnv::load("")

// Charge .env.prod
DotEnv::load("prod")

// Charge .env.dev
DotEnv::load("dev")
```

**Format du fichier .env** :
```bash
# Commentaire
KEY=value
APP_NAME=MyApp
DEBUG=true
PORT=3000

# Avec guillemets (les guillemets seront retirés)
API_KEY="secret-key-123"
MESSAGE='Hello, World!'

# Avec espaces (pas besoin de guillemets)
DESCRIPTION=This is my application
```

**Comportement** :
- Les lignes vides et les commentaires (`#`) sont ignorés
- Format : `KEY=VALUE`
- Les guillemets simples et doubles sont automatiquement retirés
- Les variables sont chargées dans l'environnement système ET dans le stockage interne DotEnv
- Si le fichier n'existe pas, un warning est affiché mais le programme continue

## Récupération de variables

### `DotEnv::get(key: string) → string|null`

Récupère la valeur d'une variable d'environnement.

**Retour** :
- La valeur si la variable existe
- `null` si la variable n'existe pas

```ocara
const appName:string = DotEnv::get("APP_NAME")

if appName != null {
    IO::writeln(`Application: ${appName}`)
} else {
    IO::writeln("APP_NAME non définie")
}
```

**Ordre de recherche** :
1. Variables chargées depuis les fichiers `.env` (via `DotEnv::load()`)
2. Variables d'environnement système

## Exemple complet

**Fichier `.env` :**
```bash
APP_NAME=OcaraApp
APP_VERSION=1.0.0
DEBUG=true
PORT=3000
DATABASE_URL=postgres://localhost/mydb
```

**Fichier `.env.prod` :**
```bash
APP_NAME=OcaraApp
APP_VERSION=1.0.0
DEBUG=false
PORT=8080
DATABASE_URL=postgres://prod.server.com/mydb
```

**Code Ocara :**
```ocara
import ocara.DotEnv
import ocara.IO
import ocara.System

// Déterminer l'environnement
const env:string = System::getEnv("ENVIRONMENT")

if env == "production" {
    IO::writeln("Mode production")
    DotEnv::load("prod")
} else {
    IO::writeln("Mode développement")
    DotEnv::load("")
}

// Récupérer les variables
const appName:string = DotEnv::get("APP_NAME")
const version:string = DotEnv::get("APP_VERSION")
const debug:string = DotEnv::get("DEBUG")
const port:string = DotEnv::get("PORT")
const dbUrl:string = DotEnv::get("DATABASE_URL")

IO::writeln(`Application: ${appName} v${version}`)
IO::writeln(`Debug: ${debug}`)
IO::writeln(`Port: ${port}`)
IO::writeln(`Database: ${dbUrl}`)
```

## Cas d'usage typiques

### Configuration par environnement

```ocara
// Charger la config selon l'environnement
const env:string = System::getEnv("ENV")

if env == "prod" {
    DotEnv::load("prod")
} else if env == "staging" {
    DotEnv::load("staging")
} else {
    DotEnv::load("dev")
}
```

### Connexion base de données

```ocara
import ocara.DotEnv
import ocara.MySQL

DotEnv::load("")

const host:string = DotEnv::get("DB_HOST")
const user:string = DotEnv::get("DB_USER")
const password:string = DotEnv::get("DB_PASSWORD")
const database:string = DotEnv::get("DB_NAME")

if host != null and user != null and password != null and database != null {
    const db:MySQL = MySQL::connect(host, user, password, database)
    // ...
} else {
    IO::writeln("Erreur: Variables de connexion manquantes")
}
```

### Secrets et API keys

```ocara
DotEnv::load("")

const apiKey:string = DotEnv::get("API_KEY")
const secretToken:string = DotEnv::get("SECRET_TOKEN")

if apiKey != null {
    // Utiliser l'API key
} else {
    IO::writeln("Warning: API_KEY non configurée")
}
```

## Bonnes pratiques

1. **Ne jamais commiter les fichiers `.env`** : ajoutez `.env*` à votre `.gitignore`
2. **Fournir un fichier exemple** : créez `.env.example` avec les clés mais sans valeurs sensibles
3. **Valider les variables requises** : vérifiez que les variables critiques sont définies
4. **Séparer par environnement** : utilisez `.env.dev`, `.env.prod`, `.env.test`
5. **Documentation** : documentez chaque variable dans un README ou `.env.example`

## Exemple .env.example

```bash
# Configuration de l'application
APP_NAME=MyApp
APP_VERSION=1.0.0
DEBUG=true

# Base de données
DB_HOST=localhost
DB_USER=
DB_PASSWORD=
DB_NAME=

# API externes
API_KEY=
SECRET_TOKEN=

# Serveur
PORT=3000
HOST=0.0.0.0
```

## Notes

- Les variables sont stockées globalement et persistent durant toute l'exécution
- Charger un nouveau fichier `.env` écrase les variables précédentes avec le même nom
- Les variables système existantes ne sont pas écrasées, mais DotEnv les surcharge dans `get()`
- Le parsing est simple : une ligne = une variable, pas de substitution ou d'expansion
- Les valeurs multi-lignes ne sont pas supportées
