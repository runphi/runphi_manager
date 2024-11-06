# 0 "cell.c"
# 0 "<built-in>"
# 0 "<command-line>"
# 1 "/home/dottavia/runPHI/runPHI_project/build/buildroot/output/host/aarch64-buildroot-linux-gnu/sysroot/usr/include/stdc-predef.h" 1 3 4
# 0 "<command-line>" 2
# 1 "cell.c"
# 16 "cell.c"
# 1 "../../hypervisor/include/jailhouse/types.h" 1
# 16 "../../hypervisor/include/jailhouse/types.h"
# 1 "../../hypervisor/arch/arm64/include/asm/types.h" 1
# 17 "../../hypervisor/include/jailhouse/types.h" 2

#ifndef ARRAY_SIZE
#define ARRAY_SIZE(a) sizeof(a) / sizeof(a[0])
#endif


#define JAILHOUSE_CONFIG_REVISION       13

#define JAILHOUSE_CELL_NAME_MAXLEN      31

#define JAILHOUSE_CELL_PASSIVE_COMMREG  0x00000001
#define JAILHOUSE_CELL_TEST_DEVICE      0x00000002
#define JAILHOUSE_CELL_AARCH32          0x00000004


#define JAILHOUSE_CELL_VIRTUAL_CONSOLE_PERMITTED        0x40000000
#define JAILHOUSE_CELL_VIRTUAL_CONSOLE_ACTIVE           0x80000000

#define CELL_FLAGS_VIRTUAL_CONSOLE_ACTIVE(flags) \
        !!((flags) & JAILHOUSE_CELL_VIRTUAL_CONSOLE_ACTIVE)
#define CELL_FLAGS_VIRTUAL_CONSOLE_PERMITTED(flags) \
        !!((flags) & JAILHOUSE_CELL_VIRTUAL_CONSOLE_PERMITTED)

#define JAILHOUSE_CELL_DESC_SIGNATURE   "JHCELL"


#define JAILHOUSE_MEM_READ              0x0001
#define JAILHOUSE_MEM_WRITE             0x0002
#define JAILHOUSE_MEM_EXECUTE           0x0004
#define JAILHOUSE_MEM_DMA               0x0008
#define JAILHOUSE_MEM_IO                0x0010
#define JAILHOUSE_MEM_COMM_REGION       0x0020
#define JAILHOUSE_MEM_LOADABLE          0x0040
#define JAILHOUSE_MEM_ROOTSHARED        0x0080
#define JAILHOUSE_MEM_NO_HUGEPAGES      0x0100
#define JAILHOUSE_MEM_COLORED           0x0200
#define JAILHOUSE_MEM_COLORED_NO_COPY   0x0400
/* Set internally for remap_to/unmap_from root ops */
#define JAILHOUSE_MEM_TMP_ROOT_REMAP    0x0800
#define JAILHOUSE_MEM_IO_UNALIGNED      0x8000
#define JAILHOUSE_MEM_IO_WIDTH_SHIFT    16 /* uses bits 16..19 */
#define JAILHOUSE_MEM_IO_8              (1 << JAILHOUSE_MEM_IO_WIDTH_SHIFT)
#define JAILHOUSE_MEM_IO_16             (2 << JAILHOUSE_MEM_IO_WIDTH_SHIFT)
#define JAILHOUSE_MEM_IO_32             (4 << JAILHOUSE_MEM_IO_WIDTH_SHIFT)
#define JAILHOUSE_MEM_IO_64             (8 << JAILHOUSE_MEM_IO_WIDTH_SHIFT)


#define JAILHOUSE_SHMEM_NET_REGIONS(start, dev_id)                      \
        {                                                               \
                .phys_start = start,                                    \
                .virt_start = start,                                    \
                .size = 0x1000,                                         \
                .flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_ROOTSHARED, \
        },                                                              \
        { 0 },                                                          \
        {                                                               \
                .phys_start = (start) + 0x1000,                         \
                .virt_start = (start) + 0x1000,                         \
                .size = 0x7f000,                                        \
                .flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_ROOTSHARED | \
                        ((dev_id == 0) ? JAILHOUSE_MEM_WRITE : 0),      \
        },                                                              \
        {                                                               \
                .phys_start = (start) + 0x80000,                        \
                .virt_start = (start) + 0x80000,                        \
                .size = 0x7f000,                                        \
                .flags = JAILHOUSE_MEM_READ | JAILHOUSE_MEM_ROOTSHARED | \
                        ((dev_id == 1) ? JAILHOUSE_MEM_WRITE : 0),      \
        }

