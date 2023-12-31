use actix_web::{get, web, HttpRequest, HttpResponse, Responder};
use jsonwebtoken::{encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use reqwest::Client;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::AppState;
use std::collections::HashMap;
use std::env;

#[derive(Deserialize)]
pub struct CallbackData {
    code: String,
}

#[derive(Deserialize, Debug, Serialize)]
pub struct User {
    pub id: String,
    pub name: String,
    email: String,
    exp: i64,
    login: Option<String>,
}

fn create_token(user_id: String, user_name: String, email: String, login: String) -> String {
    let mut header = Header::default();
    header.alg = Algorithm::HS512;
    header.typ = Some("JWT".to_string());
    let now = chrono::Utc::now();
    let exp = now.timestamp() + 60 * 60 * 24;
    let claims = User {
        id: user_id,
        name: user_name,
        email,
        exp,
        login: Some(login),
    };
    let key = env::var("JWT_SECRET").unwrap();
    let token = encode(&header, &claims, &EncodingKey::from_secret(key.as_ref())).unwrap();
    token
}

fn create_oauth_url() -> String {
    let mut url = Url::parse("https://github.com/login/oauth/authorize").unwrap();
    let client_id = format!("client_id={}", env::var("GITHUB_CLIENT_ID").unwrap());
    url.set_query(Some(
        &(client_id
            + "&"
            + format!("redirect_uri={}", env::var("GITHUB_REDIRECT_URI").unwrap()).as_str()
            + "&"
            + "scope=user"),
    ));
    url.to_string()
}

pub fn verify_token(token: String) -> Result<User, jsonwebtoken::errors::Error> {
    let key = env::var("JWT_SECRET").unwrap();
    let validation = Validation::new(Algorithm::HS512);
    let token_data =
        jsonwebtoken::decode::<User>(&token, &DecodingKey::from_secret(key.as_ref()), &validation);
    let user = token_data.unwrap().claims;
    Ok(user)
}

async fn fetch_access_token(code: String) -> String {
    let mut form = HashMap::new();
    form.insert("client_id", env::var("GITHUB_CLIENT_ID").unwrap());
    form.insert("client_secret", env::var("GITHUB_CLIENT_SECRET").unwrap());
    form.insert("code", code);
    form.insert("redirect_uri", env::var("GITHUB_REDIRECT_URI").unwrap());
    let client = Client::new();
    let data = client
        .post("https://github.com/login/oauth/access_token")
        .header("Accept", "application/json")
        .form(&form)
        .send()
        .await
        .unwrap()
        .json::<serde_json::Value>()
        .await
        .unwrap();
    println!("{:?}", data);
    data["access_token"].as_str().unwrap().to_string()
}

#[get("/users/callback")]
pub async fn callback(
    data: web::Query<CallbackData>,
    app_state: web::Data<AppState>,
) -> impl Responder {
    let client = Client::new();
    let code = data.code.clone();
    let token = fetch_access_token(code).await;
    let res = client
        .post("https://api.github.com/user")
        .header("Accept", "application/json")
        .header("Authorization", format!("Bearer {}", token))
        .header("User-Agent", "reqwest")
        .send()
        .await
        .unwrap();
    let user = res.json::<serde_json::Value>().await.unwrap();
    let user_id = user["id"].to_string();
    let pool = app_state.pool.lock().unwrap();
    let data = sqlx::query!("SELECT * FROM User WHERE id = ?", user_id)
        .fetch_optional(&*pool)
        .await
        .unwrap();
    if data.is_none() {
        sqlx::query!(
            "INSERT INTO User VALUES (?, ?)",
            user_id,
            user["name"].to_string(),
        )
        .execute(&*pool)
        .await
        .unwrap();
    }
    // create jwt token
    let token = create_token(
        user_id,
        user["name"].to_string(),
        user["email"].to_string(),
        user["login"].to_string(),
    );
    let responde_data = serde_json::json!({
        "token": token,
    });
    web::Json(responde_data)
}

#[get("/users/oauth_url")]
pub async fn oauth_url() -> impl Responder {
    let url = create_oauth_url();
    web::Json(serde_json::json!({
        "url": url,
    }))
}

#[get("/users/me")]
pub async fn get_me(app_state: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    let token = {
        let headers = req.headers();
        let authorization = headers.get("Authorization").unwrap();
        let token = authorization.to_str().unwrap();
        // split token
        let token = token.split(" ").collect::<Vec<&str>>();
        token[1]
    };
    let user = verify_token(token.to_string()).unwrap();
    let pool = app_state.pool.lock().unwrap();
    let data = sqlx::query!("SELECT * FROM User WHERE id = ?", user.id)
        .fetch_optional(&*pool)
        .await
        .unwrap();
    if data.is_none() {
        return HttpResponse::NotFound().body("Not Found");
    }
    HttpResponse::Ok().json(user)
}

#[derive(Deserialize, Serialize)]
pub struct UserData {
    pub name: String,
    pub id: String,
}

#[derive(Deserialize)]
pub struct UserDataPath {
    pub user_id: String,
}

#[get("/users/{user_id}")]
pub async fn get_user(
    app_state: web::Data<AppState>,
    path: web::Path<UserDataPath>,
) -> impl Responder {
    let pool = app_state.pool.lock().unwrap();
    let data = sqlx::query!("SELECT * FROM User WHERE id = ?", path.user_id)
        .fetch_optional(&*pool)
        .await
        .unwrap();
    if data.is_none() {
        return HttpResponse::NotFound().body("Not Found");
    }
    let data = data.unwrap();
    HttpResponse::Ok().json(UserData {
        id: data.id.unwrap(),
        name: data.userName.unwrap(),
    })
}
