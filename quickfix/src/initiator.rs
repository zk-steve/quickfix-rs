use std::marker::PhantomData;

use quickfix_ffi::{
    FixInitiator_block, FixInitiator_delete, FixInitiator_getSession, FixInitiator_isLoggedOn,
    FixInitiator_isStopped, FixInitiator_new, FixInitiator_poll, FixInitiator_start,
    FixInitiator_stop, FixInitiator_t,
};

use crate::{
    utils::{ffi_code_to_bool, ffi_code_to_result},
    Application, ApplicationCallback, ConnectionHandler, FfiMessageStoreFactory,
    FixSocketServerKind, LogFactory, QuickFixError, Session, SessionContainer, SessionId,
    SessionSettings, StdLogger,
};

/// Socket implementation of establishing connections handler.
#[derive(Debug)]
pub struct Initiator<'a, A, S>
where
    A: ApplicationCallback,
    S: FfiMessageStoreFactory,
{
    inner: FixInitiator_t,
    phantom_application: PhantomData<&'a A>,
    phantom_message_store_factory: PhantomData<&'a S>,
    _log_factory: LogFactory<'static, StdLogger>,
}

unsafe impl<'a, A, S> Send for Initiator<'a, A, S>
where
    A: ApplicationCallback,
    S: FfiMessageStoreFactory,
{
}

unsafe impl<'a, A, S> Sync for Initiator<'a, A, S>
where
    A: ApplicationCallback,
    S: FfiMessageStoreFactory,
{
}

impl<'a, A, S> Initiator<'a, A, S>
where
    A: ApplicationCallback,
    S: FfiMessageStoreFactory,
{
    /// Try create new struct from its mandatory components.
    pub fn try_new(
        settings: &SessionSettings,
        application: &'a Application<A>,
        store_factory: &'a S,
        server_mode: FixSocketServerKind,
    ) -> Result<Self, QuickFixError> {
        let log_factory = LogFactory::try_new(&StdLogger::Stdout)?;

        match unsafe {
            FixInitiator_new(
                application.0,
                store_factory.as_ffi_ptr(),
                settings.0,
                log_factory.0,
                server_mode.is_multi_threaded() as i8,
                server_mode.is_ssl_enabled() as i8,
            )
        } {
            Some(inner) => Ok(Self {
                inner,
                phantom_application: PhantomData,
                phantom_message_store_factory: PhantomData,
                _log_factory: log_factory,
            }),
            None => Err(QuickFixError::from_last_error()),
        }
    }
}

impl<A, S> ConnectionHandler for Initiator<'_, A, S>
where
    A: ApplicationCallback,
    S: FfiMessageStoreFactory,
{
    fn start(&mut self) -> Result<(), QuickFixError> {
        ffi_code_to_result(unsafe { FixInitiator_start(self.inner) })
    }

    fn block(&mut self) -> Result<(), QuickFixError> {
        ffi_code_to_result(unsafe { FixInitiator_block(self.inner) })
    }

    fn poll(&mut self) -> Result<bool, QuickFixError> {
        ffi_code_to_bool(unsafe { FixInitiator_poll(self.inner) })
    }

    fn stop(&mut self) -> Result<(), QuickFixError> {
        ffi_code_to_result(unsafe { FixInitiator_stop(self.inner) })
    }

    fn is_logged_on(&self) -> Result<bool, QuickFixError> {
        ffi_code_to_bool(unsafe { FixInitiator_isLoggedOn(self.inner) })
    }

    fn is_stopped(&self) -> Result<bool, QuickFixError> {
        ffi_code_to_bool(unsafe { FixInitiator_isStopped(self.inner) })
    }
}

impl<A, S> SessionContainer for Initiator<'_, A, S>
where
    A: ApplicationCallback,
    S: FfiMessageStoreFactory,
{
    fn session(&self, session_id: SessionId) -> Result<Session<'_>, QuickFixError> {
        unsafe {
            FixInitiator_getSession(self.inner, session_id.0)
                .map(|inner| Session {
                    inner,
                    phantom_container: PhantomData,
                })
                .ok_or_else(|| {
                    QuickFixError::SessionNotFound(format!("No session found: {session_id:?}"))
                })
        }
    }
}

impl<A, S> Drop for Initiator<'_, A, S>
where
    A: ApplicationCallback,
    S: FfiMessageStoreFactory,
{
    fn drop(&mut self) {
        let _ = self.stop();
        unsafe { FixInitiator_delete(self.inner) }
    }
}