#define JAILHOUSE_MEMORY_IS_SUBPAGE(mem)        \
        ((mem)->virt_start & PAGE_OFFS_MASK || (mem)->size & PAGE_OFFS_MASK)

#define JAILHOUSE_CACHE_L3_CODE         0x01
#define JAILHOUSE_CACHE_L3_DATA         0x02
#define JAILHOUSE_CACHE_L3              (JAILHOUSE_CACHE_L3_CODE | \
                                         JAILHOUSE_CACHE_L3_DATA)

#define JAILHOUSE_CACHE_ROOTSHARED      0x0001


#define JAILHOUSE_PCI_TYPE_DEVICE       0x01
#define JAILHOUSE_PCI_TYPE_BRIDGE       0x02
#define JAILHOUSE_PCI_TYPE_IVSHMEM      0x03

#define JAILHOUSE_SHMEM_PROTO_UNDEFINED         0x0000
#define JAILHOUSE_SHMEM_PROTO_VETH              0x0001
#define JAILHOUSE_SHMEM_PROTO_CUSTOM            0x4000  /* 0x4000..0x7fff */
#define JAILHOUSE_SHMEM_PROTO_VIRTIO_FRONT      0x8000  /* 0x8000..0xbfff */
#define JAILHOUSE_SHMEM_PROTO_VIRTIO_BACK       0xc000  /* 0xc000..0xffff */

#define VIRTIO_DEV_NET                          1
#define VIRTIO_DEV_BLOCK                        2
#define VIRTIO_DEV_CONSOLE                      3


#define JAILHOUSE_IVSHMEM_BAR_MASK_INTX                 \
        {                                               \
                0xfffff000, 0x00000000, 0x00000000,     \
                0x00000000, 0x00000000, 0x00000000,     \
        }

#define JAILHOUSE_IVSHMEM_BAR_MASK_MSIX                 \
        {                                               \
                0xfffff000, 0xfffff000, 0x00000000,     \
                0x00000000, 0x00000000, 0x00000000,     \
        }

#define JAILHOUSE_IVSHMEM_BAR_MASK_INTX_64K             \
        {                                               \
                0xffff0000, 0x00000000, 0x00000000,     \
                0x00000000, 0x00000000, 0x00000000,     \
        }

#define JAILHOUSE_IVSHMEM_BAR_MASK_MSIX_64K             \
        {                                               \
                0xffff0000, 0xffff0000, 0x00000000,     \
                0x00000000, 0x00000000, 0x00000000,     \
        }

#define JAILHOUSE_PCI_EXT_CAP           0x8000

#define JAILHOUSE_PCICAPS_WRITE         0x0001



#define JAILHOUSE_APIC_MODE_AUTO        0
#define JAILHOUSE_APIC_MODE_XAPIC       1
#define JAILHOUSE_APIC_MODE_X2APIC      2

#define JAILHOUSE_MAX_IOMMU_UNITS       8

#define JAILHOUSE_IOMMU_AMD             1
#define JAILHOUSE_IOMMU_INTEL           2
#define JAILHOUSE_IOMMU_SMMUV3          3
#define JAILHOUSE_IOMMU_PVU             4
#define JAILHOUSE_IOMMU_ARM_MMU500      5

#define PIO_RANGE(__base, __length)     \
        {                               \
                .base = __base,         \
                .length = __length,     \
        }

#define JAILHOUSE_SYSTEM_SIGNATURE      "JHSYST"

/*
 * The flag JAILHOUSE_SYS_VIRTUAL_DEBUG_CONSOLE allows the root cell to read
 * from the virtual console.
 */
#define JAILHOUSE_SYS_VIRTUAL_DEBUG_CONSOLE     0x0001

