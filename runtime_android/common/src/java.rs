//! Appels des méthodes statiques des classes Kotlin `com.ocara.bridge.*`
//! depuis un thread Ocara, attaché à la JVM au premier appel.

use std::sync::Mutex;

use jni::objects::{GlobalRef, JClass, JObject, JObjectArray, JString, JThrowable, JValue, JValueOwned};
use jni::sys::jobject;
use jni::{JNIEnv, JavaVM};

use crate::{Failure, ERR_ANDROID, ERR_PERMISSION, ERR_UNAVAILABLE};

unsafe extern "C" {
    fn ocara_android_vm() -> *mut jni::sys::JavaVM;
    fn ocara_android_bridge_class() -> jobject;
}

pub enum Arg<'a> {
    Str(&'a str),
    Int(i64),
    Float(f64),
}

#[derive(Clone, Copy)]
pub enum Ret {
    Void,
    Int,
    Bool,
    Str,
    Strs,
}

pub enum Value {
    Void,
    Int(i64),
    Bool(bool),
    Str(String),
    Strs(Vec<String>),
}

impl Value {
    pub fn int(self) -> i64 {
        if let Value::Int(n) = self { n } else { 0 }
    }

    pub fn bool(self) -> i64 {
        matches!(self, Value::Bool(true)) as i64
    }

    /// Chaîne Ocara possédée.
    pub fn string(self) -> i64 {
        if let Value::Str(s) = self { crate::ffi::string(&s) } else { crate::ffi::string("") }
    }

    /// `array<string>` Ocara.
    pub fn strings(self) -> i64 {
        if let Value::Strs(v) = self { crate::ffi::string_array(&v) } else { crate::ffi::string_array(&[]) }
    }
}

/// Exception Kotlin portant un code `AndroidException` (`OcaraAndroidError.kt`).
const ERROR_CLASS: &str = "com.ocara.bridge.OcaraAndroidError";

/// Classes déjà chargées (référence globale), par nom.
static CLASSES: Mutex<Vec<(&'static str, GlobalRef)>> = Mutex::new(Vec::new());

impl From<jni::errors::Error> for Failure {
    fn from(error: jni::errors::Error) -> Self {
        Failure::new(ERR_ANDROID, format!("JNI : {error}"))
    }
}

/// Appelle `class.method(args)` et lève l'`AndroidException` d'un échec,
/// au nom du builtin `source`.
pub fn invoke(source: &'static str, class: &'static str, method: &str, args: &[Arg], ret: Ret) -> Value {
    call(class, method, args, ret).unwrap_or_else(|failure| failure.raise(source))
}

pub fn call(class: &'static str, method: &str, args: &[Arg], ret: Ret) -> Result<Value, Failure> {
    let vm = vm()?;
    let mut env = vm.attach_current_thread_permanently()?;
    env.with_local_frame(16, |env| {
        let class = load_class(env, class)?;
        let strings = args.iter()
            .map(|arg| match arg {
                Arg::Str(s) => env.new_string(s).map(JObject::from),
                _ => Ok(JObject::null()),
            })
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| pending(env, e))?;
        let values: Vec<JValue> = args.iter().zip(&strings)
            .map(|(arg, string)| match arg {
                Arg::Str(_) => JValue::Object(string),
                Arg::Int(n) => JValue::Long(*n),
                Arg::Float(f) => JValue::Double(*f),
            })
            .collect();
        let result = env.call_static_method(&class, method, signature(args, ret), &values)
            .map_err(|e| pending(env, e))?;
        convert(env, result, ret)
    })
}

fn vm() -> Result<JavaVM, Failure> {
    let ptr = unsafe { ocara_android_vm() };
    if ptr.is_null() {
        return Err(unavailable());
    }
    Ok(unsafe { JavaVM::from_raw(ptr) }?)
}

fn unavailable() -> Failure {
    Failure::new(ERR_UNAVAILABLE, "pont JNI non initialisé : l'app doit démarrer le programme par OcaraBridge.nativeStartServer")
}

