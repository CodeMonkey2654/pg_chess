//! grpc-web transport for WASM (fetch + prost framing).

use prost::Message;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Headers, Request, RequestInit, RequestMode, Response};

const GRPC_WEB_CT: &str = "application/grpc-web+proto";

fn api_base() -> String {
    option_env!("GAMBIT_STUDIO_API")
        .unwrap_or("http://127.0.0.1:8080")
        .to_string()
}

fn encode_frame(msg: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(5 + msg.len());
    buf.push(0);
    let len = msg.len() as u32;
    buf.extend_from_slice(&len.to_be_bytes());
    buf.extend_from_slice(msg);
    buf
}

fn parse_trailer_status(trailer: &[u8]) -> Option<String> {
    let text = String::from_utf8_lossy(trailer);
    for line in text.split('\r') {
        let line = line.trim();
        if let Some(msg) = line.strip_prefix("grpc-message:") {
            return Some(msg.trim().to_string());
        }
        if let Some(code) = line.strip_prefix("grpc-status:") {
            let code = code.trim();
            if code != "0" && !text.contains("grpc-message:") {
                return Some(format!("gRPC status {code}"));
            }
        }
    }
    None
}

fn decode_frames(data: &[u8]) -> Result<Vec<(u8, Vec<u8>)>, String> {
    let mut frames = Vec::new();
    let mut pos = 0;
    while pos + 5 <= data.len() {
        let flags = data[pos];
        pos += 1;
        let len =
            u32::from_be_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]) as usize;
        pos += 4;
        if pos + len > data.len() {
            return Err("truncated grpc-web frame".into());
        }
        frames.push((flags, data[pos..pos + len].to_vec()));
        pos += len;
    }
    Ok(frames)
}

async fn fetch_post(path: &str, body: Vec<u8>) -> Result<Response, String> {
    let url = format!("{}{path}", api_base());
    let opts = RequestInit::new();
    opts.set_method("POST");
    opts.set_mode(RequestMode::Cors);
    opts.set_body(&js_sys::Uint8Array::from(body.as_slice()).into());

    let headers = Headers::new().map_err(|e| format!("{e:?}"))?;
    headers
        .set("content-type", GRPC_WEB_CT)
        .map_err(|e| format!("{e:?}"))?;
    headers
        .set("accept", GRPC_WEB_CT)
        .map_err(|e| format!("{e:?}"))?;
    headers
        .set("x-grpc-web", "1")
        .map_err(|e| format!("{e:?}"))?;
    opts.set_headers(&headers);

    let request = Request::new_with_str_and_init(&url, &opts).map_err(|e| format!("{e:?}"))?;
    let window = web_sys::window().ok_or("no window")?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| format!("{e:?}"))?;
    resp_value
        .dyn_into::<Response>()
        .map_err(|_| "bad response".into())
}

async fn read_response_bytes(resp: &Response) -> Result<Vec<u8>, String> {
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }
    let buf = JsFuture::from(resp.array_buffer().map_err(|e| format!("{e:?}"))?)
        .await
        .map_err(|e| format!("{e:?}"))?;
    let arr = js_sys::Uint8Array::new(&buf);
    Ok(arr.to_vec())
}

fn decode_unary<Resp: Message + Default>(data: &[u8]) -> Result<Resp, String> {
    let frames = decode_frames(data)?;
    let mut message = None;
    for (flags, payload) in frames {
        if flags & 0x80 != 0 {
            if let Some(err) = parse_trailer_status(&payload) {
                return Err(err);
            }
            continue;
        }
        message = Some(payload);
    }
    let payload = message.ok_or("empty grpc-web response")?;
    Resp::decode(payload.as_slice()).map_err(|e| e.to_string())
}

/// Unary grpc-web RPC call.
pub async fn unary<Req: Message, Resp: Message + Default>(
    method: &str,
    request: &Req,
) -> Result<Resp, String> {
    let mut req_bytes = Vec::new();
    request.encode(&mut req_bytes).map_err(|e| e.to_string())?;
    let path = format!("/gambit.v1.StudioService/{method}");
    let resp = fetch_post(&path, encode_frame(&req_bytes)).await?;
    let bytes = read_response_bytes(&resp).await?;
    decode_unary(&bytes)
}

/// Server-streaming grpc-web RPC; invokes `on_item` for each message.
pub async fn server_streaming<Req: Message, Resp: Message + Default, F>(
    method: &str,
    request: &Req,
    mut on_item: F,
) -> Result<(), String>
where
    F: FnMut(Resp) -> bool,
{
    let mut req_bytes = Vec::new();
    request.encode(&mut req_bytes).map_err(|e| e.to_string())?;
    let path = format!("/gambit.v1.StudioService/{method}");
    let resp = fetch_post(&path, encode_frame(&req_bytes)).await?;
    if !resp.ok() {
        return Err(format!("HTTP {}", resp.status()));
    }

    let body = resp.body().ok_or("no response body")?;
    let reader = body
        .get_reader()
        .dyn_into::<web_sys::ReadableStreamDefaultReader>()
        .map_err(|_| "bad stream reader".to_string())?;

    let mut buffer = Vec::new();
    loop {
        let result = JsFuture::from(reader.read())
            .await
            .map_err(|e| format!("{e:?}"))?;
        let obj = result
            .dyn_into::<js_sys::Object>()
            .map_err(|_| "bad stream chunk")?;
        let done = js_sys::Reflect::get(&obj, &"done".into())
            .map_err(|e| format!("{e:?}"))?
            .as_bool()
            .unwrap_or(true);
        if done {
            break;
        }
        let value = js_sys::Reflect::get(&obj, &"value".into()).map_err(|e| format!("{e:?}"))?;
        if !value.is_undefined() {
            let chunk = js_sys::Uint8Array::new(&value).to_vec();
            buffer.extend_from_slice(&chunk);
        }
    }

    let frames = decode_frames(&buffer)?;
    for (flags, payload) in frames {
        if flags & 0x80 != 0 {
            if let Some(err) = parse_trailer_status(&payload) {
                return Err(err);
            }
            continue;
        }
        let item = Resp::decode(payload.as_slice()).map_err(|e| e.to_string())?;
        if !on_item(item) {
            break;
        }
    }
    Ok(())
}
