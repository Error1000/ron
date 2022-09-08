# Ron
A kernel and accompanying utilities written in rust, aimed at
being as architecture agnostic/portable as possible.
Current goal: make it work on bare metal, i want to see an application (e.g. bash), loaded from disk, run on it, on bare metal

TODO list:
 - Drivers ~ ( close enough, for now )
 - Framebuffer ~ ( actually kind of good ngl )
 - Char device ~ ( close enough, for now )
 - Memory allocator ~ ( simple design, but should work well enough )
 - Filesystem ~ ( decent, but no support for creating hard links, or managing any kind of timestamps or permissions yet ) (N.B. Right now only supports ata, but i'm just going to ignore everything else and come back later to do usb/nvme when it actually becomes a problem tho, as right now i do have an ancient laptop on which the kernel can see the drives, so it would be possible to boot the kernel on that laptop )
 - Emulator for running programs x ( decent )
 - Port programs X ( DOING )

