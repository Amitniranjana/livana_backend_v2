use crate::repository::user_repository::UserRepository;
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct UserService{
    pub user_repository:UserRepository,
}


impl UserService{
    pub fn new(user_repository: UserRepository)->Self{
        UserService{
            user_repository,
        }
    }
#[allow(dead_code)]
    pub fn create_user(){


    }
}