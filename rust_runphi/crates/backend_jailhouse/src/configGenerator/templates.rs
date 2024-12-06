//*********************************************
// Authors: Francesco Boccola (f.boccola@studenti.unina.it)
//*********************************************

use std::collections::HashMap;

pub const RAM_TEMPLATE: &'static str = r#"
/* RAM */ {
    .phys_start = {phys_start},
    .virt_start = {virt_start},
    .size = {size},
    .flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_WRITE |
        JAILHOUSE_MEM_EXECUTE | JAILHOUSE_MEM_DMA |
        JAILHOUSE_MEM_LOADABLE,
},
"#;

pub const RAM0_TEMPLATE: &'static str = r#"
/* RAM */ {
    .phys_start = {phys_start},
    .virt_start = {virt_start},
    .size = {size},
    .flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_WRITE |
        JAILHOUSE_MEM_EXECUTE | JAILHOUSE_MEM_LOADABLE,
},
"#;

pub const TCMA_TEMPLATE: &'static str = r#"
/* TCM 0-A */  {
    .phys_start = {phys_start},
    .virt_start = {virt_start},
    .size = {size},
    .flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_WRITE |
		JAILHOUSE_MEM_EXECUTE | JAILHOUSE_MEM_LOADABLE | 
		JAILHOUSE_MEM_TCM_A,
},
"#;

pub const TCMB_TEMPLATE: &'static str = r#"
/* TCM 0-B */  {
    .phys_start = {phys_start},
    .virt_start = {virt_start},
    .size = {size},
    .flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_WRITE |
		JAILHOUSE_MEM_EXECUTE | JAILHOUSE_MEM_LOADABLE |
		JAILHOUSE_MEM_TCM_B,
},
"#;

pub const UART_TEMPLATE: &'static str = r#"
/* UART */ {
    .phys_start = {phys_start},
    .virt_start = {virt_start},
    .size = {size},
    .flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_WRITE |
        JAILHOUSE_MEM_IO | JAILHOUSE_MEM_ROOTSHARED,
},
"#;

pub const COMM_REGION_TEMPLATE: &'static str = r#"
/* communication region */ {
    .virt_start = 0x80000000,
    .size = 0x00001000,
    .flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_WRITE |
        JAILHOUSE_MEM_COMM_REGION,
},
"#;

pub const IVSHMEM_DEMO_TEMPLATE: &'static str = r#"
/* IVSHMEM shared memory region for 00:00.0 (demo) */
	{
	.phys_start = 0x7f8f0000,
	.virt_start = 0x7f8f0000,
	.size = 0x1000,
	.flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_ROOTSHARED,
	},
	{
	.phys_start = 0x7f8f1000,
	.virt_start = 0x7f8f1000,
	.size = 0x9000,
	.flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_WRITE |
		JAILHOUSE_MEM_ROOTSHARED,
	},
	{
	.phys_start = 0x7f8fa000,
	.virt_start = 0x7f8fa000,
	.size = 0x2000,
	.flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_ROOTSHARED,
	},
	{
	.phys_start = 0x7f8fc000,
	.virt_start = 0x7f8fc000,
	.size = 0x2000,
	.flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_ROOTSHARED,
	},
	{
	.phys_start = 0x7f8fe000,
	.virt_start = 0x7f8fe000,
	.size = 0x2000,
	.flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_WRITE |
		JAILHOUSE_MEM_ROOTSHARED,
	},
"#;

pub const IVSHMEM_TEMPLATE: &'static str = r#"JAILHOUSE_SHMEM_NET_REGIONS({address}, 1),"#;

pub const IRQ_CHIP_TEMPLATE: &'static str = r#"
.irqchips = {
	/* GIC */ {
		.address = {gic_address},
		.pin_base = 32,
		.pin_bitmap = {
			1 << ({uart_pin} - 32),
			0,
			0,
			(1 << (140 - 128)) | (1 << ({ivshmem_pin} - 128))
		},
	},
},
"#;

pub const IRQ_CHIP_BOARD_TEMPLATE: &'static str = r#"
.irqchips = {
	/* GIC */ {
		.address = {gic_address},
		.pin_base = 32,
		.pin_bitmap = {
			1 << ({uart_pin} - 32),
			0,
			0,
			(1 << ({ivshmem_pin} - 128))
		},
	},
},
"#;

pub const PCI_DEVICE_TEMPLATE_WITH_DEMO: &'static str = r#"
.pci_devices = {
{ /* IVSHMEM 00:00.0 (demo) */
	.type = JAILHOUSE_PCI_TYPE_IVSHMEM,
	.domain = 1,
	.bdf = 0 << 3,
	.bar_mask = JAILHOUSE_IVSHMEM_BAR_MASK_INTX,
	.shmem_regions_start = 0,
	.shmem_dev_id = {ivshmem_bdf},
	.shmem_peers = 3,
	.shmem_protocol = JAILHOUSE_SHMEM_PROTO_UNDEFINED,
},
{ /* IVSHMEM 00:0{ivshmem_bdf}.0 (networking) */
	.type = JAILHOUSE_PCI_TYPE_IVSHMEM,
	.domain = 1, 
	.bdf = {ivshmem_bdf} << 3,
	.bar_mask = JAILHOUSE_IVSHMEM_BAR_MASK_INTX,
	.shmem_regions_start = 5,
	.shmem_dev_id = 1,
	.shmem_peers = 2,
	.shmem_protocol = JAILHOUSE_SHMEM_PROTO_VETH,
},
},
"#;

