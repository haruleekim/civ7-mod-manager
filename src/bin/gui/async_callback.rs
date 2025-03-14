use std::{pin::Pin, sync::Arc};

#[derive(Clone)]
pub struct AsyncCallback<Args = (), Ret = ()>(
    Arc<Box<dyn Fn(Args) -> Pin<Box<dyn Future<Output = Ret> + 'static>> + 'static>>,
);

impl<Args, Ret> AsyncCallback<Args, Ret> {
    pub fn new<Fut, F>(f: F) -> Self
    where
        Fut: Future<Output = Ret> + 'static,
        F: Fn(Args) -> Fut + 'static,
    {
        AsyncCallback(Arc::new(Box::new(move |args| Box::pin(f(args)))))
    }

    pub fn call(&self, args: Args) -> Pin<Box<dyn Future<Output = Ret>>> {
        self.0(args)
    }
}

impl<Args, Ret> PartialEq for AsyncCallback<Args, Ret> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl<Args, Ret, Fut, F> From<F> for AsyncCallback<Args, Ret>
where
    Fut: Future<Output = Ret> + 'static,
    F: Fn(Args) -> Fut + 'static,
{
    #[inline(always)]
    fn from(f: F) -> Self {
        Self::new(f)
    }
}
