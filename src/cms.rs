use crate::AppState;
use crate::user::verify_token;
use actix_web::{get, post, web, HttpResponse, Responder, HttpRequest};
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize)]
pub struct ArticleData {
    slug: String,
    title: String,
    description: String,
    author: String,
    body: String,
}

#[derive(Deserialize)]
pub struct CreateArticlePath {
    org_id: String,
}

#[post("/organizations/{org_id}/articles")]
pub async fn create_article(
    app_state: web::Data<AppState>,
    article_data: web::Json<ArticleData>,
    path: web::Path<CreateArticlePath>,
    req: HttpRequest,
) -> impl Responder {
    println!("RUnn");
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
    let check = {
        let data = sqlx::query!(
            "SELECT * FROM OrganizationMember WHERE orgId = ? AND userId = ?",
            path.org_id, user.id
        ).fetch_optional(&*pool).await.unwrap();
        data.is_some()
    };
    if !check {
        return HttpResponse::Forbidden().body("Forbidden");
    }
    sqlx::query!(
        "INSERT INTO Article VALUES (?, ?, ?, ?, ?, ?)",
        path.org_id,
        article_data.slug,
        article_data.title,
        article_data.description,
        article_data.author,
        article_data.body,
    ).execute(&*pool).await.unwrap();
    HttpResponse::Ok().body("Created")
}

#[get("/organizations/{org_id}/articles")]
pub async fn get_articles(
    app_state: web::Data<AppState>,
    path: web::Path<CreateArticlePath>,
) -> impl Responder {
    let pool = app_state.pool.lock().unwrap();
    let data = sqlx::query!(
        "SELECT * FROM Article WHERE orgId = ?",
        path.org_id
    ).fetch_all(&*pool).await.unwrap();
    let mut articles = Vec::new();
    for article in data {
        articles.push(ArticleData {
            slug: article.slug.unwrap(),
            title: article.title.unwrap(),
            description: article.description.unwrap(),
            author: article.authorId.unwrap(),
            body: article.body.unwrap(),
        });
    }
    HttpResponse::Ok().json(articles)
}

#[derive(Deserialize)]
pub struct GetArticlePath {
    org_id: String,
    slug: String,
}

#[get("/organizations/{org_id}/articles/{slug}")]
pub async fn get_article(
    app_state: web::Data<AppState>,
    path: web::Path<GetArticlePath>,
) -> impl Responder {
    let pool = app_state.pool.lock().unwrap();
    let data = sqlx::query!(
        "SELECT * FROM Article WHERE orgId = ? AND slug = ?",
        path.org_id, path.slug
    ).fetch_optional(&*pool).await.unwrap();
    if data.is_none() {
        return HttpResponse::NotFound().body("Not Found");
    }
    let data = data.unwrap();
    HttpResponse::Ok().json(ArticleData {
        slug: data.slug.unwrap(),
        title: data.title.unwrap(),
        description: data.description.unwrap(),
        body: data.body.unwrap(),
        author: data.authorId.unwrap(),
    })
}