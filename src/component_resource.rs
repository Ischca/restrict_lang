//! Synchronous Component resource and callable-resource handle semantics.
//!
//! This module is a runtime substrate for the schema-v2 contracts in
//! `component_selection_v2`. It is intentionally independent of Core Wasm
//! addresses, arena layouts, and function-table indexes. Integration into
//! Component Interface IR and generated Canonical ABI adapters is still a
//! separate step.
//!
//! A table-issued [`OwnHandle`] is affine at the Rust API: it is neither
//! `Copy` nor `Clone`. The table also rejects duplicated raw handles after the
//! first consuming operation. [`BorrowScope`] ties a borrow to one synchronous
//! dynamic call, and its RAII cleanup runs on normal return, guest error, or
//! unwind. An own invocation removes the handle before entering guest code and
//! retains the resource destructor in a guard, so that cleanup also happens
//! when the invocation unwinds.

use std::any::{type_name, Any, TypeId};
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::marker::PhantomData;
use std::num::NonZeroU32;
use std::ops::{Deref, DerefMut};

/// Execution modes understood by the v0.1 resource runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComponentInvocationKind {
    Synchronous,
    Asynchronous,
}

/// Fail-closed errors from the typed handle table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComponentHandleError {
    /// Async resources, callbacks, and ownership across suspension are v0.2.
    AsyncUnsupported,
    /// Zero is reserved as an invalid/sentinel handle.
    InvalidHandle(u32),
    /// This table never recycles a raw handle, preventing stale-handle ABA.
    HandleSpaceExhausted,
    /// The handle was never issued by this table or was already consumed.
    UnknownOrConsumedHandle(u32),
    /// A valid raw handle was lifted with the wrong resource/signature type.
    TypeMismatch {
        handle: u32,
        expected: &'static str,
        actual: &'static str,
    },
    /// An owned operation cannot consume a resource during an active borrow.
    ActiveBorrow { handle: u32, count: u32 },
    /// Defensive failure for an impossible number of leaked borrow scopes.
    BorrowCountOverflow(u32),
}

impl fmt::Display for ComponentHandleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AsyncUnsupported => formatter.write_str(
                "asynchronous Component resources and callables are outside canonical-v1-sync; use the v0.2 async ABI when available",
            ),
            Self::InvalidHandle(handle) => {
                write!(formatter, "invalid Component resource handle {handle}")
            }
            Self::HandleSpaceExhausted => {
                formatter.write_str("Component resource handle space is exhausted")
            }
            Self::UnknownOrConsumedHandle(handle) => write!(
                formatter,
                "Component resource handle {handle} is unknown or was already consumed"
            ),
            Self::TypeMismatch {
                handle,
                expected,
                actual,
            } => write!(
                formatter,
                "Component resource handle {handle} has type '{actual}', expected '{expected}'"
            ),
            Self::ActiveBorrow { handle, count } => write!(
                formatter,
                "Component resource handle {handle} cannot be consumed while {count} borrow scope(s) are active"
            ),
            Self::BorrowCountOverflow(handle) => write!(
                formatter,
                "Component resource handle {handle} exceeded its synchronous borrow count"
            ),
        }
    }
}

impl Error for ComponentHandleError {}

/// A typed affine ownership token for one table entry.
///
/// `try_from_raw` is safe because every table operation revalidates liveness
/// and the concrete Rust type. Reconstructing the same raw value twice cannot
/// cause a double drop: the first own operation removes the entry, and the
/// second receives [`ComponentHandleError::UnknownOrConsumedHandle`].
#[must_use = "an owned Component handle must be transferred, invoked, or dropped"]
pub struct OwnHandle<T: 'static> {
    raw: NonZeroU32,
    marker: PhantomData<T>,
}

impl<T: 'static> OwnHandle<T> {
    pub fn try_from_raw(raw: u32) -> Result<Self, ComponentHandleError> {
        let raw = NonZeroU32::new(raw).ok_or(ComponentHandleError::InvalidHandle(raw))?;
        Ok(Self {
            raw,
            marker: PhantomData,
        })
    }

    pub fn raw(&self) -> u32 {
        self.raw.get()
    }

    /// Transfer this typed ownership token to a Canonical ABI raw handle.
    pub fn into_raw(self) -> u32 {
        self.raw.get()
    }
}

impl<T: 'static> fmt::Debug for OwnHandle<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OwnHandle")
            .field("raw", &self.raw)
            .field("type", &type_name::<T>())
            .finish()
    }
}

