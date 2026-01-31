Firmware updates should be possible to support.
The official software update tool is written in C# using .NET Framework v4.8 which is easy to decompile.

The mouse is first put into firmware update mode which changes it's PID.
It uses USB HID with a custom protocol to upload the encrypted (and possibly signed) update file.
The entire process shouldn't be too hard to replicate using
