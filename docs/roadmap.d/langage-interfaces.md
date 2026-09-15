# Interfaces et polymorphisme

L'import transitif des interfaces, la vérification de signature E09, et le polymorphisme réel à l'exécution (identité d'objet, affectation classe/interface, dispatch dynamique, `is ClassName`/`is InterfaceName` réel) sont corrigés — voir git log pour le détail (chantier conséquent : header d'instance étendu, `types_compat`/`class_matches`, dispatchers générés par classe/interface, plus deux bugs pré-existants découverts et corrigés en cours de route : héritage de méthode sur 3+ niveaux, SEGFAULT `is` sur un entier concret).

## Reste à faire : `self.méthode()`/`parent.méthode()` ne sont jamais dispatchés dynamiquement

Un appel externe (`obj.méthode()` depuis en dehors du corps de la classe) bénéficie du dispatch dynamique réel. Un appel `self.méthode()` (ou `parent.méthode()`) depuis l'**intérieur** d'un corps de méthode reste résolu **statiquement**, vers l'implémentation concrète de la classe sous laquelle ce corps a été lowered — jamais redirigé vers une éventuelle surcharge du type réel de l'objet à l'exécution. C'est une différence de comportement avec le polymorphisme classique (le patron "template method", où une méthode de base appelle `self.hook()` en attendant qu'une sous-classe la substitue, ne fonctionnerait pas ici).

À faire : étendre le dispatch dynamique (`IrModule::classes_with_subclasses`/`class_dispatch::class_dispatcher_name`) aux appels `self`/`parent` quand la méthode appelée est effectivement surchargeable — actuellement exclu délibérément pour rester dans le périmètre initial de ce chantier.

## Fichiers clés

`src/lower/expr.d/lower.rs` (appel de méthode d'instance, `is_self_or_parent`), `src/lower/builder.d/class_dispatch.rs`, `src/lower/builder.d/interfaces.rs`.