impl<T: 'static> PartialEq for OwnHandle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.raw == other.raw
    }
}

impl<T: 'static> Eq for OwnHandle<T> {}

type ErasedDestructor = Box<dyn FnOnce(Box<dyn Any>)>;

struct Entry {
    value: Option<Box<dyn Any>>,
    destructor: Option<ErasedDestructor>,
    type_id: TypeId,
    type_name: &'static str,
    active_borrows: u32,
}

impl Entry {
    fn new<T, D>(value: T, destructor: D) -> Self
    where
        T: 'static,
        D: FnOnce(T) + 'static,
    {
        let erased_destructor = move |value: Box<dyn Any>| {
            let value = value.downcast::<T>().unwrap_or_else(|_| {
                panic!(
                    "typed Component handle table lost the value type '{}'",
                    type_name::<T>()
                )
            });
            destructor(*value);
        };
        Self {
            value: Some(Box::new(value)),
            destructor: Some(Box::new(erased_destructor)),
            type_id: TypeId::of::<T>(),
            type_name: type_name::<T>(),
            active_borrows: 0,
        }
    }

    fn ensure_type<T: 'static>(&self, handle: u32) -> Result<(), ComponentHandleError> {
        if self.type_id == TypeId::of::<T>() {
            Ok(())
        } else {
            Err(ComponentHandleError::TypeMismatch {
                handle,
                expected: type_name::<T>(),
                actual: self.type_name,
            })
        }
    }

    fn value<T: 'static>(&self) -> &T {
        self.value
            .as_deref()
            .and_then(<dyn Any>::downcast_ref::<T>)
            .expect("typed Component handle entry must retain its value")
    }

    fn value_mut<T: 'static>(&mut self) -> &mut T {
        self.value
            .as_deref_mut()
            .and_then(<dyn Any>::downcast_mut::<T>)
            .expect("typed Component handle entry must retain its value")
    }

    fn destroy(mut self) {
        let value = self
            .value
            .take()
            .expect("typed Component handle entry must retain its value");
        let destructor = self
            .destructor
            .take()
            .expect("typed Component handle entry must retain its destructor");
        destructor(value);
    }
}

/// Heterogeneous table of signature-checked Component resource values.
///
/// Raw handles are monotonically allocated and never reused during a table's
/// lifetime. This deliberately trades a bounded `u32` lifetime allocation
/// space for stale-handle safety: an old raw handle can never alias a newer
/// resource. Handle values and Rust type names are runtime diagnostics only;
/// neither belongs in normalized WIT or stable ABI metadata.
pub struct TypedHandleTable {
    entries: BTreeMap<NonZeroU32, Entry>,
    next_handle: Option<NonZeroU32>,
}

impl TypedHandleTable {
    /// Construct a table only for the synchronous v0.1 ABI.
    pub fn new(kind: ComponentInvocationKind) -> Result<Self, ComponentHandleError> {
        match kind {
            ComponentInvocationKind::Synchronous => Ok(Self::synchronous()),
            ComponentInvocationKind::Asynchronous => Err(ComponentHandleError::AsyncUnsupported),
        }
    }