pub const PCI_DEVICE_TEMPLATE: &'static str = r#"
.pci_devices = {
{ /* IVSHMEM 00:0{ivshmem_bdf}.0 (networking) */
	.type = JAILHOUSE_PCI_TYPE_IVSHMEM,
	.domain = 1, 
	.bdf = {ivshmem_bdf} << 3,
	.bar_mask = JAILHOUSE_IVSHMEM_BAR_MASK_INTX,
	.shmem_regions_start = 0,
	.shmem_dev_id = 1,
	.shmem_peers = 2,
	.shmem_protocol = JAILHOUSE_SHMEM_PROTO_VETH,
},
},
"#;

pub const PCI_DEVICE_EMPTY_TEMPLATE: &'static str = r#"
.pci_devices = {
},
"#;

pub const QEMU_PREAMBLE_TEMPLATE: &'static str = r#"
#include "cell.h"

struct {
	struct jailhouse_cell_desc cell;
	__u64 cpus[1];
} __attribute__((packed)) config = {
	.cell = {
		.signature = JAILHOUSE_CELL_DESC_SIGNATURE,
		.revision = JAILHOUSE_CONFIG_REVISION,
		.name = "{containerid}",
		.flags = JAILHOUSE_CELL_PASSIVE_COMMREG |
			JAILHOUSE_CELL_VIRTUAL_CONSOLE_PERMITTED,

		.cpu_set_size = sizeof(config.cpus),
		.num_memory_regions = ARRAY_SIZE(config.mem_regions),
		.num_irqchips = ARRAY_SIZE(config.irqchips),
		.num_pci_devices = ARRAY_SIZE(config.pci_devices),

		.vpci_irq_base = 140-32,
		.cpu_reset_address = 0x70000000,

		.console = {
			.address = 0x09000000,
			.type = JAILHOUSE_CON_TYPE_PL011,
			.flags = JAILHOUSE_CON_ACCESS_MMIO |
				 JAILHOUSE_CON_REGDIST_4,
		},
	},
"#;

pub const ULTRASCALE_PREAMBLE_TEMPLATE: &'static str = r#"
#include "cell.h"

struct {
	struct jailhouse_cell_desc cell;
	__u64 cpus[1];
	__u64 rcpus[1];
} __attribute__((packed)) config = {
	.cell = {
		.signature = JAILHOUSE_CELL_DESC_SIGNATURE,
		.revision = JAILHOUSE_CONFIG_REVISION,
		.architecture = JAILHOUSE_ARM64,
		.name = "{containerid}",
		.flags = JAILHOUSE_CELL_PASSIVE_COMMREG,

		.cpu_set_size = sizeof(config.cpus),
		.rcpu_set_size = sizeof(config.rcpus),
		.num_memory_regions = ARRAY_SIZE(config.mem_regions),
		.num_irqchips = ARRAY_SIZE(config.irqchips),
		.num_pci_devices = ARRAY_SIZE(config.pci_devices),

		.console = {
			.address = 0xff010000,
			.type = JAILHOUSE_CON_TYPE_XUARTPS,
			.flags = JAILHOUSE_CON_ACCESS_MMIO |
				 JAILHOUSE_CON_REGDIST_4,
		},
	},
"#;

pub const SHM_TEMPLATE: &'static str = r#"
/* SHM */ {
	.phys_start = 0x46d00000,
	.virt_start = 0x46d00000,
	.size = 0x10000,
	.flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_WRITE |
		JAILHOUSE_MEM_ROOTSHARED, 
},
"#;

// Function to create a HashMap of all available templates
pub fn get_templates_map() -> HashMap<&'static str, &'static str> {
    let mut templates = HashMap::new();
    
    // Add templates to the HashMap
    templates.insert("RAM_TEMPLATE", RAM_TEMPLATE);
    templates.insert("RAM0_TEMPLATE", RAM0_TEMPLATE);
    templates.insert("UART_TEMPLATE", UART_TEMPLATE);
    templates.insert("COMM_REGION_TEMPLATE", COMM_REGION_TEMPLATE);
    templates.insert("IVSHMEM_TEMPLATE", IVSHMEM_TEMPLATE);
    templates.insert("IVSHMEM_DEMO_TEMPLATE", IVSHMEM_DEMO_TEMPLATE);
    templates.insert("IRQ_CHIP_TEMPLATE", IRQ_CHIP_TEMPLATE);
	templates.insert("IRQ_CHIP_BOARD_TEMPLATE", IRQ_CHIP_BOARD_TEMPLATE);
    templates.insert("PCI_DEVICE_TEMPLATE", PCI_DEVICE_TEMPLATE);
    templates.insert("PCI_DEVICE_TEMPLATE_WITH_DEMO", PCI_DEVICE_TEMPLATE_WITH_DEMO);
    templates.insert("PCI_DEVICE_EMPTY_TEMPLATE", PCI_DEVICE_EMPTY_TEMPLATE);
    templates.insert("TCMA_TEMPLATE", TCMA_TEMPLATE);
    templates.insert("TCMB_TEMPLATE", TCMB_TEMPLATE);
	templates.insert("QEMU_PREAMBLE_TEMPLATE", QEMU_PREAMBLE_TEMPLATE);
	templates.insert("ULTRASCALE_PREAMBLE_TEMPLATE", ULTRASCALE_PREAMBLE_TEMPLATE);
	templates.insert("SHM_TEMPLATE", SHM_TEMPLATE);

	templates
}
