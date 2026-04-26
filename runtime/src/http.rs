// ─────────────────────────────────────────────────────────────────────────────
// ocara.HTTPRequest — implémentation via ureq 2.x
//
// API Ocara :
//   HTTPRequest::new(url)                        → req:int
//   HTTPRequest::set_method(req, method)         → void
//   HTTPRequest::set_header(req, key, value)     → void
//   HTTPRequest::set_body(req, body)             → void
//   HTTPRequest::set_timeout(req, ms)            → void
//   HTTPRequest::send(req)                       → res:int
//   HTTPRequest::status(res)                     → int
//   HTTPRequest::body(res)                       → string
//   HTTPRequest::header(res, name)               → string
//   HTTPRequest::headers(res)                    → map<string,string>
//   HTTPRequest::ok(res)                         → bool  (2xx)
//   HTTPRequest::is_error(res)                   → bool
//   HTTPRequest::error(res)                      → string
//   HTTPRequest::get(url)                        → res:int
//   HTTPRequest::post(url, body)                 → res:int
//   HTTPRequest::put(url, body)                  → res:int
//   HTTPRequest::delete(url)                     → res:int
//   HTTPRequest::patch(url, body)                → res:int
// ─────────────────────────────────────────────────────────────────────────────

use crate::{alloc_str, ptr_to_str, new_map, __map_set};

// ─── Structures ──────────────────────────────────────────────────────────────

struct OcaraHttpRequest {
    url:     String,
    method:  String,
    headers: Vec<(String, String)>,
    body:    Option<String>,
    timeout: Option<u64>, // millisecondes
}

struct OcaraHttpResponse {
    status:  i64,
    body:    String,
    headers: Vec<(String, String)>,
    is_err:  bool,
    error:   String,
}

// ─── Accès aux structs ────────────────────────────────────────────────────────

fn req_ref(ptr: i64) -> &'static mut OcaraHttpRequest {
    unsafe { &mut *(ptr as *mut OcaraHttpRequest) }
}

fn res_ref(ptr: i64) -> &'static OcaraHttpResponse {
    unsafe { &*(ptr as *const OcaraHttpResponse) }
}

fn alloc_req(req: OcaraHttpRequest) -> i64 {
    Box::into_raw(Box::new(req)) as i64
}

fn alloc_res(res: OcaraHttpResponse) -> i64 {
    Box::into_raw(Box::new(res)) as i64
}

// ─── Exécution de la requête ─────────────────────────────────────────────────

fn do_request(
    url:        &str,
    method:     &str,
    headers:    &[(String, String)],
    body:       Option<&str>,
    timeout_ms: Option<u64>,
) -> OcaraHttpResponse {
    let agent = {
        let mut b = ureq::AgentBuilder::new();
        if let Some(ms) = timeout_ms {
            b = b.timeout(std::time::Duration::from_millis(ms));
        }
        b.build()
    };

    let req = match method.to_uppercase().as_str() {
        "POST"   => agent.post(url),
        "PUT"    => agent.put(url),
        "DELETE" => agent.delete(url),
        "PATCH"  => agent.patch(url),
        "HEAD"   => agent.head(url),
        _        => agent.get(url),
    };

    let req = headers.iter().fold(req, |r, (k, v)| r.set(k, v));

    let result = if let Some(b) = body {
        req.send_string(b)
    } else {
        req.call()
    };

    match result {
        Ok(resp) => {
            let status = resp.status() as i64;
            let names  = resp.headers_names();
            let mut hdrs: Vec<(String, String)> = names
                .iter()
                .filter_map(|n| resp.header(n).map(|v| (n.clone(), v.to_string())))
                .collect();
            // Trier pour un affichage prévisible
            hdrs.sort_by(|a, b| a.0.cmp(&b.0));
            let body_str = resp.into_string().unwrap_or_default();
            OcaraHttpResponse {
                status,
                body: body_str,
                headers: hdrs,
                is_err: false,
                error:  String::new(),
            }
        }
        Err(ureq::Error::Status(code, resp)) => {
            let body_str = resp.into_string().unwrap_or_default();
            OcaraHttpResponse {
                status:  code as i64,
                body:    body_str,
                headers: Vec::new(),
                is_err:  true,
                error:   format!("HTTP {}", code),
            }
        }
        Err(e) => {
            OcaraHttpResponse {
                status:  0,
                body:    String::new(),
                headers: Vec::new(),
                is_err:  true,
                error:   e.to_string(),
            }
        }
    }
}

// ─── API publique ─────────────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn HTTPRequest_new(url: i64) -> i64 {
    let url_str = unsafe { ptr_to_str(url).to_string() };
    alloc_req(OcaraHttpRequest {
        url:     url_str,
        method:  "GET".into(),
        headers: Vec::new(),
        body:    None,
        timeout: None,
    })
}

