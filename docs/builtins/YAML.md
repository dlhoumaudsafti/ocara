# ocara.YAML

Classe builtin pour sérialiser et désérialiser des données au format YAML.

## Import

```ocara
import ocara.YAML
```

## Encodage de données

### `YAML::encode(data: mixed) → string`

Encode un array ou une map en YAML.

**Paramètres** :
- `data` : données à encoder (array ou map)

**Retour** : string YAML

```ocara
const user:map<string, mixed> = {}
user["name"] = "Alice"
user["age"] = 30
user["active"] = true

const yaml:string = YAML::encode(user)
IO::write(yaml)
```

**Sortie** :
```yaml
name: Alice
age: 30
active: true
```

**Avec un array** :
```ocara
const colors:array<string> = []
colors[0] = "red"
colors[1] = "green"
colors[2] = "blue"

const yaml:string = YAML::encode(colors)
IO::write(yaml)
```

**Sortie** :
```yaml
- red
- green
- blue
```

## Décodage de YAML

### `YAML::decode(yaml: string) → mixed`

Décode une string YAML en structure Ocara (array ou map).

**Paramètres** :
- `yaml` : string YAML à décoder

**Retour** : `mixed` (sera un array ou une map selon le YAML)

```ocara
const yamlStr:string = "name: Bob\nage: 25\ncity: Paris\n"

const data:map<string, mixed> = YAML::decode(yamlStr)
IO::writeln(`Nom: ${data["name"]}`)      // Bob
IO::writeln(`Age: ${data["age"]}`)       // 25
IO::writeln(`Ville: ${data["city"]}`)    // Paris
```

**Décoder un array** :
```ocara
const yamlArray:string = "- apple\n- banana\n- orange\n"

const fruits:array<string> = YAML::decode(yamlArray)
for fruit in fruits {
    IO::writeln(fruit)
}
```

### `YAML::parse(yaml: string) → mixed`

Alias de `YAML::decode()`. Comportement identique.

```ocara
const data:mixed = YAML::parse(yamlStr)  // Identique à decode()
```

## Types supportés

| Type Ocara | Type YAML | Notes |
|------------|-----------|-------|
| `int` | number | Entiers |
| `float` | number | `decode()`/`parse()` reconstruisent un vrai flottant depuis un nombre YAML à virgule. Pour `encode()` : un flottant lu depuis une source qui le transporte "boxé" (résultat de requête SQL, valeur déjà décodée d'un YAML/JSON...) s'encode comme un nombre YAML ; un littéral flottant écrit directement dans un `array<mixed>`/`map<string, mixed>` (ex. `{'pi': 3.14159}`) s'encode comme une **chaîne** (`'3.14159'`), car ces littéraux sont stockés sous forme de string dans un conteneur `mixed` (limitation du langage, pas de YAML — voir `docs/roadmap.d/langage-mixed-literal-stringification.md`) |
| `string` | string | Chaînes de caractères |
| `bool` | boolean | `true` / `false` |
| `null` | null | Valeur nulle |
| `array<T>` | sequence | Listes YAML (`- item`) |
| `map<string, mixed>` | mapping | Objets YAML (`key: value`) |

**Types YAML non supportés** : les types "tagged" (`!!something`) sont décodés en `null`.

## Exemple complet

```ocara
import ocara.YAML
import ocara.IO

main {
    // Créer une structure de données
    const config:map<string, mixed> = {}
    config["app_name"] = "MyApp"
    config["version"] = 1
    config["debug"] = true
    
    const servers:array<string> = []
    servers[0] = "server1.example.com"
    servers[1] = "server2.example.com"
    config["servers"] = servers
    
    // Encoder en YAML
    const yaml:string = YAML::encode(config)
    IO::writeln("Configuration YAML:")
    IO::write(yaml)
    IO::writeln("")
    
    // Décoder le YAML
    const loaded:map<string, mixed> = YAML::decode(yaml)
    IO::writeln(`Application: ${loaded["app_name"]}`)
    IO::writeln(`Version: ${loaded["version"]}`)
    IO::writeln(`Debug: ${loaded["debug"]}`)
    
    const loadedServers:array<string> = loaded["servers"]
    IO::writeln("Serveurs:")
    for server in loadedServers {
        IO::writeln(`  - ${server}`)
    }
}
```

**Sortie** :
```
Configuration YAML:
app_name: MyApp
version: 1
debug: true
servers:
- server1.example.com
- server2.example.com

Application: MyApp
Version: 1
Debug: true
Serveurs:
  - server1.example.com
  - server2.example.com
```

## Round-trip (encode/decode)

```ocara
// Données originales
const original:map<string, mixed> = {}
original["name"] = "Alice"
original["score"] = 100

// Encoder
const yaml:string = YAML::encode(original)

// Décoder
const restored:map<string, mixed> = YAML::decode(yaml)

// restored contient les mêmes données que original
IO::writeln(`${restored["name"]}: ${restored["score"]}`)  // Alice: 100
```

## Cas d'usage

### Configuration d'application

```ocara
import ocara.YAML
import ocara.File

// Charger un fichier de configuration
const yamlContent:string = File::read("config.yaml")
const config:map<string, mixed> = YAML::decode(yamlContent)

const dbHost:string = config["database"]["host"]
const dbPort:int = config["database"]["port"]
```

### Sérialisation de données

```ocara
// Sauvegarder des données en YAML
const users:array<map<string, mixed>> = getUsersFromDatabase()
const yaml:string = YAML::encode(users)
File::write("users.yaml", yaml)
```

### Échange de données

```ocara
// Envoyer des données en YAML via HTTP
const payload:map<string, mixed> = {}
payload["action"] = "create_user"
payload["username"] = "alice"

const yaml:string = YAML::encode(payload)
HTTPRequest::post("https://api.example.com/users", yaml)
```

## YAML vs JSON

| Aspect | YAML | JSON |
|--------|------|------|
| Lisibilité | Excellente (indentation, pas de guillemets) | Bonne |
| Commentaires | ✅ Supportés (`#`) | ❌ Non supportés |
| Taille | Plus compact | Plus verbeux |
| Performance | Parsing légèrement plus lent | Parsing très rapide |
| Cas d'usage | Configuration, CI/CD, documentation | APIs, échange de données |

## Notes

- YAML est plus lisible que JSON pour les humains
- YAML supporte les commentaires (ligne commençant par `#`)
- Les fichiers YAML utilisent l'extension `.yaml` ou `.yml`
- La sérialisation YAML préserve la structure mais pas l'ordre des clés de map
- Pour les APIs REST, préférez JSON (plus rapide)
- Pour les fichiers de configuration, YAML est recommandé

## Erreurs

En cas d'erreur de parsing YAML, `decode()` et `parse()` retournent `null` (0) — ces fonctions ne lèvent jamais d'exception, il n'existe pas de `YAMLException` déclenchée en pratique aujourd'hui. Toujours vérifier le retour :

```ocara
const invalidYaml:string = "{ invalid : yaml : : }"
const result:mixed = YAML::decode(invalidYaml)

if result equal null {
    IO::writeln("Erreur: YAML invalide")
}
```