fn signature(args: &[Arg], ret: Ret) -> String {
    let params: String = args.iter()
        .map(|arg| match arg {
            Arg::Str(_) => "Ljava/lang/String;",
            Arg::Int(_) => "J",
            Arg::Float(_) => "D",
        })
        .collect();
    let ret = match ret {
        Ret::Void => "V",
        Ret::Int => "J",
        Ret::Bool => "Z",
        Ret::Str => "Ljava/lang/String;",
        Ret::Strs => "[Ljava/lang/String;",
    };
    format!("({params}){ret}")
}

fn convert(env: &mut JNIEnv, value: JValueOwned, ret: Ret) -> Result<Value, Failure> {
    Ok(match ret {
        Ret::Void => Value::Void,
        Ret::Int => Value::Int(value.j()?),
        Ret::Bool => Value::Bool(value.z()?),
        Ret::Str => Value::Str(java_string(env, value.l()?)?),
        Ret::Strs => {
            let array = JObjectArray::from(value.l()?);
            let len = if array.is_null() { 0 } else { env.get_array_length(&array)? };
            let mut out = Vec::with_capacity(len as usize);
            for i in 0..len {
                let item = env.get_object_array_element(&array, i)?;
                out.push(java_string(env, item)?);
            }
            Value::Strs(out)
        }
    })
}

fn java_string(env: &mut JNIEnv, object: JObject) -> Result<String, Failure> {
    if object.is_null() {
        return Ok(String::new());
    }
    Ok(env.get_string(&JString::from(object))?.into())
}

/// Classe d'app chargée par le chargeur d'`OcaraBridge` : `FindClass` depuis
/// un thread natif ne voit que les classes du système.
fn load_class<'local>(env: &mut JNIEnv<'local>, name: &'static str) -> Result<JClass<'local>, Failure> {
    let mut cache = CLASSES.lock().unwrap_or_else(|e| e.into_inner());
    if let Some((_, class)) = cache.iter().find(|(n, _)| *n == name) {
        return Ok(unsafe { JClass::from_raw(class.as_obj().as_raw()) });
    }
    let bridge = unsafe { ocara_android_bridge_class() };
    if bridge.is_null() {
        return Err(unavailable());
    }
    let bridge = unsafe { JObject::from_raw(bridge) };
    let loader = env.call_method(&bridge, "getClassLoader", "()Ljava/lang/ClassLoader;", &[])
        .and_then(JValueOwned::l)
        .map_err(|e| pending(env, e))?;
    let java_name = env.new_string(name).map_err(|e| pending(env, e))?;
    let class = env.call_method(&loader, "loadClass", "(Ljava/lang/String;)Ljava/lang/Class;", &[JValue::Object(&java_name)])
        .and_then(JValueOwned::l)
        .map_err(|e| pending(env, e))?;
    let global = env.new_global_ref(&class)?;
    let raw = global.as_obj().as_raw();
    cache.push((name, global));
    Ok(unsafe { JClass::from_raw(raw) })
}

/// Erreur d'un appel JNI : l'exception Java en attente devient l'échec.
fn pending(env: &mut JNIEnv, error: jni::errors::Error) -> Failure {
    if !matches!(error, jni::errors::Error::JavaException) {
        return error.into();
    }
    let Ok(throwable) = env.exception_occurred() else { return error.into() };
    let _ = env.exception_clear();
    let failure = describe(env, &throwable);
    let _ = env.exception_clear();
    failure.unwrap_or_else(|| Failure::new(ERR_ANDROID, "exception Java"))
}

fn describe(env: &mut JNIEnv, throwable: &JThrowable) -> Option<Failure> {
    let class = env.call_method(throwable, "getClass", "()Ljava/lang/Class;", &[]).ok()?.l().ok()?;
    let class_name = env.call_method(&class, "getName", "()Ljava/lang/String;", &[]).ok()?.l().ok()?;
    let ours = java_string(env, class_name).ok()? == ERROR_CLASS;
    let code = if ours {
        env.call_method(throwable, "getCode", "()I", &[]).ok()?.i().ok()? as i64
    } else if env.is_instance_of(throwable, "java/lang/SecurityException").unwrap_or(false) {
        ERR_PERMISSION
    } else {
        ERR_ANDROID
    };
    let text_method = if ours { "getMessage" } else { "toString" };
    let text = env.call_method(throwable, text_method, "()Ljava/lang/String;", &[]).ok()?.l().ok()?;
    Some(Failure::new(code, java_string(env, text).ok()?))
}
