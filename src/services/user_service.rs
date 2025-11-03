use crate::repository::user_repository::UserRepository;
#[derive(Clone, Debug)]
pub struct UserService{
    pub user_repository:UserRepository,
}


impl UserService{
    pub fn new(user_repository: UserRepository)->Self{
        UserService{
            user_repository,
        }
    }

    pub fn create_user(){
        
        
    }
}