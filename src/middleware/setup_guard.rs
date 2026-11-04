use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::task::{Context, Poll};

use actix_web::{
    body::{BoxBody, MessageBody},
    dev::{Service, ServiceRequest, ServiceResponse, Transform},
    Error, HttpResponse,
};
use futures::future::{ok, Ready};

pub struct SetupGuard {
    initialized: Arc<AtomicBool>,
    force_setup: bool,
}

impl SetupGuard {
    pub fn new(initialized: Arc<AtomicBool>, force_setup: bool) -> Self {
        Self {
            initialized,
            force_setup,
        }
    }
}

impl<S, B> Transform<S, ServiceRequest> for SetupGuard
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type Transform = SetupGuardMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ok(SetupGuardMiddleware {
            service,
            initialized: self.initialized.clone(),
            force_setup: self.force_setup,
        })
    }
}

pub struct SetupGuardMiddleware<S> {
    service: S,
    initialized: Arc<AtomicBool>,
    force_setup: bool,
}

impl<S, B> Service<ServiceRequest> for SetupGuardMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error>,
    S::Future: 'static,
    B: MessageBody + 'static,
{
    type Response = ServiceResponse<BoxBody>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(&self, ctx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        let path = req.path().to_string();
        let allow = path.starts_with("/api/setup")
            || path == "/api/health"
            || !self.force_setup
            || self.initialized.load(Ordering::Relaxed);

        if allow {
            let fut = self.service.call(req);
            Box::pin(async move {
                let res = fut.await?;
                Ok(res.map_into_boxed_body())
            })
        } else {
            Box::pin(async move {
                let res = HttpResponse::Forbidden().json(serde_json::json!({
                    "code": 4030,
                    "message": "SETUP_REQUIRED"
                }));
                Ok(req.into_response(res))
            })
        }
    }

}