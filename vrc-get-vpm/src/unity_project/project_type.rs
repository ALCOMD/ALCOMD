use crate::io::IoTrait;
use crate::{ProjectType, UnityProject};

pub(crate) const VRCHAT_AVATARS_PACKAGE: &str = "com.vrchat.avatars";
pub(crate) const VRCHAT_WORLDS_PACKAGE: &str = "com.vrchat.worlds";

impl UnityProject {
    pub async fn detect_project_type(&self) -> ProjectType {
        if self.get_locked(VRCHAT_AVATARS_PACKAGE).is_some() {
            return ProjectType::Avatars;
        } else if self.get_locked(VRCHAT_WORLDS_PACKAGE).is_some() {
            return ProjectType::Worlds;
        } else if self.manifest.has_any() {
            return ProjectType::VpmStarter;
        }

        if self.has_upm_package(VRCHAT_AVATARS_PACKAGE) {
            return ProjectType::UpmAvatars;
        } else if self.has_upm_package(VRCHAT_WORLDS_PACKAGE) {
            return ProjectType::UpmWorlds;
        } else if self.has_upm_package("com.vrchat.base") {
            return ProjectType::UpmStarter;
        }

        // VRCSDK2.dll is for SDK2
        if self
            .io
            .is_file("Assets/VRCSDK/Plugins/VRCSDK2.dll".as_ref())
            .await
        {
            return ProjectType::LegacySdk2;
        }

        // VRCSDK3.dll is for SDK3 Worlds
        if self
            .io
            .is_file("Assets/VRCSDK/Plugins/VRCSDK3.dll".as_ref())
            .await
        {
            return ProjectType::LegacyWorlds;
        }

        // VRCSDK3A.dll is for SDK3 Worlds
        if self
            .io
            .is_file("Assets/VRCSDK/Plugins/VRCSDK3A.dll".as_ref())
            .await
        {
            return ProjectType::LegacyAvatars;
        }

        ProjectType::Unknown
    }
}
