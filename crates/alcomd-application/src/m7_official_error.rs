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
            /// Called only by the selected source execution owner, never an RPC encoder.
            pub fn record(mut self,message:&'static str)->Self {
                if self.diagnostic_id().is_none() {
                    let id=crate::record_technical_failure(&self,self.path(),None,message);
                    self.context.get_or_insert_with(Default::default).diagnostic_id=Some(id);
                }
                self
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
    ActivityNotFound => "activity not found",
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
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    struct CountErrors(Arc<AtomicUsize>);
    impl tracing::Subscriber for CountErrors {
        fn enabled(&self, metadata: &tracing::Metadata<'_>) -> bool {
            *metadata.level() == tracing::Level::ERROR
        }
        fn new_span(&self, _: &tracing::span::Attributes<'_>) -> tracing::span::Id {
            tracing::span::Id::from_u64(1)
        }
        fn record(&self, _: &tracing::span::Id, _: &tracing::span::Record<'_>) {}
        fn record_follows_from(&self, _: &tracing::span::Id, _: &tracing::span::Id) {}
        fn event(&self, _: &tracing::Event<'_>) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
        fn enter(&self, _: &tracing::span::Id) {}
        fn exit(&self, _: &tracing::span::Id) {}
    }
    #[test]
    fn source_error_is_reported_once_and_propagation_preserves_its_identity() {
        let count = Arc::new(AtomicUsize::new(0));
        tracing::subscriber::with_default(CountErrors(Arc::clone(&count)), || {
            let source = ConfigStoreError::Unavailable
                .with_source(std::io::Error::other("isolated disk failure"))
                .with_path("C:/测试/settings.toml")
                .record("failed to read settings");
            let diagnostic = source
                .diagnostic_id()
                .expect("source diagnostic")
                .to_owned();
            let propagated = OfficialGuiError::from(source)
                .record("must not duplicate an already recorded failure");
            assert_eq!(propagated.diagnostic_id(), Some(diagnostic.as_str()));
            assert_eq!(propagated.path(), Some("C:/测试/settings.toml"));
            assert!(crate::technical_error_chain(&propagated).contains("isolated disk failure"));
            assert_eq!(count.load(Ordering::Relaxed), 1);
        });
    }
}
