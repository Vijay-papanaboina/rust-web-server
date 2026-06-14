use crate::mrequest::Request;

pub fn logger(request: &Request) {
    println!("Request-Line: {} {}", request.method, request.path);
    if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&request.body) {
        println!("Request-Body:{json:#}\n");
    }
}