#define SYS_FLAGS_VIRTUAL_DEBUG_CONSOLE(flags) \
        !!((flags) & JAILHOUSE_SYS_VIRTUAL_DEBUG_CONSOLE)

/**
 * Memguard total number of PMU Interrupts (one for each CPU).
 */
#define JAILHOUSE_MAX_PMU2CPU_IRQ       8

#define FLAGS_HAS_RWQOS         (1 << 0)
#define FLAGS_HAS_REGUL         (1 << 1)
#define FLAGS_HAS_DYNQOS        (1 << 2)

/* Board-independent QoS support */
#define FLAGS_HAS_RWQOS		(1 << 0)
#define FLAGS_HAS_REGUL		(1 << 1)
#define FLAGS_HAS_DYNQOS	(1 << 2)

/* Offsets of control registers from beginning of device-specific
 * config space */

/* The typical QoS interface has the following layout:
 *
 * BASE: 0x??80
 * read_qos    = BASE
 * write_qos   = + 0x04
 * fn_mod      = + 0x08
----- REGULATION ------
 * qos_cntl    = + 0x0C
 * max_ot      = + 0x10
 * max_comb_ot = + 0x14
 * aw_p        = + 0x18
 * aw_b        = + 0x1C
 * aw_r        = + 0x20
 * ar_p        = + 0x24
 * ar_b        = + 0x28
 * ar_r        = + 0x2C
----- DYNAMIC QOS -----
 * tgt_latency = + 0x30
 * ki          = + 0x34
 * qos_range   = + 0x38
 */

#define READ_QOS           0x00
#define WRITE_QOS          0x04
#define FN_MOD             0x08
#define QOS_CNTL           0x0C
#define MAX_OT             0x10
#define MAX_COMB_OT        0x14
#define AW_P               0x18
#define AW_B               0x1C
#define AW_R               0x20
#define AR_P               0x24
#define AR_B               0x28
#define AR_R               0x2C
#define TGT_LATENCY        0x30
#define KI                 0x34
#define QOS_RANGE          0x38

/* QOS_CNTL REgister  */
#define EN_AWAR_OT_SHIFT    (7)
#define EN_AR_OT_SHIFT      (6)
#define EN_AW_OT_SHIFT      (5)
#define EN_AR_LATENCY_SHIFT (4)
#define EN_AW_LATENCY_SHIFT (3)
#define EN_AWAR_RATE_SHIFT  (2)
#define EN_AR_RATE_SHIFT    (1)
#define EN_AW_RATE_SHIFT    (0)
#define EN_NO_ENABLE        (31)

/* Number of settable QoS parameters */
#define QOS_PARAMS          22

/* Bit fields and masks in control registers  */
#define READ_QOS_SHIFT      (0)
#define READ_QOS_MASK       (0x0f)
#define WRITE_QOS_SHIFT     (0)
#define WRITE_QOS_MASK      (0x0f)

#define AW_MAX_OTF_SHIFT    (0)
#define AW_MAX_OTI_SHIFT    (8)
#define AR_MAX_OTF_SHIFT    (16)
#define AR_MAX_OTI_SHIFT    (24)
#define AW_MAX_OTF_MASK     (0xff)
#define AW_MAX_OTI_MASK     (0x3f)
#define AR_MAX_OTF_MASK     (0xff)
#define AR_MAX_OTI_MASK     (0x3f)

#define AWAR_MAX_OTF_SHIFT  (0)
#define AWAR_MAX_OTI_SHIFT  (8)
#define AWAR_MAX_OTF_MASK   (0xff)
#define AWAR_MAX_OTI_MASK   (0x7f)

#define AW_P_SHIFT          (24)
#define AW_B_SHIFT          (0)
#define AW_R_SHIFT          (20)
#define AW_P_MASK           (0xff)
#define AW_B_MASK           (0xffff)
#define AW_R_MASK           (0xfff)

#define AR_P_SHIFT          (24)
#define AR_B_SHIFT          (0)
#define AR_R_SHIFT          (20)
#define AR_P_MASK           (0xff)
#define AR_B_MASK           (0xffff)
#define AR_R_MASK           (0xfff)

