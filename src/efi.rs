type EFI_STATUS = u32;
struct EfiBootServices{
    locate_protocol: extern "cstd" fn() -> EFI_STATUS
}