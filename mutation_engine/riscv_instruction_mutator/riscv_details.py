from enum import Enum

REG_RANGE = [
    "ZERO", "RA", "SP", "GP", "TP", "T0", "T1", "T2", "S0", "S1", "A0", "A1",
    "A2", "A3", "A4", "A5", "A6", "A7", "S2", "S3", "S4", "S5", "S6", "S7",
    "S8", "S9", "S10", "S11", "T3", "T4", "T5", "T6"
]

FLOAT_RANGE = [
    "FT0", "FT1", "FT2", "FT3", "FT4", "FT5", "FT6", "FT7",
    "FS0", "FS1", "FA0", "FA1", "FA2", "FA3", "FA4", "FA5",
    "FA6", "FA7", "FS2", "FS3", "FS4", "FS5", "FS6", "FS7",
    "FS8", "FS9", "FS10", "FS11", "FT8", "FT9", "FT10", "FT11"
]

RVC_REG_RANGE = [
    "S0", "S1", "A0", "A1", "A2", "A3", "A4", "A5"
]

RVC_FLOAT_RANGE = [
    "FS0", "FS1", "FA0", "FA1", "FA2", "FA3", "FA4", "FA5"
]

CSR_RANGE = [
    # M-Mode
    'MVENDORID', 'MARCHID', 'MIMPID', 'MHARTID',
    'MTVEC', 'MIDELEG', 'MIP', 'MCOUNTEREN', 'MCOUNTINHIBIT',
    'MSCRATCH', 'MEPC', 'MCAUSE', 'MTVAL',
    # 'MEDELEG', 'MIE', 'MSTATUS', 'MISA'

    # S-Mode
    'SSTATUS', 'STVEC', 'SIP', 'SCOUNTEREN', 'SSCRATCH', 'SEPC', 'SCAUSE',
    'STVAL', 'SIE', 'SATP',

    # VS-Mode
    'VSSTATUS', 'VSTVEC', 'VSIP', 'VSIE', 'VSSCRATCH', 'VSEPC', 'VSCAUSE', 'VSATP', 'VSTVAL'

    # Unknown
    # 'SENVCFG', 'MSECCFG', 'MTIMECMP', 'MCONFIGPTR', 'MTIME', 'MENVCFG', 'MSTATUSH', 'MENVCFGH',
]