#define AR_TGT_LAT_SHIFT    (16)
#define AW_TGT_LAT_SHIFT    (0)
#define AR_TGT_LAT_MASK     (0xfff)
#define AW_TGT_LAT_MASK     (0xfff)

#define AR_KI_SHIFT         (8)
#define AW_KI_SHIFT         (0)
#define AR_KI_MASK          (0x7)
#define AW_KI_MASK          (0x7)

#define AR_MAX_QOS_SHIFT    (24)
#define AR_MIN_QOS_SHIFT    (16)
#define AW_MAX_QOS_SHIFT    (8)
#define AW_MIN_QOS_SHIFT    (0)
#define AR_MAX_QOS_MASK     (0xf)
#define AR_MIN_QOS_MASK     (0xf)
#define AW_MAX_QOS_MASK     (0xf)
#define AW_MIN_QOS_MASK     (0xf)

/* Those definitions are used for the type in struct jailhouse_console */
#define JAILHOUSE_CON_TYPE_NONE		0x0000
#define JAILHOUSE_CON_TYPE_EFIFB	0x0001
#define JAILHOUSE_CON_TYPE_8250		0x0002
#define JAILHOUSE_CON_TYPE_PL011	0x0003
#define JAILHOUSE_CON_TYPE_XUARTPS	0x0004
#define JAILHOUSE_CON_TYPE_MVEBU	0x0005
#define JAILHOUSE_CON_TYPE_HSCIF	0x0006
#define JAILHOUSE_CON_TYPE_SCIFA	0x0007
#define JAILHOUSE_CON_TYPE_IMX		0x0008
#define JAILHOUSE_CON_TYPE_IMX_LPUART	0x0009
#define JAILHOUSE_CON_TYPE_LINFLEX	0x000a

/* Flags: bit 0 is used to select PIO (cleared) or MMIO (set) access */
#define JAILHOUSE_CON_ACCESS_PIO	0x0000
#define JAILHOUSE_CON_ACCESS_MMIO	0x0001

#define CON_IS_MMIO(flags) !!((flags) & JAILHOUSE_CON_ACCESS_MMIO)

/*
 * Flags: bit 1 is used to select 1 (cleared) or 4-bytes (set) register distance.
 * 1 byte implied 8-bit access, 4 bytes 32-bit access.
 */
#define JAILHOUSE_CON_REGDIST_1		0x0000
#define JAILHOUSE_CON_REGDIST_4		0x0002

#define CON_USES_REGDIST_1(flags) (((flags) & JAILHOUSE_CON_REGDIST_4) == 0)

/* Flags: bit 2 is used to select framebuffer format */
#define JAILHOUSE_CON_FB_1024x768	0x0000
#define JAILHOUSE_CON_FB_1920x1080	0x0004

#define FB_IS_1920x1080(flags) !!((flags) & JAILHOUSE_CON_FB_1920x1080)

/* Bits 3-11: Reserved */

/* Bit 12 is used to indicate to clear instead of to set the clock gate */
#define JAILHOUSE_CON_INVERTED_GATE	0x1000

#define CON_HAS_INVERTED_GATE(flags)	!!((flags) & JAILHOUSE_CON_INVERTED_GATE)

/* Bit 13 is used to apply (set) or skip (clear) a MDR quirk on the console */
#define JAILHOUSE_CON_MDR_QUIRK		0x2000

#define CON_HAS_MDR_QUIRK(flags)	!!((flags) & JAILHOUSE_CON_MDR_QUIRK)

/* LPD_OFFSET: (0xFE100000) - (0xFD700000) = 0xA00000 */
#define LPD_OFFSET 		0xA00000

