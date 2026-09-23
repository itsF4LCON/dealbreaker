mod room;

use serde::Serialize;
use worker::*;

pub use room::Room;

const CODE_ALPHABET: &[u8] = b"BCDFGHJKLMNPQRSTVWXZ";
const CODE_LEN: usize = 4;
const CREATE_ATTEMPTS: usize = 6;
const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; img-src 'self' data:; font-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'";

#[derive(Serialize)]
struct Created {
    code: String,
}

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    let path = req.path();
    let segments: Vec<&str> = path.trim_matches('/').split('/').collect();

    match (req.method(), segments.as_slice()) {
        (Method::Post, ["api", "rooms"]) => with_headers(create_room(&req, &env).await?),
        (Method::Get, ["api", "rooms", code]) => {
            let Some(code) = normalize_code(code) else {
                return with_headers(Response::error("Room not found", 404)?);
            };
            let init = RequestInit::new();
            let res = room_stub(&env, &code)?
                .fetch_with_request(Request::new_with_init("https://room/info", &init)?)
                .await?;
            with_headers(copy_response(res).await?)
        }
        (Method::Get, ["api", "rooms", code, "ws"]) => {
            let Some(code) = normalize_code(code) else {
                return Response::error("Room not found", 404);
            };
            let upgrade = req.headers().get("Upgrade")?.unwrap_or_default();
            if !upgrade.eq_ignore_ascii_case("websocket") {
                return Response::error("Expected a WebSocket", 426);
            }
            room_stub(&env, &code)?.fetch_with_request(req).await
        }
        (Method::Get | Method::Head, ["r", code]) if normalize_code(code).is_some() => {
            let mut url = req.url()?;
            url.set_path("/room.html");
            let mut page = env.assets("ASSETS")?.fetch(url.to_string(), None).await?;
            with_headers(Response::from_html(page.text().await?)?)
        }
        _ => with_headers(Response::error("Not found", 404)?),
    }
}

async fn create_room(req: &Request, env: &Env) -> Result<Response> {
    if let Ok(limiter) = env.rate_limiter("CREATE_LIMITER") {
        let ip = req.headers().get("CF-Connecting-IP")?.unwrap_or_else(|| "local".into());
        if !limiter.limit(ip).await?.success {
            return Response::error("Too many rooms created. Try again in a minute.", 429);
        }
    }
    for _ in 0..CREATE_ATTEMPTS {
        let code = random_code()?;
        let mut init = RequestInit::new();
        init.with_method(Method::Post);
        let res = room_stub(env, &code)?
            .fetch_with_request(Request::new_with_init("https://room/create", &init)?)
            .await?;
        if res.status_code() == 201 {
            return Response::from_json(&Created { code }).map(|r| r.with_status(201));
        }
    }
    Response::error("Couldn't find a free room code. Try again.", 503)
}

fn room_stub(env: &Env, code: &str) -> Result<Stub> {
    env.durable_object("ROOMS")?.id_from_name(code)?.get_stub()
}

fn normalize_code(raw: &str) -> Option<String> {
    let code = raw.to_ascii_uppercase();
    (code.len() == CODE_LEN && code.bytes().all(|b| CODE_ALPHABET.contains(&b))).then_some(code)
}

fn random_code() -> Result<String> {
    let seed = room::random_u64()?;
    Ok((0..CODE_LEN)
        .map(|i| CODE_ALPHABET[((seed >> (i * 8)) % CODE_ALPHABET.len() as u64) as usize] as char)
        .collect())
}

async fn copy_response(mut res: Response) -> Result<Response> {
    let status = res.status_code();
    let body = res.text().await?;
    let mut copy = Response::ok(body)?.with_status(status);
    copy.headers_mut().set("Content-Type", "application/json")?;
    Ok(copy)
}

fn with_headers(mut res: Response) -> Result<Response> {
    let h = res.headers_mut();
    h.set("Cache-Control", "no-store")?;
    h.set("Referrer-Policy", "no-referrer")?;
    h.set("X-Content-Type-Options", "nosniff")?;
    h.set("Content-Security-Policy", CSP)?;
    Ok(res)
}
