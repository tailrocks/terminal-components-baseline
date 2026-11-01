use super::docker::DockerSetting;
use super::doctor::DoctorSurface;
use super::role::RoleAdmin;

#[derive(Debug, Clone, Default)]
pub struct Surfaces {
    pub roles: Option<RoleAdmin>,
    pub docker: Option<DockerSetting>,
    pub doctor: Option<DoctorSurface>,
}