/* Peripherials in LPD with QoS Support */
#define M_RPU0_BASE		LPD_OFFSET + (0x42100)
#define M_RPU1_BASE		LPD_OFFSET + (0x43100)
#define M_ADMA_BASE 		LPD_OFFSET + (0x44100)
#define M_AFIFM6_BASE		LPD_OFFSET + (0x45100)
#define M_DAP_BASE		LPD_OFFSET + (0x47100)
#define M_USB0_BASE		LPD_OFFSET + (0x48100)
#define M_USB1_BASE		LPD_OFFSET + (0x49100)
#define M_INTIOU_BASE		LPD_OFFSET + (0x4A100)
#define M_INTCSUPMU_BASE	LPD_OFFSET + (0x4B100)
#define M_INTLPDINBOUND_BASE	LPD_OFFSET + (0x4C100)
#define M_INTLPDOCM_BASE	LPD_OFFSET + (0x4D100)
#define M_IB5_BASE 		LPD_OFFSET + (0xC3100)
#define M_IB6_BASE		LPD_OFFSET + (0xC4100)
#define M_IB8_BASE 		LPD_OFFSET + (0xC5100)
#define M_IB0_BASE		LPD_OFFSET + (0xC6100)
#define M_IB11_BASE 		LPD_OFFSET + (0xC7100)
#define M_IB12_BASE 		LPD_OFFSET + (0xC8100)

/* Peripherials in FPD with QoS Support */
#define M_INTFPDCCI_BASE 	(0x42100)
#define M_INTFPDSMMUTBU3_BASE	(0x43100)
#define M_INTFPDSMMUTBU4_BASE	(0x44100)
#define M_AFIFM0_BASE		(0x45100)
#define M_AFIFM1_BASE		(0x46100)
#define M_AFIFM2_BASE		(0x47100)
#define M_INITFPDSMMUTBU5_BASE	(0x48100)
#define M_DP_BASE		(0x49100)
#define M_AFIFM3_BASE		(0x4A100)
#define M_AFIFM4_BASE		(0x4B100)
#define M_AFIFM5_BASE		(0x4C100)
#define M_GPU_BASE 		(0x4D100)
#define M_PCIE_BASE		(0x4E100)
#define M_GDMA_BASE		(0x4F100)
#define M_SATA_BASE 		(0x50100)
#define M_CORESIGHT_BASE	(0x52100)
#define ISS_IB2_BASE		(0xC2100)
#define ISS_IB6_BASE		(0xC3100)


typedef enum { true = 1, false = 0 } bool;


struct cpu_set {

 unsigned long max_cpu_id;



 unsigned long bitmap[1];
};

typedef signed char s8;
typedef unsigned char u8;

typedef signed short s16;
typedef unsigned short u16;

typedef signed int s32;
typedef unsigned int u32;

typedef signed long long s64;
typedef unsigned long long u64;

typedef s8 __s8;
typedef u8 __u8;

typedef s16 __s16;
typedef u16 __u16;

typedef s32 __s32;
typedef u32 __u32;

typedef s64 __s64;
typedef u64 __u64;


typedef unsigned long size_t;
# 17 "cell.c" 2
# 1 "../../include/jailhouse/cell-config.h" 1
# 43 "../../include/jailhouse/cell-config.h"
# 1 "../../include/jailhouse/console.h" 1
# 90 "../../include/jailhouse/console.h"
struct jailhouse_console {
 __u64 address;
 __u32 size;
 __u16 type;
 __u16 flags;
 __u32 divider;
 __u32 gate_nr;
 __u64 clock_reg;
} __attribute__((packed));
# 44 "../../include/jailhouse/cell-config.h" 2
# 1 "../../include/jailhouse/pci_defs.h" 1
# 45 "../../include/jailhouse/cell-config.h" 2
# 1 "../../include/jailhouse/qos-common.h" 1
# 19 "../../include/jailhouse/qos-common.h"
struct qos_setting {
 char dev_name [15];
 char param_name [16];
 __u32 value;
};
# 46 "../../include/jailhouse/cell-config.h" 2
# 86 "../../include/jailhouse/cell-config.h"
struct jailhouse_cell_desc {
 char signature[6];
 __u16 revision;

 char name[31 +1];
 __u32 id;
 __u32 flags;

 __u32 cpu_set_size;
 __u32 num_memory_regions;
 __u32 num_cache_regions;
 __u32 num_irqchips;
 __u32 num_pio_regions;
 __u32 num_pci_devices;
 __u32 num_pci_caps;
 __u32 num_stream_ids;
 __u32 num_qos_devices;

