clear
rm -f serial_pipe
if cargo +nightly build --target x86_64-unknown-uefi --features "debug-mode"; then
mkfifo serial_pipe.in serial_pipe.out

./log_viewer < serial_pipe.out &

rm -rf ./esp
mkdir -p ./esp/EFI/BOOT
cp ./target/x86_64-unknown-uefi/debug/test_os_v2.efi ./esp/EFI/BOOT/BOOTx64.EFI
cp -r ./contents ./esp/EFI/BOOT

qemu-system-x86_64 \
  -bios /usr/share/ovmf/x64/OVMF.4m.fd \
  -drive file=fat:rw:./esp,format=raw \
  -serial pipe:serial_pipe \
  -device virtio-gpu-pci \
  -display gtk,gl=on \
  -machine q35 -m 16G -enable-kvm -cpu host \
  -no-reboot \
  -no-shutdown \
  -smp 2

# 4. 終わったら両方消す
rm -f serial_pipe.in serial_pipe.out
fi