#[no_mangle]
pub extern "C" fn HTTPRequest_set_method(req: i64, method: i64) {
    let m = unsafe { ptr_to_str(method).to_string() };
    req_ref(req).method = m;
}

#[no_mangle]
pub extern "C" fn HTTPRequest_set_header(req: i64, k: i64, v: i64) {
    let key = unsafe { ptr_to_str(k).to_string() };
    let val = unsafe { ptr_to_str(v).to_string() };
    req_ref(req).headers.push((key, val));
}

#[no_mangle]
pub extern "C" fn HTTPRequest_set_body(req: i64, body: i64) {
    let b = unsafe { ptr_to_str(body).to_string() };
    req_ref(req).body = Some(b);
}

#[no_mangle]
pub extern "C" fn HTTPRequest_set_timeout(req: i64, ms: i64) {
    req_ref(req).timeout = Some(ms as u64);
}

#[no_mangle]
pub extern "C" fn HTTPRequest_send(req: i64) -> i64 {
    let r  = req_ref(req);
    let res = do_request(
        &r.url.clone(),
        &r.method.clone(),
        &r.headers.clone(),
        r.body.as_deref(),
        r.timeout,
    );
    alloc_res(res)
}

#[no_mangle]
pub extern "C" fn HTTPRequest_status(res: i64) -> i64 {
    if res == 0 { return 0; }
    res_ref(res).status
}

#[no_mangle]
pub extern "C" fn HTTPRequest_body(res: i64) -> i64 {
    if res == 0 { return unsafe { alloc_str("") }; }
    unsafe { alloc_str(&res_ref(res).body.clone()) }
}

#[no_mangle]
pub extern "C" fn HTTPRequest_header(res: i64, name: i64) -> i64 {
    if res == 0 { return unsafe { alloc_str("") }; }
    let k = unsafe { ptr_to_str(name).to_lowercase() };
    let r = res_ref(res);
    for (hk, hv) in &r.headers {
        if hk.to_lowercase() == k {
            return unsafe { alloc_str(hv) };
        }
    }
    unsafe { alloc_str("") }
}

#[no_mangle]
pub extern "C" fn HTTPRequest_headers(res: i64) -> i64 {
    let map_ptr = new_map();
    if res == 0 { return map_ptr; }
    let r = res_ref(res);
    for (k, v) in &r.headers {
        let kp = unsafe { alloc_str(k) };
        let vp = unsafe { alloc_str(v) };
        __map_set(map_ptr, kp, vp);
    }
    map_ptr
}

#[no_mangle]
pub extern "C" fn HTTPRequest_ok(res: i64) -> i64 {
    if res == 0 { return 0; }
    let s = res_ref(res).status;
    if s >= 200 && s < 300 { 1 } else { 0 }
}

#[no_mangle]
pub extern "C" fn HTTPRequest_is_error(res: i64) -> i64 {
    if res == 0 { return 1; }
    if res_ref(res).is_err { 1 } else { 0 }
}

#[no_mangle]
pub extern "C" fn HTTPRequest_error(res: i64) -> i64 {
    if res == 0 { return unsafe { alloc_str("null response") }; }
    unsafe { alloc_str(&res_ref(res).error.clone()) }
}

// ─── Raccourcis statiques ─────────────────────────────────────────────────────

#[no_mangle]
pub extern "C" fn HTTPRequest_get(url: i64) -> i64 {
    let u   = unsafe { ptr_to_str(url).to_string() };
    let res = do_request(&u, "GET", &[], None, None);
    alloc_res(res)
}

#[no_mangle]
pub extern "C" fn HTTPRequest_post(url: i64, body: i64) -> i64 {
    let u   = unsafe { ptr_to_str(url).to_string() };
    let b   = unsafe { ptr_to_str(body).to_string() };
    let res = do_request(&u, "POST", &[], Some(&b), None);
    alloc_res(res)
}

#[no_mangle]
pub extern "C" fn HTTPRequest_put(url: i64, body: i64) -> i64 {
    let u   = unsafe { ptr_to_str(url).to_string() };
    let b   = unsafe { ptr_to_str(body).to_string() };
    let res = do_request(&u, "PUT", &[], Some(&b), None);
    alloc_res(res)
}

#[no_mangle]
pub extern "C" fn HTTPRequest_delete(url: i64) -> i64 {
    let u   = unsafe { ptr_to_str(url).to_string() };
    let res = do_request(&u, "DELETE", &[], None, None);
    alloc_res(res)
}

#[no_mangle]
pub extern "C" fn HTTPRequest_patch(url: i64, body: i64) -> i64 {
    let u   = unsafe { ptr_to_str(url).to_string() };
    let b   = unsafe { ptr_to_str(body).to_string() };
    let res = do_request(&u, "PATCH", &[], Some(&b), None);
    alloc_res(res)
}