 __u32 vpci_irq_base;

 __u64 cpu_reset_address;
 __u64 msg_reply_timeout;

 struct jailhouse_console console;
} __attribute__((packed));
# 132 "../../include/jailhouse/cell-config.h"
struct jailhouse_memory {
 __u64 phys_start;
 __u64 virt_start;
 __u64 size;
 __u64 flags;

 __u64 colors;
} __attribute__((packed));

struct jailhouse_coloring {

 __u64 way_size;

 __u64 root_map_offset;
} __attribute__((packed));
# 181 "../../include/jailhouse/cell-config.h"
struct jailhouse_cache {
 __u32 start;
 __u32 size;
 __u8 type;
 __u8 padding;
 __u16 flags;
} __attribute__((packed));

struct jailhouse_irqchip {
 __u64 address;
 __u32 id;
 __u32 pin_base;
 __u32 pin_bitmap[4];
} __attribute__((packed));
# 210 "../../include/jailhouse/cell-config.h"
struct jailhouse_pci_device {
 __u8 type;
 __u8 iommu;
 __u16 domain;
 __u16 bdf;
 __u32 bar_mask[6];
 __u16 caps_start;
 __u16 num_caps;
 __u8 num_msi_vectors;
 __u8 msi_64bits:1;
 __u8 msi_maskable:1;
 __u16 num_msix_vectors;
 __u16 msix_region_size;
 __u64 msix_address;

 __u32 shmem_regions_start;

 __u8 shmem_dev_id;

 __u8 shmem_peers;

 __u16 shmem_protocol;
} __attribute__((packed));
# 262 "../../include/jailhouse/cell-config.h"
struct jailhouse_pci_capability {
 __u16 id;
 __u16 start;
 __u16 len;
 __u16 flags;
} __attribute__((packed));
# 281 "../../include/jailhouse/cell-config.h"
struct jailhouse_iommu {
 __u32 type;
 __u64 base;
 __u32 size;

 union {
  struct {
   __u16 bdf;
   __u8 base_cap;
   __u8 msi_cap;
   __u32 features;
  } __attribute__((packed)) amd;

  struct {
   __u64 tlb_base;
   __u32 tlb_size;
  } __attribute__((packed)) tipvu;
 };
} __attribute__((packed));

union jailhouse_stream_id {
 __u32 id;
 struct {

  __u16 id;



  __u16 mask_out;
 } __attribute__((packed)) mmu500;
} __attribute__((packed));

struct jailhouse_pio {
 __u16 base;
 __u16 length;
} __attribute__((packed));
# 344 "../../include/jailhouse/cell-config.h"
struct jailhouse_memguard_config {



 __u32 num_irqs;



 __u32 hv_timer;

 __u8 irq_prio_min;
 __u8 irq_prio_max;

 __u8 irq_prio_step;

 __u8 irq_prio_threshold;

 __u32 num_pmu_irq;

 __u32 pmu_cpu_irq[8];
} __attribute__((packed));

struct jailhouse_qos {
 __u64 nic_base;
 __u64 nic_size;
} __attribute__((packed));

struct jailhouse_qos_device {
 char name [15];
 __u8 flags;
 __u32 base;
} __attribute__((packed));





struct jailhouse_system {
 char signature[6];
 __u16 revision;
 __u32 flags;


 struct jailhouse_memory hypervisor_memory;
 struct jailhouse_console debug_console;
 struct {
  __u64 pci_mmconfig_base;
  __u8 pci_mmconfig_end_bus;
  __u8 pci_is_virtual;
  __u16 pci_domain;

  __u32 no_spectre_mitigation;
  struct jailhouse_iommu iommu_units[8];
  struct jailhouse_coloring color;
  struct jailhouse_memguard_config memguard;
  struct jailhouse_qos qos;
  union {
   struct {
    __u16 pm_timer_address;
    __u8 apic_mode;
    __u8 padding;
    __u32 vtd_interrupt_limit;
    __u32 tsc_khz;
    __u32 apic_khz;
   } __attribute__((packed)) x86;
   struct {
    u8 maintenance_irq;
    u8 gic_version;
    u8 padding[2];
    u64 gicd_base;
    u64 gicc_base;
    u64 gich_base;
    u64 gicv_base;
    u64 gicr_base;
   } __attribute__((packed)) arm;
  } __attribute__((packed));
 } __attribute__((packed)) platform_info;
 struct jailhouse_cell_desc root_cell;
} __attribute__((packed));

