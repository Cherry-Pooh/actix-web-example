use super::db::*;

use actix_identity::Identity;
use actix_web::{post, web::block, web::Data, web::Form, Responder};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use diesel::insert_into;

fn sha256(s: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.input(s);
    hasher.result().to_vec()
}

#[derive(Serialize, Deserialize)]
pub struct FormParams {
    username: String,
    password: String,
}

#[post("/acc_create")]
pub async fn create_account(pool: Data<DbPool>, params: Form<FormParams>) -> impl Responder {
    let pass_hash = sha256(&params.password);

    match block(move || {
        let conn = pool.get().expect("Could not get db connection");
        insert_into(accounts)
            .values(&models::Account {
                username: params.username.clone(),
                password_hash: pass_hash,
            })
            .execute(&conn)
    })
    .await
    {
        Ok(_) => "Account created! <a href='/'>Login</a>",
        Err(_) => "Username exists",
    }
}

#[post("/login_request")]
pub async fn login_request(
    id: Identity,
    pool: Data<DbPool>,
    params: Form<FormParams>,
) -> impl Responder {
    let username = params.username.clone();

    match get_user_from_database(username, pool).await {
        Ok(result) => match result.len() {
            0 => "No such user",
            _ => {
                if result[0].password_hash == sha256(&params.password) {
                    id.remember(params.username.clone());
                    "LOGIN_SUCCESS"
                } else {
                    "Wrong Password"
                }
            }
        },
        Err(e) => panic!(e),
    }
}

#[derive(Serialize, Deserialize)]
pub struct ChangePassParams {
    oldpass: String,
    newpass: String,
}

#[post("/chpass_request")]
pub async fn chpass_request(
    id: Identity,
    pool: Data<DbPool>,
    params: Form<ChangePassParams>,
) -> impl Responder {
    let pool1 = pool.clone();

    match id.identity() {
        None => "Invalid session",
        Some(username) => {
            let name = username.clone();
            match get_user_from_database(username, pool).await {
                Ok(result) => {
                    if result[0].password_hash == sha256(&params.oldpass) {
                        change_password(
                            name,
                            sha256(&params.newpass),
                            pool1.get().expect("Could not get db connection"),
                        )
                        .await;
                        "Password changed"
                    } else {
                        "Wrong Password"
                    }
                }
                Err(e) => panic!(e),
            }
        }
    }
}

#[post("/confirm_delacc")]
pub async fn confirm_delacc(
    id: Identity,
    pool: Data<DbPool>,
    body: bytes::Bytes,
) -> impl Responder {
    let password = String::from_utf8_lossy(&body);

    match id.identity() {
        None => "Invalid session",
        Some(username) => {
            let pool1 = pool.clone();
            let name = username.clone();

            match block(move || {
                let conn = pool.get().expect("Could not get db connection");
                accounts
                    .filter(dsl::username.eq(name))
                    .load::<models::Account>(&conn)
            })
            .await
            {
                Ok(result) => {
                    if result[0].password_hash == sha256(&password) {
                        delete_user(username, pool1.get().expect("Could not get db connection"))
                            .await;
                        id.forget();
                        "Account deleted"
                    } else {
                        "Wrong Password"
                    }
                }
                Err(e) => panic!(e),
            }
        }
    }
}