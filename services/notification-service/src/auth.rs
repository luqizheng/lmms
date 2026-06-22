pub use jwt_ctx::Principal;

pub type Identity = Principal;

pub trait AdminCheck {
    fn is_admin(&self) -> bool;
}

impl AdminCheck for Principal {
    fn is_admin(&self) -> bool {
        self.has_role("admin") || self.has_role("notification.admin")
    }
}
