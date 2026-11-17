//! Wire configuration is part of the pinned server/client compatibility contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Profile {
    pub revision: u32,
    pub use_isaac: bool,
    pub login_use_strings: bool,
    pub login_extra_info: bool,
    pub login_fake_index_28: bool,
    /// The pinned LoginManager writes base-37 regardless of LOGIN_USE_STRINGS.
    pub login_name_encoding: LoginNameEncoding,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LoginNameEncoding {
    Base37,
}

pub const RS09_REVISION_530: Profile = Profile {
    revision: 530,
    use_isaac: false,
    login_use_strings: true,
    login_extra_info: true,
    login_fake_index_28: true,
    login_name_encoding: LoginNameEncoding::Base37,
};
