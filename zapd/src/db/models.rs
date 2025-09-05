
#[derive(Debug,Clone,sqlx::FromRow)]
pub struct UserModel {
    pub id : u64,
    pub username : String,
    pub password : String,
}