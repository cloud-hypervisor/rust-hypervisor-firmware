use atomic_refcell::AtomicRefCell;
use x86_64::{
    registers::control::Cr3,
    structures::paging::{PageSize, PageTable, PageTableFlags, PhysFrame, Size2MiB},
    PhysAddr,
};

// Keep in sync with address_space_gib in layout.ld
const ADDRESS_SPACE_GIB: usize = 4;

pub static MANAGER: AtomicRefCell<Manager> = AtomicRefCell::new(Manager);
pub struct Manager;

extern "C" {
    static mut pml4t: PageTable;
    static mut pml3t: PageTable;
    static mut pml2ts: [PageTable; ADDRESS_SPACE_GIB];
}

struct Tables<'a> {
    l4: &'a mut PageTable,
    l3: &'a mut PageTable,
    l2s: &'a mut [PageTable],
}

impl Manager {
    fn tables(&mut self) -> Tables {
        Tables {
            l4: unsafe { &mut pml4t },
            l3: unsafe { &mut pml3t },
            l2s: unsafe { &mut pml2ts },
        }
    }

    pub fn setup(&mut self) {
        log!("Setting up {} GiB identity mapping", ADDRESS_SPACE_GIB);
        let Tables { l4, l3, l2s } = self.tables();

        let pt_flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        // Setup Identity map using L2 huge pages
        let mut next_addr = PhysAddr::new(0);
        for l2 in l2s.iter_mut() {
            for l2e in l2.iter_mut() {
                l2e.set_addr(next_addr, pt_flags | PageTableFlags::HUGE_PAGE);
                next_addr += Size2MiB::SIZE;
            }
        }

        // Point L3 at L2s
        for (i, l2) in l2s.iter().enumerate() {
            l3[i].set_addr(phys_addr(l2), pt_flags);
        }

        // Point L4 at L3
        l4[0].set_addr(phys_addr(l3), pt_flags);

        // Point Cr3 at PML4
        let cr3_flags = Cr3::read().1;
        let pml4t_frame = PhysFrame::from_start_address(phys_addr(l4)).unwrap();
        unsafe { Cr3::write(pml4t_frame, cr3_flags) };
        log!("Page tables setup");
    }
}

// Map a virtual address to a PhysAddr (assumes identity mapping)
fn phys_addr<T>(virt_addr: *const T) -> PhysAddr {
    PhysAddr::new(virt_addr as u64)
}