static inline __u32
jailhouse_cell_config_size(struct jailhouse_cell_desc *cell)
{
 return sizeof(struct jailhouse_cell_desc) +
  cell->cpu_set_size +
  cell->num_memory_regions * sizeof(struct jailhouse_memory) +
  cell->num_cache_regions * sizeof(struct jailhouse_cache) +
  cell->num_irqchips * sizeof(struct jailhouse_irqchip) +
  cell->num_pio_regions * sizeof(struct jailhouse_pio) +
  cell->num_pci_devices * sizeof(struct jailhouse_pci_device) +
  cell->num_pci_caps * sizeof(struct jailhouse_pci_capability) +
  cell->num_stream_ids * sizeof(__u32) +
  cell->num_qos_devices * sizeof(struct jailhouse_qos_device);
}

static inline __u32
jailhouse_system_config_size(struct jailhouse_system *system)
{
 return sizeof(*system) - sizeof(system->root_cell) +
  jailhouse_cell_config_size(&system->root_cell);
}

static inline const unsigned long *
jailhouse_cell_cpu_set(const struct jailhouse_cell_desc *cell)
{
 return (const unsigned long *)((const void *)cell +
  sizeof(struct jailhouse_cell_desc));
}

static inline const struct jailhouse_memory *
jailhouse_cell_mem_regions(const struct jailhouse_cell_desc *cell)
{
 return (const struct jailhouse_memory *)
  ((void *)jailhouse_cell_cpu_set(cell) + cell->cpu_set_size);
}

static inline const struct jailhouse_cache *
jailhouse_cell_cache_regions(const struct jailhouse_cell_desc *cell)
{
 return (const struct jailhouse_cache *)
  ((void *)jailhouse_cell_mem_regions(cell) +
   cell->num_memory_regions * sizeof(struct jailhouse_memory));
}

static inline const struct jailhouse_irqchip *
jailhouse_cell_irqchips(const struct jailhouse_cell_desc *cell)
{
 return (const struct jailhouse_irqchip *)
  ((void *)jailhouse_cell_cache_regions(cell) +
   cell->num_cache_regions * sizeof(struct jailhouse_cache));
}

static inline const struct jailhouse_pio *
jailhouse_cell_pio(const struct jailhouse_cell_desc *cell)
{
 return (const struct jailhouse_pio *)
  ((void *)jailhouse_cell_irqchips(cell) +
  cell->num_irqchips * sizeof(struct jailhouse_irqchip));
}

static inline const struct jailhouse_pci_device *
jailhouse_cell_pci_devices(const struct jailhouse_cell_desc *cell)
{
 return (const struct jailhouse_pci_device *)
  ((void *)jailhouse_cell_pio(cell) +
   cell->num_pio_regions * sizeof(struct jailhouse_pio));
}

static inline const struct jailhouse_pci_capability *
jailhouse_cell_pci_caps(const struct jailhouse_cell_desc *cell)
{
 return (const struct jailhouse_pci_capability *)
  ((void *)jailhouse_cell_pci_devices(cell) +
   cell->num_pci_devices * sizeof(struct jailhouse_pci_device));
}

static inline const union jailhouse_stream_id *
jailhouse_cell_stream_ids(const struct jailhouse_cell_desc *cell)
{
 return (const union jailhouse_stream_id *)
  ((void *)jailhouse_cell_pci_caps(cell) +
  cell->num_pci_caps * sizeof(struct jailhouse_pci_capability));
}

static inline const struct jailhouse_qos_device *
jailhouse_cell_qos_devices(const struct jailhouse_cell_desc *cell)
{
 return (const struct jailhouse_qos_device *)
  ((void *)jailhouse_cell_stream_ids(cell) +
   cell->num_stream_ids * sizeof(union jailhouse_stream_id));
}