    pub fn synchronous() -> Self {
        Self {
            entries: BTreeMap::new(),
            next_handle: NonZeroU32::new(1),
        }
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Insert a resource representation and its exactly-once destructor.
    pub fn insert_resource<T, D>(
        &mut self,
        value: T,
        destructor: D,
    ) -> Result<OwnHandle<T>, ComponentHandleError>
    where
        T: 'static,
        D: FnOnce(T) + 'static,
    {
        let raw = self
            .next_handle
            .ok_or(ComponentHandleError::HandleSpaceExhausted)?;
        self.next_handle = raw.get().checked_add(1).and_then(NonZeroU32::new);
        let previous = self.entries.insert(raw, Entry::new(value, destructor));
        debug_assert!(previous.is_none(), "monotonic handles cannot collide");
        Ok(OwnHandle {
            raw,
            marker: PhantomData,
        })
    }

    /// Invoke a synchronous method with a call-scoped borrowed receiver.
    pub fn invoke_borrow<T, R>(
        &mut self,
        handle: &OwnHandle<T>,
        operation: impl FnOnce(&mut T) -> R,
    ) -> Result<R, ComponentHandleError>
    where
        T: 'static,
    {
        self.invoke_borrow_raw(handle.raw(), operation)
    }

    /// Lift and invoke a raw `borrow<T>` receiver for one synchronous call.
    pub fn invoke_borrow_raw<T, R>(
        &mut self,
        raw: u32,
        operation: impl FnOnce(&mut T) -> R,
    ) -> Result<R, ComponentHandleError>
    where
        T: 'static,
    {
        let mut scope: BorrowScope<'_, T> = self.borrow_raw(raw)?;
        Ok(operation(scope.get_mut()))
    }

    /// Open an explicit synchronous borrow scope.
    pub fn borrow<T: 'static>(
        &mut self,
        handle: &OwnHandle<T>,
    ) -> Result<BorrowScope<'_, T>, ComponentHandleError> {
        self.borrow_raw(handle.raw())
    }

    /// Open an explicit synchronous borrow scope from a Canonical ABI handle.
    pub fn borrow_raw<T: 'static>(
        &mut self,
        raw: u32,
    ) -> Result<BorrowScope<'_, T>, ComponentHandleError> {
        let raw = parse_raw_handle(raw)?;
        {
            let entry = self.entry_mut::<T>(raw)?;
            entry.active_borrows = entry
                .active_borrows
                .checked_add(1)
                .ok_or(ComponentHandleError::BorrowCountOverflow(raw.get()))?;
        }
        Ok(BorrowScope {
            table: self,
            raw,
            marker: PhantomData,
        })
    }

    /// Invoke an owning receiver. The handle is removed before guest code is
    /// entered, and its destructor is guarded across normal return and unwind.
    pub fn invoke_own<T, R>(
        &mut self,
        handle: OwnHandle<T>,
        operation: impl FnOnce(&mut T) -> R,
    ) -> Result<R, ComponentHandleError>
    where
        T: 'static,
    {
        self.invoke_own_raw(handle.into_raw(), operation)
    }

    /// Lift and consume a raw `own<T>` receiver.
    pub fn invoke_own_raw<T, R>(
        &mut self,
        raw: u32,
        operation: impl FnOnce(&mut T) -> R,
    ) -> Result<R, ComponentHandleError>
    where
        T: 'static,
    {
        let raw = parse_raw_handle(raw)?;
        let entry = self.remove_owned_entry::<T>(raw)?;
        let mut guard = OwnedEntryGuard { entry: Some(entry) };
        let result = operation(guard.value_mut::<T>());
        drop(guard);
        Ok(result)
    }

    /// Explicitly drop one owned resource.
    pub fn drop_own<T: 'static>(
        &mut self,
        handle: OwnHandle<T>,
    ) -> Result<(), ComponentHandleError> {
        self.drop_own_raw::<T>(handle.into_raw())
    }

    /// Lift and explicitly drop one raw owned resource handle.
    pub fn drop_own_raw<T: 'static>(&mut self, raw: u32) -> Result<(), ComponentHandleError> {
        let raw = parse_raw_handle(raw)?;
        self.remove_owned_entry::<T>(raw)?.destroy();
        Ok(())
    }

    fn entry_mut<T: 'static>(
        &mut self,
        raw: NonZeroU32,
    ) -> Result<&mut Entry, ComponentHandleError> {
        let entry = self
            .entries
            .get_mut(&raw)
            .ok_or(ComponentHandleError::UnknownOrConsumedHandle(raw.get()))?;
        entry.ensure_type::<T>(raw.get())?;
        Ok(entry)
    }

    fn remove_owned_entry<T: 'static>(
        &mut self,
        raw: NonZeroU32,
    ) -> Result<Entry, ComponentHandleError> {
        let entry = self
            .entries
            .get(&raw)
            .ok_or(ComponentHandleError::UnknownOrConsumedHandle(raw.get()))?;
        entry.ensure_type::<T>(raw.get())?;
        if entry.active_borrows != 0 {
            return Err(ComponentHandleError::ActiveBorrow {
                handle: raw.get(),
                count: entry.active_borrows,
            });
        }
        Ok(self
            .entries
            .remove(&raw)
            .expect("validated Component handle must remain present"))
    }
}

impl Default for TypedHandleTable {
    fn default() -> Self {
        Self::synchronous()
    }
}

impl Drop for TypedHandleTable {
    fn drop(&mut self) {
        // Reverse allocation order makes implicit teardown deterministic.
        while let Some((_raw, entry)) = self.entries.pop_last() {
            entry.destroy();
        }
    }
}

