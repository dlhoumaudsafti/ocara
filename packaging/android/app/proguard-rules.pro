# OcaraBridge.nativeStartServer est résolu par le pont JNI natif (le .so
# compilé depuis runtime_android_jni) via son nom Java EXACT, mangled à la
# compilation du .so :
# Java_com_ocara_bridge_OcaraBridge_nativeStartServer (voir
# runtime_android_jni/src/lib.rs). R8 ne doit ni renommer ni supprimer cette
# classe/méthode : le nom recherché par la JVM au premier appel natif est figé
# côté .so, pas recalculé — un renommage produirait un UnsatisfiedLinkError
# silencieux uniquement en production (jamais en debug, où le minify est
# désactivé), le pire moment pour le découvrir.
-keep class com.ocara.bridge.OcaraBridge {
    native <methods>;
}
-keepclassmembers class com.ocara.bridge.OcaraBridge {
    native <methods>;
}
