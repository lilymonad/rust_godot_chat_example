use actix_identity::{
    Identity,
    CookieIdentityPolicy,
    IdentityService,
};
use actix::{Actor, StreamHandler, Addr, Handler, Message};
use actix_web_actors::ws;
use actix_web::{
    web,
    Error,
    App,
    HttpResponse,
    HttpServer,
    HttpRequest,
    middleware::Logger,
};
use std::collections::HashMap;
use std::sync::{Mutex};


/// HTTP PART

/// ChatData represent the state of the HTTP server.
/// It contains the chat messages as well as the state of each user.
///
/// We are impementing an asynchronous message system which let people
/// request unread messages.
///
/// - messages contains all messages from the server start
/// - user_state contains user data (here, only the number of unread message)
/// - user_ws contains each user websocket used to ping them each time a message is sent
struct ChatData {
    messages: Vec<String>,
    user_state: HashMap<String, usize>,
    user_ws: HashMap<String, Addr<Ws>>,
}
type ChatState = web::Data<Mutex<ChatData>>;

/// The index of the server (only useful for HTTP response when logging with /login /logout)
async fn index(id:Identity) -> String {
    format!(
        "Hello {}",
        id.identity().unwrap_or_else(|| "Anonymous".to_owned())
    )
}

/// /login path handler
///
/// Registers an Identity cookie for the client and redirect to the site's root (/)
async fn login(data:ChatState, id:Identity, req:String) -> HttpResponse {
    println!("login with infos: {}", req);
    id.remember(req.clone());

    let mut dlock = data.lock().unwrap();
    let nbm = dlock.messages.len();
    dlock.user_state.insert(req, nbm);

    HttpResponse::SeeOther().header("location", "/").finish()
}

/// /logout path handler
///
/// Forget a user's Identity cookie
async fn logout(id:Identity) -> HttpResponse {
    id.forget();
    HttpResponse::SeeOther().header("location", "/").finish()
}

/// POST /message handler
///
/// Sends a message from "id" to everyone, and ping them using their websocket handle
async fn send_message(data:ChatState, id:Identity, message:String)
    -> Option<String>
{
    id.identity().map(|username| {
        let mut dlock = data.lock().unwrap();
        dlock.messages.push(format!("{}: {}", username, message));
        for (_, unread_messages) in &mut dlock.user_state {
            *unread_messages += 1
        }
        println!("Received {} from {}", message, username);
        for (_, ws) in &mut dlock.user_ws {
            ws.do_send(NewMessage)
        }
        String::new()
    })
}

/// GET /message handler
///
/// Update the count of unread messages of the user "id"
/// and sends him all unread messages
async fn get_messages(data:ChatState, id:Identity) -> Option<String> {
    let username = id.identity()?;
    let mut dlock = data.lock().unwrap();
    let len = dlock.messages.len();
    let unread = dlock.user_state.get_mut(&username)?;

    let start_offset = len - *unread;
    *unread = 0;

    let mut ret = String::new();
    for msg in &dlock.messages[start_offset..] {
        ret.push_str(msg);
        ret.push('\n');
    }

    println!("Sending: {} to {}", ret, username);
    Some(ret)
}

/// WEBSOCKET PART

/// This will represent a websocket's client data
struct Ws;

/// We need to tell actix it is an Actor with a Context of type WebsocketContext,
/// because actix-web-actors accept only this kind of Actor to be started as a websocket listener
impl Actor for Ws {
    type Context = ws::WebsocketContext<Self>;
}

/// We also need to implement our own function for handling messages
///
/// This allows us to discard some types of messages or to do some other stuff like command parsing
/// Here we just log the message
impl StreamHandler<Result<ws::Message, ws::ProtocolError>> for Ws {
    fn handle(
        &mut self,
        msg: Result<ws::Message, ws::ProtocolError>,
        ctx: &mut Self::Context
    ) {
        println!("{:?}", msg);
    }
}

/// Actix messaging system is type based.
/// For each new protocol you want to handle, you can create a Type
/// which implement Message, and implement Handler<Type> for an actor.
///
/// Here we just want to be able to say "hey you have a new message"
/// and we implement it by sending text to the websocket
struct NewMessage;
impl Message for NewMessage {
    type Result = ();
}

impl Handler<NewMessage> for Ws {
    type Result = ();
    fn handle(
        &mut self,
        _msg: NewMessage,
        ctx: &mut Self::Context
    ) -> Self::Result {
        ctx.text("hello");
    }
}

async fn ws_connect(
    data:ChatState,
    req:HttpRequest,
    stream:web::Payload,
    path:web::Path<(String,)>
)
    -> Result<HttpResponse, Error>
{
    let (addr, resp) = ws::start_with_addr(Ws, &req, stream)?;
    println!("{:?}", resp);

    let mut dlock = data.lock().unwrap();
    dlock.user_ws.insert(path.0.clone(), addr);
    Ok(resp)
}

#[actix_rt::main]
async fn main() -> std::io::Result<()> {
    // init logger for debug
    std::env::set_var("RUST_LOG", "actix_web=info");
    env_logger::init();

    // Init server data
    let data = web::Data::new(Mutex::new(ChatData {
        messages: vec![],
        user_state: HashMap::new(),
        user_ws: HashMap::new(),
    }));

    // create and run the server
    HttpServer::new(move || {
        App::new()
            .app_data(data.clone())
            .wrap(Logger::default())
            .wrap(IdentityService::new(
                    CookieIdentityPolicy::new(&[0; 32])
                        .name("auth-example")
                        .secure(false)
            ))
            .service(web::resource("/login").route(web::post().to(login)))
            .service(web::resource("/logout").to(logout))
            .service(web::resource("/").route(web::get().to(index)))
            .service(web::resource("/message")
                     .route(web::post().to(send_message))
                     .route(web::get().to(get_messages))
            )
            .route("/ws/{username}", web::get().to(ws_connect))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