fn parse_raw_handle(raw: u32) -> Result<NonZeroU32, ComponentHandleError> {
    NonZeroU32::new(raw).ok_or(ComponentHandleError::InvalidHandle(raw))
}

/// A non-escaping synchronous `borrow<T>` scope.
#[must_use = "a Component resource borrow must remain live for its call scope"]
pub struct BorrowScope<'table, T: 'static> {
    table: &'table mut TypedHandleTable,
    raw: NonZeroU32,
    marker: PhantomData<&'table mut T>,
}

impl<T: 'static> BorrowScope<'_, T> {
    pub fn get(&self) -> &T {
        self.table
            .entries
            .get(&self.raw)
            .expect("borrowed Component handle must remain present")
            .value::<T>()
    }

    pub fn raw(&self) -> u32 {
        self.raw.get()
    }

    /// WIT `borrow<T>` borrows ownership, not constness. A synchronous method
    /// may mutate resource state while the handle itself remains owned by its
    /// caller.
    pub fn get_mut(&mut self) -> &mut T {
        self.table
            .entries
            .get_mut(&self.raw)
            .expect("borrowed Component handle must remain present")
            .value_mut::<T>()
    }
}

impl<T: 'static> Deref for BorrowScope<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.get()
    }
}

impl<T: 'static> DerefMut for BorrowScope<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.get_mut()
    }
}

impl<T: 'static> Drop for BorrowScope<'_, T> {
    fn drop(&mut self) {
        let entry = self
            .table
            .entries
            .get_mut(&self.raw)
            .expect("borrowed Component handle must remain present");
        entry.active_borrows = entry
            .active_borrows
            .checked_sub(1)
            .expect("Component borrow count must be balanced");
    }
}

struct OwnedEntryGuard {
    entry: Option<Entry>,
}

impl OwnedEntryGuard {
    fn value_mut<T: 'static>(&mut self) -> &mut T {
        self.entry
            .as_mut()
            .expect("owned Component entry guard must be live")
            .value_mut::<T>()
    }
}

impl Drop for OwnedEntryGuard {
    fn drop(&mut self) {
        if let Some(entry) = self.entry.take() {
            entry.destroy();
        }
    }
}

struct OnceCallable<Args, Output> {
    callback: Option<Box<dyn FnOnce(Args) -> Output>>,
}

/// A signature-specific affine handle for a `multiplicity = "once"`
/// callable resource.
#[must_use = "an affine callable must be called, transferred, or dropped"]
pub struct CallableHandle<Args: 'static, Output: 'static> {
    inner: OwnHandle<OnceCallable<Args, Output>>,
}

impl<Args: 'static, Output: 'static> CallableHandle<Args, Output> {
    pub fn try_from_raw(raw: u32) -> Result<Self, ComponentHandleError> {
        Ok(Self {
            inner: OwnHandle::try_from_raw(raw)?,
        })
    }

    pub fn raw(&self) -> u32 {
        self.inner.raw()
    }

    pub fn into_raw(self) -> u32 {
        self.inner.into_raw()
    }
}

impl<Args: 'static, Output: 'static> fmt::Debug for CallableHandle<Args, Output> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CallableHandle")
            .field("raw", &self.raw())
            .field("params", &type_name::<Args>())
            .field("result", &type_name::<Output>())
            .finish()
    }
}

impl TypedHandleTable {
    /// Insert one signature-specific callable resource. Its closure owns all
    /// captures. Calling or dropping the resource releases those captures once.
    pub fn insert_callable_once<Args, Output, Callback>(
        &mut self,
        callback: Callback,
    ) -> Result<CallableHandle<Args, Output>, ComponentHandleError>
    where
        Args: 'static,
        Output: 'static,
        Callback: FnOnce(Args) -> Output + 'static,
    {
        let callable = OnceCallable {
            callback: Some(Box::new(callback)),
        };
        let inner = self.insert_resource(callable, drop)?;
        Ok(CallableHandle { inner })
    }

    /// Consume and synchronously call an affine callable resource.
    pub fn call_once<Args, Output>(
        &mut self,
        handle: CallableHandle<Args, Output>,
        args: Args,
    ) -> Result<Output, ComponentHandleError>
    where
        Args: 'static,
        Output: 'static,
    {
        self.invoke_own(handle.inner, |callable| {
            let callback = callable
                .callback
                .take()
                .expect("live affine callable must retain its callback");
            callback(args)
        })
    }

