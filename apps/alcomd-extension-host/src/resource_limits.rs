//! One guest table plus the Component encoder's canonical-call trampoline table.
//! Their combined element budget remains the frozen runtime quota.
use alcomd_extensions::RuntimeLimits;
use wasmtime::{ResourceLimiter, StoreLimits, StoreLimitsBuilder};

pub(super) struct HostLimits {
    inner: StoreLimits,
    table_elements: usize,
    reserved_elements: usize,
    pending_growth: usize,
}
impl HostLimits {
    pub fn new(limits: &RuntimeLimits) -> anyhow::Result<Self> {
        let table_elements = usize::try_from(limits.table_elements)?;
        Ok(Self {
            inner: StoreLimitsBuilder::new()
                .memory_size(usize::try_from(limits.linear_memory_bytes)?)
                .table_elements(table_elements)
                .instances(16)
                .memories(1)
                .tables(2)
                .trap_on_grow_failure(true)
                .build(),
            table_elements,
            reserved_elements: 0,
            pending_growth: 0,
        })
    }
}
impl ResourceLimiter for HostLimits {
    fn memory_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        self.inner.memory_growing(current, desired, maximum)
    }
    fn memory_grow_failed(&mut self, error: wasmtime::Error) -> wasmtime::Result<()> {
        self.inner.memory_grow_failed(error)
    }
    fn table_growing(
        &mut self,
        current: usize,
        desired: usize,
        maximum: Option<usize>,
    ) -> wasmtime::Result<bool> {
        self.pending_growth = 0;
        let delta = desired
            .checked_sub(current)
            .ok_or_else(|| wasmtime::Error::msg("invalid table growth"))?;
        let total = self
            .reserved_elements
            .checked_add(delta)
            .filter(|total| *total <= self.table_elements)
            .ok_or_else(|| wasmtime::Error::new(wasmtime::Trap::AllocationTooLarge))?;
        let allowed = self.inner.table_growing(current, desired, maximum)?;
        if allowed {
            self.reserved_elements = total;
            self.pending_growth = delta;
        }
        Ok(allowed)
    }
    fn table_grow_failed(&mut self, error: wasmtime::Error) -> wasmtime::Result<()> {
        self.reserved_elements -= self.pending_growth;
        self.pending_growth = 0;
        self.inner.table_grow_failed(error)
    }
    fn instances(&self) -> usize {
        self.inner.instances()
    }
    fn memories(&self) -> usize {
        self.inner.memories()
    }
    fn tables(&self) -> usize {
        self.inner.tables()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn two_core_tables_share_one_element_quota_and_original_memory_bound() {
        let mut limits = HostLimits::new(&RuntimeLimits::default()).expect("limits");
        assert_eq!(limits.tables(), 2);
        assert_eq!(limits.instances(), 16);
        assert_eq!(limits.memories(), 1);
        assert!(limits.table_growing(0, 6000, None).expect("first table"));
        assert!(limits.table_growing(0, 4000, None).expect("second table"));
        assert!(limits.table_growing(4000, 4001, None).is_err());
        assert_eq!(limits.reserved_elements, 10000);
        assert!(
            limits
                .memory_growing(0, 64 * 1024 * 1024, None)
                .expect("original memory quota")
        );
        assert!(
            limits
                .memory_growing(64 * 1024 * 1024, 64 * 1024 * 1024 + 65536, None)
                .is_err()
        );
    }
    #[test]
    fn failed_growth_releases_only_its_own_reservation() {
        let mut limits = HostLimits::new(&RuntimeLimits::default()).expect("limits");
        assert!(limits.table_growing(0, 6000, None).expect("first table"));
        assert!(
            limits
                .table_growing(0, 4000, None)
                .expect("permitted allocation")
        );
        assert!(
            limits
                .table_grow_failed(wasmtime::Error::msg("controlled allocation failure"))
                .is_err()
        );
        assert_eq!(limits.reserved_elements, 6000);
        assert!(limits.table_growing(0, 4000, None).expect("quota restored"));
        assert!(limits.table_growing(6000, 6001, None).is_err());
    }
}
