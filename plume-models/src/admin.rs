use users::User;

/// Wrapper around User to use as a request guard on pages reserved to admins.
pub struct Admin(pub User);
