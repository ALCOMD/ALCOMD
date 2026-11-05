//! Finite settings/Hub classifications with private technical context.
use crate::TechnicalErrorContext;

macro_rules! contextual_error {
    ($name:ident, $kind:ident, {$($variant:ident => $message:literal),+ $(,)?}) => {
        #[derive(Clone, Copy, Debug, Eq, PartialEq)]
        pub enum $kind { $($variant),+ }
        #[derive(Clone, Debug)]
        pub struct $name { kind: $kind, context: Option<TechnicalErrorContext> }
        // Existing equality assertions concern the finite failure classification;
        // technical context and diagnostic identities are observed separately.
        impl PartialEq for $name { fn eq(&self, other:&Self)->bool {self.kind==other.kind} }
        impl Eq for $name {}
        #[allow(non_upper_case_globals)]
        impl $name {
            $(pub const $variant: Self = Self {kind:$kind::$variant,context:None};)+
            pub fn kind(&self)->$kind {self.kind}
            pub fn path(&self)->Option<&str>{self.context.as_ref().and_then(|v|v.path.as_deref())}
            pub fn diagnostic_id(&self)->Option<&str>{self.context.as_ref().and_then(|v|v.diagnostic_id.as_deref())}
            pub fn with_source(mut self,error:impl std::error::Error+Send+Sync+'static)->Self {
                self.context=Some(self.context.take().unwrap_or_default().with_source(error));self
            }
            pub fn with_path(mut self,path:impl Into<String>)->Self {
                self.context=Some(self.context.take().unwrap_or_default().with_path(path));self
            }
            pub fn with_reason(mut self,reason:impl Into<String>)->Self {
                self.context=Some(self.context.take().unwrap_or_default().with_reason(reason));self
            }
            pub fn with_diagnostic_id(mut self,id:Option<String>)->Self {
                self.context.get_or_insert_with(Default::default).diagnostic_id=id;self
            }
        }
        impl std::fmt::Display for $name {
            fn fmt(&self,f:&mut std::fmt::Formatter<'_>)->std::fmt::Result {
                f.write_str(match self.kind {$($kind::$variant=>$message),+})
            }
        }
        impl std::error::Error for $name {
            fn source(&self)->Option<&(dyn std::error::Error+'static)> {
                self.context.as_ref().and_then(TechnicalErrorContext::source_error)
            }
        }
    }
}

contextual_error!(OfficialGuiError, OfficialGuiErrorKind, {
    InvalidInput => "invalid settings input",
    PermissionDenied => "settings access denied",
    RevisionConflict => "settings revision changed",
    Unavailable => "settings storage unavailable",
    Corrupt => "settings document is invalid",
});
contextual_error!(ConfigStoreError, ConfigStoreErrorKind, {
    Unavailable => "configuration storage unavailable",
    Corrupt => "configuration document is invalid",
    TooLarge => "configuration document exceeds the size limit",
});
contextual_error!(UnityHubOpenError, UnityHubOpenErrorKind, {
    InvalidInput => "invalid Unity Hub request",
    PermissionDenied => "Unity Hub access denied",
    RevisionConflict => "Unity Hub settings revision changed",
    PathChanged => "Unity Hub executable path changed",
    PathInvalid => "Unity Hub executable path is invalid",
    Unavailable => "Unity Hub location unavailable",
    Busy => "Unity Hub dispatch capacity unavailable",
    OpenFailed => "Unity Hub process creation failed",
    Internal => "Unity Hub dispatch failed",
});

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn propagated_configuration_error_preserves_its_context_and_identity() {
        let diagnostic = crate::OperationId::new().to_string();
        let source = ConfigStoreError::Unavailable
            .with_source(std::io::Error::other("isolated disk failure"))
            .with_path("C:/测试/settings.toml")
            .with_diagnostic_id(Some(diagnostic.clone()));
        let propagated = OfficialGuiError::from(source);
        assert_eq!(propagated.diagnostic_id(), Some(diagnostic.as_str()));
        assert_eq!(propagated.path(), Some("C:/测试/settings.toml"));
        assert!(crate::technical_error_chain(&propagated).contains("isolated disk failure"));
    }
}
