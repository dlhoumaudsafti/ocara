/// Fonctions de recherche (lookup) dans la table des symboles
use super::types::{
    ClassInfo,
    EnumInfo,
    FieldInfo,
    FuncSig,
    GenericInfo,
    InterfaceInfo,
    ModuleInfo,
    SymbolTable,
};
use crate::parsing::ast::{Type, Visibility};

impl SymbolTable {
    /// Recherche une fonction par son nom
    pub fn lookup_function(&self, name: &str) -> Option<&FuncSig> {
        self.functions.get(name)
    }

    /// Recherche une classe par son nom
    pub fn lookup_class(&self, name: &str) -> Option<&ClassInfo> {
        self.classes.get(name)
    }

    /// Récupère le nom de la classe parent d'une classe
    pub fn lookup_parent_class(&self, name: &str) -> Option<String> {
        self.classes.get(name)?.extends.clone()
    }

    /// Recherche un module (mixin) par son nom
    #[allow(dead_code)]
    pub fn lookup_module(&self, name: &str) -> Option<&ModuleInfo> {
        self.modules.get(name)
    }

    /// Recherche un générique par son nom
    pub fn lookup_generic(&self, name: &str) -> Option<&GenericInfo> {
        self.generics.get(name)
    }

    /// Recherche une interface par son nom
    #[allow(dead_code)]
    pub fn lookup_interface(&self, name: &str) -> Option<&InterfaceInfo> {
        self.interfaces.get(name)
    }

    /// Recherche une constante par son nom
    pub fn lookup_const(&self, name: &str) -> Option<&Type> {
        self.consts.get(name)
    }

    /// Recherche un enum par son nom
    #[allow(dead_code)]
    pub fn lookup_enum(&self, name: &str) -> Option<&EnumInfo> {
        self.enums.get(name)
    }

    /// Recherche une constante de classe (ex: MyClass::CONST_NAME)
    pub fn lookup_class_const(&self, class: &str, name: &str) -> Option<&(Type, Visibility)> {
        self.classes.get(class)?.class_consts.get(name)
    }

    /// Résout un nom de classe builtin `ocara.*` CANONIQUE (ex: `"HTTPRequest"`)
    /// vers le nom LOCAL sous lequel il est réellement enregistré dans ce
    /// fichier — son alias d'import (`import ocara.HTTPRequest as Request`
    /// → `"Request"`), ou lui-même si importé sans alias / jamais importé.
    ///
    /// `register_import` enregistre une classe builtin sous UNE SEULE clé
    /// (l'alias s'il y en a un, sinon le nom canonique — jamais les deux à la
    /// fois, voir `register_import`). Tout code qui compare un nom de classe
    /// résolu contre un littéral canonique en dur (ex: `resolved_class ==
    /// "HTTPRequest"`) pour activer un comportement natif spécial (carve-out
    /// d'échappement de ressource, redirection `HTTPResponse` → `HTTPRequest`
    /// pour le sucre d'instance...) doit passer par cette résolution, sinon
    /// la comparaison échoue silencieusement dès que l'import est aliasé —
    /// bug reproduit avec `import ocara.HTTPRequest as Request` puis
    /// `Request::ok(res)` / `res.ok()`, qui rejetait `res` comme un
    /// échappement de ressource ou une méthode introuvable.
    pub fn local_name_for_builtin(&self, canonical: &str) -> String {
        for imp in &self.imports {
            let is_ocara = imp.path.first().map(|s| s == "ocara").unwrap_or(false);
            if is_ocara && imp.path.last().map(|s| s.as_str()) == Some(canonical) {
                return imp.alias.clone().unwrap_or_else(|| canonical.to_string());
            }
        }
        canonical.to_string()
    }

    /// Cherche un champ en remontant la chaîne d'héritage
    pub fn lookup_field_in_chain(&self, class_name: &str, field: &str) -> Option<&FieldInfo> {
        let mut current = class_name;
        loop {
            let info = self.classes.get(current)?;
            if let Some(f) = info.fields.get(field) {
                return Some(f);
            }
            match info.extends.as_deref() {
                Some(parent) => current = parent,
                None => return None,
            }
        }
    }

    /// Cherche une méthode en remontant la chaîne d'héritage
    pub fn lookup_method_in_chain(&self, class_name: &str, method: &str) -> Option<&FuncSig> {
        let mut current = class_name;
        loop {
            let info = self.classes.get(current)?;
            if let Some(m) = info.methods.get(method) {
                return Some(m);
            }
            match info.extends.as_deref() {
                Some(parent) => current = parent,
                None => return None,
            }
        }
    }

    /// Vrai si `class_name` "est un(e)" `target` — un `target` CLASSE
    /// (`class_name` lui-même ou l'un quelconque de ses ancêtres via
    /// `extends`), ou un `target` INTERFACE implémentée directement par
    /// `class_name` OU par l'un de ses ancêtres (l'implémentation d'une
    /// interface n'est PAS elle-même héritée dans `ClassInfo.implements` —
    /// remonter la chaîne `extends` ici est ce qui rend cette relation
    /// transitive : un enfant qui n'a pas d'`implements` propre "hérite"
    /// quand même de l'interface de son parent, cohérent avec le fait qu'il
    /// hérite aussi de l'implémentation de la méthode, voir
    /// `lower_class`/"Émettre les méthodes héritées" dans
    /// `src/lower/builder.d/classes.rs`).
    ///
    /// Utilisé pour l'affectation polymorphe réelle (`var s:Shape =
    /// use Circle()`, `var d:Drawable = use Circle()`) — voir
    /// `types_compat` et docs/roadmap.d/langage-interfaces.md. `target`
    /// n'a pas besoin d'exister (retourne simplement `false`, l'appelant a
    /// déjà vérifié son existence séparément si besoin).
    pub fn class_matches(&self, class_name: &str, target: &str) -> bool {
        let mut current = Some(class_name.to_string());
        while let Some(c) = current {
            if c == target {
                return true;
            }
            let Some(info) = self.classes.get(&c) else { break };
            if info.implements.iter().any(|i| i == target) {
                return true;
            }
            current = info.extends.clone();
        }
        false
    }

    /// Résoudre un type nommé → vérifie que la classe ou interface existe
    #[allow(dead_code)]
    pub fn type_exists(&self, name: &str) -> bool {
        self.classes.contains_key(name)
            || self.interfaces.contains_key(name)
            || matches!(
                name,
                "int" | "float" | "string" | "bool" | "mixed" | "void"
            )
    }
}
