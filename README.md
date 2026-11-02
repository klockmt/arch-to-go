# arch-to-go

Instala Arch Linux (rolling) com Hyprland num SSD USB externo, sem tocar no disco do sistema.

## Uso

Rode a partir de um usuário comum com `sudo` disponível, com o SSD conectado:

```sh
sudo ./install.sh /dev/sdX           # instala de verdade
./install.sh /dev/sdX --dry-run      # só mostra os comandos, não altera nada
```

Antes de instalar, o script:

1. Recusa o disco se ele não for USB, se não for disco inteiro, se tiver partição montada ou se for um disco do sistema atual.
2. Mostra o `lsblk` do alvo e pede que você digite o caminho exato (ex.: `/dev/sdb`).

O instalador pergunta o nome e a senha da conta. Essa conta entra no grupo `wheel` (sudo) e recebe a mesma senha como root.

Variáveis opcionais: `ARCH_HOSTNAME`, `ARCH_LOCALE`, `ARCH_TZ`, `ESP_SIZE`, `BTRFS_OPTS`, `LY_UNIT`.

## O que acontece

- **Apaga tudo** no disco informado (tabela GPT, ESP e btrfs).
- ESP FAT32 de 1 GiB em `/boot` e btrfs no restante, com subvolumes `@`, `@home`, `@log` e `@pkg`.
- Montagem com `noatime,ssd,discard=async,nodatacow`.
- Base: `base linux linux-firmware btrfs-progs sudo`, fstab por UUID, initramfs sem `autodetect`.
- Bootloader Limine instalado como removível (`EFI/BOOT/BOOTX64.EFI`). A NVRAM do computador onde você roda o instalador não é alterada.
- Desktop: Hyprland, kitty, fish, nautilus, `ttf-dejavu`. Login pelo ly (`LY_UNIT`, padrão `ly@tty1.service`).
- Rede: iwd só para Wi-Fi; systemd-networkd faz DHCP; systemd-resolved faz DNS.
- tuned e tuned-ppd habilitados.
- paru compilado no host e instalado no alvo com `pacman -U`.

## Avisos

- **nodatacow** desliga CoW e checksums do btrfs. Um erro de disco ou de USB pode corromper dados sem aviso. Para voltar ao padrão, ajuste `BTRFS_OPTS` (sem `nodatacow`) antes de instalar.
- Para usar o SSD depois, escolha-o no menu de boot da placa (UEFI). O Limine está no caminho removível, então a placa acha sem entrada na NVRAM.
- Atualizações do pacote `limine` recopiam o EFI pelo hook em `/etc/pacman.d/hooks/limine.hook`.

## Testes

```sh
cargo test               # inclui as checagens de disco, com lsblk/findmnt simulados
shellcheck install.sh lib/*.sh
```

## Pendências (fora desta etapa)

- Kernel mainline com script de compilação.
- App "hello" em Rust com iced (fork do CachyOS Hello).
- Teste de ponta a ponta em loop device antes do disco real.
