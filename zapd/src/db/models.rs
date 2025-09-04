
#[derive(Debug,Clone,sqlx::FromRow)]
pub struct UserModel {
    pub username : String,
    pub password : String,
}