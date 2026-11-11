// Hand-written additions, `include!`d into the generated `bindings` module so they can reach its private items.
//
// windows-bindgen 0.100 emits `TypedEventHandlerBox` (the vtable) for the generic `TypedEventHandler<TSender, TResult>` delegate but
// not its `new` constructor, which every `TypedEventHandler`-based event wrapper (`CharacterReceived`, `CompositionScaleChanged`)
// needs. This mirrors what it generates for non-generic delegates such as `KeyEventHandler::new`.

impl<TSender: windows_core::RuntimeType + 'static, TResult: windows_core::RuntimeType + 'static> TypedEventHandler<TSender, TResult> {
    pub fn new<F: Fn(windows_core::Ref<TSender>, windows_core::Ref<TResult>) -> windows_core::Result<()> + Send + 'static>(invoke: F) -> Self {
        let com = windows_core::imp::DelegateBox::<Self, F>::new(&TypedEventHandlerBox::<TSender, TResult, F>::VTABLE, invoke);
        unsafe { core::mem::transmute(windows_core::imp::box_new(com)) }
    }
}