    /// Lift and consume a raw callable-resource handle.
    pub fn call_once_raw<Args, Output>(
        &mut self,
        raw: u32,
        args: Args,
    ) -> Result<Output, ComponentHandleError>
    where
        Args: 'static,
        Output: 'static,
    {
        self.call_once(CallableHandle::try_from_raw(raw)?, args)
    }

    /// Drop an uncalled callable and its affine captures.
    pub fn drop_callable<Args, Output>(
        &mut self,
        handle: CallableHandle<Args, Output>,
    ) -> Result<(), ComponentHandleError>
    where
        Args: 'static,
        Output: 'static,
    {
        self.drop_own(handle.inner)
    }

    /// Lift and drop a raw uncalled callable-resource handle.
    pub fn drop_callable_raw<Args, Output>(&mut self, raw: u32) -> Result<(), ComponentHandleError>
    where
        Args: 'static,
        Output: 'static,
    {
        self.drop_callable(CallableHandle::<Args, Output>::try_from_raw(raw)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::panic::{catch_unwind, AssertUnwindSafe};
    use std::rc::Rc;

    #[test]
    fn borrow_survives_guest_error_and_unwind_then_owned_drop_is_exactly_once() {
        let drops = Rc::new(Cell::new(0));
        let dropped_value = Rc::new(RefCell::new(String::new()));
        let mut table = TypedHandleTable::synchronous();
        let handle = table
            .insert_resource("file".to_string(), {
                let drops = Rc::clone(&drops);
                let dropped_value = Rc::clone(&dropped_value);
                move |value| {
                    drops.set(drops.get() + 1);
                    *dropped_value.borrow_mut() = value;
                }
            })
            .unwrap();

        let guest_result = table
            .invoke_borrow(&handle, |value| {
                assert_eq!(value, "file");
                value.push_str(":read");
                Err::<(), _>("guest trap")
            })
            .unwrap();
        assert_eq!(guest_result, Err("guest trap"));

        let unwind = catch_unwind(AssertUnwindSafe(|| {
            let _ = table.invoke_borrow(&handle, |_value| -> () {
                panic!("synthetic guest unwind");
            });
        }));
        assert!(unwind.is_err());

        let raw = handle.raw();
        table.drop_own(handle).unwrap();
        assert_eq!(drops.get(), 1);
        assert_eq!(&*dropped_value.borrow(), "file:read");
        assert!(matches!(
            table.drop_own_raw::<String>(raw),
            Err(ComponentHandleError::UnknownOrConsumedHandle(value)) if value == raw
        ));
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn own_invocation_consumes_before_entry_and_cleans_up_on_unwind() {
        let drops = Rc::new(Cell::new(0));
        let mut table = TypedHandleTable::synchronous();
        let handle = table
            .insert_resource(41_i32, {
                let drops = Rc::clone(&drops);
                move |_value| drops.set(drops.get() + 1)
            })
            .unwrap();
        let raw = handle.raw();

        let unwind = catch_unwind(AssertUnwindSafe(|| {
            let _ = table.invoke_own(handle, |value| -> () {
                *value += 1;
                panic!("synthetic owning trap");
            });
        }));
        assert!(unwind.is_err());
        assert_eq!(drops.get(), 1);
        assert!(matches!(
            table.invoke_own_raw::<i32, _>(raw, |_| ()),
            Err(ComponentHandleError::UnknownOrConsumedHandle(value)) if value == raw
        ));
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn raw_handle_type_mismatch_does_not_consume_the_resource() {
        let drops = Rc::new(Cell::new(0));
        let mut table = TypedHandleTable::synchronous();
        let handle = table
            .insert_resource("typed".to_string(), {
                let drops = Rc::clone(&drops);
                move |_value| drops.set(drops.get() + 1)
            })
            .unwrap();
        let raw = handle.into_raw();

        assert!(matches!(
            table.drop_own_raw::<u64>(raw),
            Err(ComponentHandleError::TypeMismatch { handle, .. }) if handle == raw
        ));
        assert_eq!(drops.get(), 0);
        table.drop_own_raw::<String>(raw).unwrap();
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn leaked_dynamic_borrow_blocks_consumption_but_table_teardown_still_drops() {
        let drops = Rc::new(Cell::new(0));
        let mut table = TypedHandleTable::synchronous();
        let handle = table
            .insert_resource(7_i32, {
                let drops = Rc::clone(&drops);
                move |_value| drops.set(drops.get() + 1)
            })
            .unwrap();
        let raw = handle.raw();
        let borrow = table.borrow(&handle).unwrap();
        std::mem::forget(borrow);

        assert!(matches!(
            table.drop_own(handle),
            Err(ComponentHandleError::ActiveBorrow { handle, count: 1 }) if handle == raw
        ));
        assert_eq!(drops.get(), 0);
        drop(table);
        assert_eq!(drops.get(), 1);
    }

    #[test]
    fn table_teardown_drops_live_resources_once_in_reverse_allocation_order() {
        let order = Rc::new(RefCell::new(Vec::new()));
        {
            let mut table = TypedHandleTable::synchronous();
            for value in [1, 2, 3] {
                let _handle = table
                    .insert_resource(value, {
                        let order = Rc::clone(&order);
                        move |value| order.borrow_mut().push(value)
                    })
                    .unwrap();
            }
            assert_eq!(table.len(), 3);
        }
        assert_eq!(&*order.borrow(), &[3, 2, 1]);
    }

    struct CaptureProbe(Rc<Cell<u32>>);

    impl Drop for CaptureProbe {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[test]
    fn callable_is_signature_typed_called_once_and_drops_capture_once() {
        let capture_drops = Rc::new(Cell::new(0));
        let probe = CaptureProbe(Rc::clone(&capture_drops));
        let mut table = TypedHandleTable::synchronous();
        let callable = table
            .insert_callable_once(move |(value,): (i32,)| {
                let _capture = probe;
                value + 1
            })
            .unwrap();
        let raw = callable.into_raw();

        let wrong = CallableHandle::<(String,), String>::try_from_raw(raw).unwrap();
        assert!(matches!(
            table.call_once(wrong, ("wrong".to_string(),)),
            Err(ComponentHandleError::TypeMismatch { handle, .. }) if handle == raw
        ));
        assert_eq!(capture_drops.get(), 0);

        assert_eq!(table.call_once_raw::<(i32,), i32>(raw, (41,)).unwrap(), 42);
        assert_eq!(capture_drops.get(), 1);
        assert!(matches!(
            table.call_once_raw::<(i32,), i32>(raw, (0,)),
            Err(ComponentHandleError::UnknownOrConsumedHandle(handle)) if handle == raw
        ));
        assert_eq!(capture_drops.get(), 1);
    }

    #[test]
    fn dropping_uncalled_callable_releases_affine_capture_once() {
        let capture_drops = Rc::new(Cell::new(0));
        let probe = CaptureProbe(Rc::clone(&capture_drops));
        let mut table = TypedHandleTable::synchronous();
        let callable = table
            .insert_callable_once(move |(): ()| {
                let _capture = probe;
            })
            .unwrap();
        let raw = callable.raw();

        table.drop_callable(callable).unwrap();
        assert_eq!(capture_drops.get(), 1);
        assert!(matches!(
            table.drop_callable_raw::<(), ()>(raw),
            Err(ComponentHandleError::UnknownOrConsumedHandle(handle)) if handle == raw
        ));
        assert_eq!(capture_drops.get(), 1);
    }

    #[test]
    fn callable_trap_consumes_handle_and_releases_capture_once() {
        let capture_drops = Rc::new(Cell::new(0));
        let probe = CaptureProbe(Rc::clone(&capture_drops));
        let mut table = TypedHandleTable::synchronous();
        let callable = table
            .insert_callable_once(move |(): ()| -> () {
                let _capture = probe;
                panic!("synthetic callable trap");
            })
            .unwrap();
        let raw = callable.raw();

        let unwind = catch_unwind(AssertUnwindSafe(|| {
            let _ = table.call_once(callable, ());
        }));
        assert!(unwind.is_err());
        assert_eq!(capture_drops.get(), 1);
        assert!(matches!(
            table.call_once_raw::<(), ()>(raw, ()),
            Err(ComponentHandleError::UnknownOrConsumedHandle(handle)) if handle == raw
        ));
    }

    #[test]
    fn async_mode_is_rejected_for_v0_1() {
        assert!(matches!(
            TypedHandleTable::new(ComponentInvocationKind::Asynchronous),
            Err(ComponentHandleError::AsyncUnsupported)
        ));
        TypedHandleTable::new(ComponentInvocationKind::Synchronous).unwrap();
    }
}
