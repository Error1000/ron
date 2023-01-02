# Ron
A kernel and accompanying utilities written in rust, aimed at
being as architecture agnostic/portable as possible.
Current goal: make it work on bare metal, i want to see an application (e.g. bash), loaded from disk, run on it, on bare metal

Info:
 - Framebuffer support for vga and uefi gop
 - Half-baked terminal driver
 - Ext2 Filesystem ( decent, but no support for creating hard links, or managing any kind of timestamps or permissions yet ) (N.B. Right now only supports ata, but i'm just going to ignore that and come back later to do usb/nvme when it actually becomes a problem, as right now i do have an ancient laptop on which the kernel can see the drives, so it would be possible to boot the kernel on that laptop )
 - Has a couple of ports in the ports folder, including a basic shell

