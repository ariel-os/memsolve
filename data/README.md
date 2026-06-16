# Example description files

This directory contains a number of example sections descriptions:

- [nrf52840.yaml](./nrf52840.yaml): chip memory layout of the Nordic nRF52840.
- [app.yaml](./app.yaml): application space, set to maximum size.
- [bootloader.yaml](./bootloader.yaml): bootloader, set to bootable
  and a minimum byte size.
- [dfu.yaml](./dfu.yaml): space for a second application image for an update.
  Must be one page larger than the application space to facilitate swapping them
- [storage.yaml](./storage.yaml): configuration storage, must be 2 pages,
  but no specific byte size
- [larger_storage.yaml](./larger_storage.yaml):
  increased configuration storage space.

These files can be used together with the [`multifile.rs` example](../examples/multifile.rs)
to generate ``LD_MEMORY`` memory layouts
