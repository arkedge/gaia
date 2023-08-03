use anyhow::Result;
use async_trait::async_trait;
use futures::future::BoxFuture;

#[async_trait]
pub trait Handle<Request> {
    type Response;
    async fn handle(&mut self, request: Request) -> Result<Self::Response>;
}

pub trait Layer<H> {
    type Handle;
    fn layer(self, handle: H) -> Self::Handle;
}

#[derive(Clone, Debug)]
pub struct BeforeHookLayer<K> {
    hook: K,
}

impl<K> BeforeHookLayer<K> {
    pub fn new(hook: K) -> Self {
        Self { hook }
    }
}

impl<K, H> Layer<H> for BeforeHookLayer<K> {
    type Handle = BeforeHook<H, K>;

    fn layer(self, inner: H) -> Self::Handle {
        BeforeHook {
            hook: self.hook,
            inner,
        }
    }
}

#[derive(Clone, Debug)]
pub struct BeforeHook<H, K> {
    hook: K,
    inner: H,
}

#[async_trait]
impl<Q, H, K> Handle<Q> for BeforeHook<H, K>
where
    Q: Send + 'static,
    H: Handle<K::Output> + Send,
    K: Hook<Q> + Send,
    K::Output: Send,
{
    type Response = H::Response;

    async fn handle(&mut self, request: Q) -> Result<Self::Response> {
        let next_request = self.hook.hook(request).await?;
        self.inner.handle(next_request).await
    }
}

#[async_trait]
pub trait Hook<Input> {
    type Output;
    async fn hook(&mut self, input: Input) -> Result<Self::Output>;
}

#[derive(Clone, Debug)]
pub struct Choice<X, Y> {
    first: X,
    second: Y,
}

#[async_trait]
impl<Q, S, X, Y> Handle<Q> for Choice<X, Y>
where
    Q: Clone + Send + Sync + 'static,
    X: Handle<Q, Response = Option<S>> + Send,
    Y: Handle<Q, Response = Option<S>> + Send,
{
    type Response = Option<S>;

    async fn handle(&mut self, request: Q) -> Result<Self::Response> {
        if let Some(ret) = self.first.handle(request.clone()).await? {
            return Ok(Some(ret));
        }
        self.second.handle(request).await
    }
}

pub trait HandleChoiceExt<Q>: Handle<Q> {
    fn prepend<X>(self, first: X) -> Choice<X, Self>
    where
        Self: Sized,
    {
        Choice {
            first,
            second: self,
        }
    }

    fn append<Y>(self, second: Y) -> Choice<Self, Y>
    where
        Self: Sized,
    {
        Choice {
            first: self,
            second,
        }
    }
}

impl<T: ?Sized, Q> HandleChoiceExt<Q> for T where T: Handle<Q> {}

#[derive(Debug, Clone, Default)]
pub struct Identity;

impl<H> Layer<H> for Identity {
    type Handle = H;

    fn layer(self, inner: H) -> Self::Handle {
        inner
    }
}

#[derive(Clone, Debug)]
pub enum Either<A, B> {
    A(A),
    B(B),
}

impl<A, B, H> Layer<H> for Either<A, B>
where
    A: Layer<H>,
    B: Layer<H>,
{
    type Handle = Either<A::Handle, B::Handle>;

    fn layer(self, inner: H) -> Self::Handle {
        match self {
            Either::A(a) => Either::A(a.layer(inner)),
            Either::B(b) => Either::B(b.layer(inner)),
        }
    }
}

impl<A, B, Q> Handle<Q> for Either<A, B>
where
    A: Handle<Q>,
    B: Handle<Q, Response = A::Response>,
{
    type Response = A::Response;

    fn handle<'a: 's, 's>(&'a mut self, request: Q) -> BoxFuture<'s, Result<Self::Response>>
    where
        Self: 's,
    {
        match self {
            Either::A(a) => a.handle(request),
            Either::B(b) => b.handle(request),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Stack<I, O> {
    inner: I,
    outer: O,
}

impl<I, O> Stack<I, O> {
    pub fn new(inner: I, outer: O) -> Stack<I, O> {
        Self { inner, outer }
    }
}

impl<I, O, H> Layer<H> for Stack<I, O>
where
    I: Layer<H>,
    O: Layer<I::Handle>,
{
    type Handle = O::Handle;

    fn layer(self, handle: H) -> Self::Handle {
        self.outer.layer(self.inner.layer(handle))
    }
}

#[derive(Clone, Debug)]
pub struct Builder<L> {
    layer: L,
}

impl Builder<Identity> {
    pub fn new() -> Self {
        Self { layer: Identity }
    }
}

impl Default for Builder<Identity> {
    fn default() -> Self {
        Self::new()
    }
}

impl<L> Builder<L> {
    pub fn layer<I>(self, layer: I) -> Builder<Stack<I, L>> {
        Builder {
            layer: Stack::new(layer, self.layer),
        }
    }

    pub fn option_layer<I>(self, layer: Option<I>) -> Builder<Stack<Either<I, Identity>, L>> {
        let inner = match layer {
            Some(layer) => Either::A(layer),
            None => Either::B(Identity),
        };
        Builder {
            layer: Stack::new(inner, self.layer),
        }
    }

    pub fn before_hook<K>(self, hook: K) -> Builder<Stack<BeforeHookLayer<K>, L>> {
        let before_hook = BeforeHookLayer::new(hook);
        self.layer(before_hook)
    }

    pub fn build<H>(self, handle: H) -> L::Handle
    where
        L: Layer<H>,
    {
        self.layer.layer(handle)
    }
}
